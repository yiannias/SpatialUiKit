//! Dates and times written the way the person's operating system writes them.
//!
//! The app stores times as UTC and converts to the local time zone itself (it
//! owns the clock and the zone database). What is left, and what belongs in
//! one shared place, is the *style*: `11/19/2026 6:29 PM` in a US region,
//! `19/11/2026 18:29` in the UK, `2026/11/19 18:29` in Japan, plus whatever
//! the person customised in Windows' Region settings. Every label in every
//! app goes through [`format_date_time`] / [`format_date`] / [`format_time`],
//! so no panel invents its own style (one mechanism per behaviour).
//!
//! # The seam
//!
//! [`RegionalFormat`] is plain data: the short date pattern, the short time
//! pattern and the AM/PM words. [`RegionalFormat::os`] asks the OS for them
//! (Windows: `GetLocaleInfoEx` for the user's own locale; macOS:
//! `CFDateFormatter` for the current locale); every other function here is
//! pure, so tests hand in a format for any region instead of depending on the
//! machine they run on.
//!
//! # Patterns
//!
//! Windows and ICU (macOS) write patterns almost the same way, and one reader
//! covers both: `d dd ddd dddd`, `M MM MMM MMMM`, `y yy yyyy`, `h hh H HH`,
//! `m mm`, `s ss`, `t tt` / `a` for AM/PM, `'quoted text'`, anything else
//! literal. Month and weekday *names* are English; a short date pattern that
//! spells them out is rare, and translated names need a locale database the
//! kit does not carry.
//!
//! Licence note: written from the published pattern syntax of
//! `GetLocaleInfoEx` and Unicode TR35; no code copied.

/// A calendar date and a wall-clock time, already in the zone to show.
/// The kit has no time-zone knowledge: the caller converts first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CivilDateTime {
    pub year: i32,
    /// 1..=12
    pub month: u32,
    /// 1..=31
    pub day: u32,
    /// 0..=23
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

/// The person's date and time style, as the OS reports it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegionalFormat {
    /// Short date pattern, e.g. `M/d/yyyy`, `dd/MM/yyyy`, `yyyy/MM/dd`.
    pub short_date: String,
    /// Short time pattern, e.g. `h:mm tt`, `HH:mm`.
    pub short_time: String,
    /// The word for before noon (`AM`, `a.m.`, `午前`).
    pub am: String,
    /// The word for after noon.
    pub pm: String,
}

impl RegionalFormat {
    /// A readable, unambiguous style for when the OS gives no answer:
    /// `2026-11-19 18:29`.
    pub fn fallback() -> Self {
        Self {
            short_date: "yyyy-MM-dd".into(),
            short_time: "HH:mm".into(),
            am: "AM".into(),
            pm: "PM".into(),
        }
    }

    /// The operating system's current date/time style for this person. Read
    /// each call (a few microseconds), so a change in Region settings shows up
    /// without restarting. Any part the OS does not supply falls back to
    /// [`RegionalFormat::fallback`].
    pub fn os() -> Self {
        let mut format = Self::fallback();
        if let Some(found) = os_format() {
            if !found.short_date.is_empty() {
                format.short_date = found.short_date;
            }
            if !found.short_time.is_empty() {
                format.short_time = found.short_time;
            }
            if !found.am.is_empty() {
                format.am = found.am;
            }
            if !found.pm.is_empty() {
                format.pm = found.pm;
            }
        }
        format
    }
}

/// Date then time, in the person's style: `11/19/2026 6:29 PM`.
pub fn format_date_time(at: &CivilDateTime, format: &RegionalFormat) -> String {
    format!("{} {}", format_date(at, format), format_time(at, format))
}

/// The date alone: `11/19/2026`.
pub fn format_date(at: &CivilDateTime, format: &RegionalFormat) -> String {
    apply_pattern(&format.short_date, at, format)
}

/// The time alone: `6:29 PM`. For use inside a sentence that already says
/// which day.
pub fn format_time(at: &CivilDateTime, format: &RegionalFormat) -> String {
    apply_pattern(&format.short_time, at, format)
}

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const WEEKDAYS: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];

/// 0 = Sunday, by days since 1970-01-01 (a Thursday).
fn weekday(year: i32, month: u32, day: u32) -> usize {
    // Howard Hinnant's days_from_civil.
    let y = i64::from(year) - i64::from(month <= 2);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (i64::from(month) + 9) % 12;
    let doy = (153 * mp + 2) / 5 + i64::from(day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    (days + 4).rem_euclid(7) as usize
}

fn apply_pattern(pattern: &str, at: &CivilDateTime, format: &RegionalFormat) -> String {
    let chars: Vec<char> = pattern.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\'' {
            // Quoted literal; '' is a single quote.
            i += 1;
            if i < chars.len() && chars[i] == '\'' {
                out.push('\'');
                i += 1;
                continue;
            }
            while i < chars.len() && chars[i] != '\'' {
                out.push(chars[i]);
                i += 1;
            }
            i += 1; // closing quote
            continue;
        }
        if !c.is_ascii_alphabetic() {
            out.push(c);
            i += 1;
            continue;
        }
        let mut run = 1;
        while i + run < chars.len() && chars[i + run] == c {
            run += 1;
        }
        i += run;
        let hour12 = if at.hour % 12 == 0 { 12 } else { at.hour % 12 };
        match c {
            'd' => match run {
                1 => out.push_str(&at.day.to_string()),
                2 => out.push_str(&format!("{:02}", at.day)),
                3 => out.push_str(&WEEKDAYS[weekday(at.year, at.month, at.day)][..3]),
                _ => out.push_str(WEEKDAYS[weekday(at.year, at.month, at.day)]),
            },
            'M' | 'L' => match run {
                1 => out.push_str(&at.month.to_string()),
                2 => out.push_str(&format!("{:02}", at.month)),
                3 => out.push_str(&MONTHS[(at.month as usize + 11) % 12][..3]),
                _ => out.push_str(MONTHS[(at.month as usize + 11) % 12]),
            },
            'y' => {
                if run == 2 {
                    out.push_str(&format!("{:02}", at.year.rem_euclid(100)));
                } else {
                    out.push_str(&format!("{:04}", at.year));
                }
            }
            'h' => out.push_str(&if run == 1 {
                hour12.to_string()
            } else {
                format!("{hour12:02}")
            }),
            'H' => out.push_str(&if run == 1 {
                at.hour.to_string()
            } else {
                format!("{:02}", at.hour)
            }),
            'm' => out.push_str(&if run == 1 {
                at.minute.to_string()
            } else {
                format!("{:02}", at.minute)
            }),
            's' => out.push_str(&if run == 1 {
                at.second.to_string()
            } else {
                format!("{:02}", at.second)
            }),
            // Windows `t`/`tt`, ICU `a`.
            't' | 'a' => {
                let word = if at.hour < 12 { &format.am } else { &format.pm };
                if c == 't' && run == 1 {
                    out.extend(word.chars().next());
                } else {
                    out.push_str(word);
                }
            }
            // Era and anything unknown: not part of a short pattern; keep it
            // visible rather than dropping a character the person's format has.
            'g' => {}
            _ => out.extend(std::iter::repeat_n(c, run)),
        }
    }
    out.trim().to_string()
}

#[cfg(windows)]
fn os_format() -> Option<RegionalFormat> {
    use windows_sys::Win32::Globalization::{
        GetLocaleInfoEx, LOCALE_S1159, LOCALE_S2359, LOCALE_SSHORTDATE, LOCALE_SSHORTTIME,
    };

    // A null locale name means "this user's default locale", which includes
    // the customisations made in Region settings.
    let read = |kind: u32| -> String {
        let mut buffer = [0u16; 160];
        // SAFETY: the buffer is valid for `buffer.len()` UTF-16 units and the
        // call writes at most that many, returning the count including the
        // terminating NUL (0 on failure).
        let written = unsafe {
            GetLocaleInfoEx(
                std::ptr::null(),
                kind,
                buffer.as_mut_ptr(),
                buffer.len() as i32,
            )
        };
        if written <= 1 {
            return String::new();
        }
        String::from_utf16_lossy(&buffer[..(written - 1) as usize])
    };
    Some(RegionalFormat {
        short_date: read(LOCALE_SSHORTDATE),
        short_time: read(LOCALE_SSHORTTIME),
        am: read(LOCALE_S1159),
        pm: read(LOCALE_S2359),
    })
}

/// macOS: the current locale's short date and short time styles through
/// CoreFoundation's `CFDateFormatter` (ICU patterns such as `M/d/yy`,
/// `h:mm a`). The AM/PM words stay `AM`/`PM`.
///
/// Not compiled or run on Windows; build it on a Mac before relying on it.
#[cfg(target_os = "macos")]
fn os_format() -> Option<RegionalFormat> {
    use std::ffi::c_void;

    const NO_STYLE: isize = 0;
    const SHORT_STYLE: isize = 1;
    const UTF8: u32 = 0x0800_0100;

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFLocaleCopyCurrent() -> *const c_void;
        fn CFDateFormatterCreate(
            allocator: *const c_void,
            locale: *const c_void,
            date_style: isize,
            time_style: isize,
        ) -> *const c_void;
        fn CFDateFormatterGetFormat(formatter: *const c_void) -> *const c_void;
        fn CFStringGetCString(
            string: *const c_void,
            buffer: *mut u8,
            size: isize,
            encoding: u32,
        ) -> u8;
        fn CFRelease(object: *const c_void);
    }

    let pattern = |date_style: isize, time_style: isize| -> String {
        // SAFETY: standard CoreFoundation create/get/release pairing; the
        // format string is owned by the formatter and copied out before it is
        // released; the buffer length is passed to CFStringGetCString.
        unsafe {
            let locale = CFLocaleCopyCurrent();
            if locale.is_null() {
                return String::new();
            }
            let formatter = CFDateFormatterCreate(std::ptr::null(), locale, date_style, time_style);
            let mut text = String::new();
            if !formatter.is_null() {
                let format = CFDateFormatterGetFormat(formatter);
                if !format.is_null() {
                    let mut buffer = [0u8; 256];
                    if CFStringGetCString(format, buffer.as_mut_ptr(), buffer.len() as isize, UTF8)
                        != 0
                    {
                        let end = buffer.iter().position(|b| *b == 0).unwrap_or(buffer.len());
                        text = String::from_utf8_lossy(&buffer[..end]).into_owned();
                    }
                }
                CFRelease(formatter);
            }
            CFRelease(locale);
            text
        }
    };
    Some(RegionalFormat {
        short_date: pattern(SHORT_STYLE, NO_STYLE),
        short_time: pattern(NO_STYLE, SHORT_STYLE),
        am: String::new(),
        pm: String::new(),
    })
}

#[cfg(not(any(windows, target_os = "macos")))]
fn os_format() -> Option<RegionalFormat> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(hour: u32, minute: u32) -> CivilDateTime {
        // Thursday 2026-11-19.
        CivilDateTime {
            year: 2026,
            month: 11,
            day: 19,
            hour,
            minute,
            second: 7,
        }
    }

    fn format(date: &str, time: &str) -> RegionalFormat {
        RegionalFormat {
            short_date: date.into(),
            short_time: time.into(),
            am: "AM".into(),
            pm: "PM".into(),
        }
    }

    #[test]
    fn a_us_region_writes_month_first_with_a_twelve_hour_clock() {
        let us = format("M/d/yyyy", "h:mm tt");
        assert_eq!(format_date_time(&at(18, 29), &us), "11/19/2026 6:29 PM");
    }

    #[test]
    fn a_uk_region_writes_day_first_with_a_twenty_four_hour_clock() {
        let uk = format("dd/MM/yyyy", "HH:mm");
        assert_eq!(format_date_time(&at(18, 29), &uk), "19/11/2026 18:29");
        assert_eq!(format_date_time(&at(6, 5), &uk), "19/11/2026 06:05");
    }

    #[test]
    fn german_and_japanese_styles_follow_their_own_separators() {
        let de = format("dd.MM.yyyy", "HH:mm");
        assert_eq!(format_date_time(&at(18, 29), &de), "19.11.2026 18:29");
        let ja = format("yyyy/MM/dd", "H:mm");
        assert_eq!(format_date_time(&at(6, 5), &ja), "2026/11/19 6:05");
    }

    #[test]
    fn midnight_and_noon_read_twelve_in_a_twelve_hour_clock() {
        let us = format("M/d/yyyy", "h:mm tt");
        assert_eq!(format_time(&at(0, 5), &us), "12:05 AM");
        assert_eq!(format_time(&at(12, 0), &us), "12:00 PM");
        assert_eq!(format_time(&at(13, 0), &us), "1:00 PM");
    }

    #[test]
    fn icu_patterns_from_macos_read_the_same_way() {
        // CFDateFormatter short styles: `M/d/yy` and `h:mm a`.
        let mac = format("M/d/yy", "h:mm a");
        assert_eq!(format_date_time(&at(18, 29), &mac), "11/19/26 6:29 PM");
    }

    #[test]
    fn the_regions_own_am_pm_words_and_single_letter_form_are_used() {
        let mut region = format("d.M.yyyy", "h:mm tt");
        region.am = "de.".into();
        region.pm = "du.".into();
        assert_eq!(format_time(&at(9, 0), &region), "9:00 de.");
        let one = format("d.M.yyyy", "h:mm t");
        assert_eq!(format_time(&at(18, 29), &one), "6:29 P");
    }

    #[test]
    fn quoted_text_is_literal_and_names_use_the_real_calendar() {
        let named = format("dddd, d MMMM yyyy", "H 'h' mm");
        assert_eq!(
            format_date_time(&at(18, 29), &named),
            "Thursday, 19 November 2026 18 h 29"
        );
        let short = format("ddd d MMM yy", "h:mm tt");
        assert_eq!(format_date(&at(18, 29), &short), "Thu 19 Nov 26");
    }

    #[test]
    fn weekday_is_right_across_a_leap_year_and_a_century() {
        assert_eq!(weekday(1970, 1, 1), 4); // Thursday
        assert_eq!(weekday(2000, 2, 29), 2); // Tuesday
        assert_eq!(weekday(2024, 3, 1), 5); // Friday
        assert_eq!(weekday(1900, 1, 1), 1); // Monday
    }

    #[test]
    fn the_fallback_is_unambiguous() {
        assert_eq!(
            format_date_time(&at(18, 29), &RegionalFormat::fallback()),
            "2026-11-19 18:29"
        );
    }

    /// The real OS read returns usable patterns on a Windows machine.
    #[cfg(windows)]
    #[test]
    fn windows_reports_a_short_date_and_time_pattern() {
        let os = RegionalFormat::os();
        let text = format_date_time(&at(18, 29), &os);
        assert!(text.contains("2026") || text.contains("26"), "{text}");
        assert!(!os.short_time.is_empty());
    }
}
