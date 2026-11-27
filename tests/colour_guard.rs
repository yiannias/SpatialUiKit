//! Kit colour guard (SDB TASK-384, 2026-11-26). Every colour the kit paints
//! comes from a theme token: `ThemePalette` (`theme.rs`), the interface
//! tokens (`interface_tokens.rs`), or the locked canvas tokens. A literal
//! `Color32` is allowed only where it *is* a token's built-in value, where it
//! is data (a caller-chosen colour, a picker), or where the file is on the
//! list of copies still to migrate.
//!
//! This is a ratchet, the same shape as SDB's `colour_guard`: each file may
//! keep exactly the literals listed here, and a new file gets none. A file
//! that drops below its number fails too, so the list is lowered in the same
//! commit that migrates a copy and never silently allows the copy back.
//!
//! Counted: `Color32::` constructors (`from_rgb`, `from_gray`, `from_rgba_*`,
//! `from_black_alpha`, `from_white_alpha`) and named constants (`WHITE`,
//! `BLACK`, `RED`, ...; not `PLACEHOLDER`, "the style's own text colour",
//! nor `TRANSPARENT`, "paint nothing"), outside `#[cfg(test)]` and comment
//! lines. Test-only files (`tests.rs`, `*/tests.rs`) are skipped.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Literal colours per file, and why they are there.
const ALLOWED: &[(&str, usize)] = &[
    // The tokens' own built-in values: these files *are* the theme.
    ("interface_tokens.rs", 46), // InterfaceColors::DARK + ERROR_ROSE, ERROR_ROSE_ON_LIGHT
    ("theme.rs", 36), // Dark/Light palettes, ok/warning colours, section_label light grey
    ("tokens.rs", 1), // hex parsing builds the colour
    // Light-theme values. Light is the platform look and has no interface
    // tokens yet (TASK-384 leaves that to the theming workshop).
    ("choice_toggle.rs", 8), // 7 light, 1 measuring
    ("settings.rs", 6),      // 4 light, 2 measuring
    ("sketch.rs", 6),        // 4 light checkbox, 1 measuring, 1 lighten-toward-white
    ("os_style.rs", 6), // platform hairlines (dark/light), contrast steps toward white/black, mixes
    // Derived or measuring, not a chosen colour: a colour computed from a
    // caller's (accent mixes, alpha), text measured for its width.
    ("command_status_panel.rs", 9), // 4 derived; 5 SSP-only defaults (cockpit caps, legacy frame border)
    ("dropdown.rs", 5),             // 4 measuring, opaque fallback
    ("font_picker.rs", 1),          // measuring
    ("menu.rs", 3),                 // MenuColor (caller-chosen) and measuring
    ("search.rs", 2),               // black/white ink picked for contrast on a caller colour
    // SSP's default ribbon style (SDB passes its palette's colours instead).
    ("ribbon.rs", 3),
];

const CONSTRUCTORS: &[&str] = &[
    "from_rgb(",
    "from_gray(",
    "from_rgba_unmultiplied(",
    "from_rgba_unmultiplied_const(",
    "from_rgba_premultiplied(",
    "from_black_alpha(",
    "from_white_alpha(",
];

const NAMED: &[&str] = &[
    "WHITE",
    "BLACK",
    "RED",
    "GREEN",
    "BLUE",
    "YELLOW",
    "GOLD",
    "GRAY",
    "DARK_GRAY",
    "LIGHT_GRAY",
    "LIGHT_RED",
    "LIGHT_GREEN",
    "LIGHT_BLUE",
    "LIGHT_YELLOW",
    "DARK_RED",
    "DARK_GREEN",
    "DARK_BLUE",
    "BROWN",
    "KHAKI",
    "ORANGE",
    "DEBUG_COLOR",
];

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Literal colours on one line.
fn literals_in(line: &str) -> usize {
    let mut n = 0;
    let mut rest = line;
    while let Some(i) = rest.find("Color32::") {
        rest = &rest[i + "Color32::".len()..];
        let ident: String = rest
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '(')
            .collect();
        if CONSTRUCTORS.iter().any(|c| ident.starts_with(c)) || NAMED.iter().any(|c| ident == *c) {
            n += 1;
        }
    }
    n
}

/// `text` without its `#[cfg(test)]` items: from each attribute through the
/// end of the item it marks (a braced block, or a `mod tests;` line). Only the
/// marked item is skipped, not the rest of the file after it.
fn without_test_items(text: &str) -> String {
    let mut out = String::new();
    let mut skipping = false;
    let mut depth = 0i32;
    let mut opened = false;
    for line in text.lines() {
        if !skipping && line.trim_start().starts_with("#[cfg(test)]") {
            skipping = true;
            depth = 0;
            opened = false;
            continue;
        }
        if skipping {
            for ch in line.chars() {
                match ch {
                    '{' => {
                        depth += 1;
                        opened = true;
                    }
                    '}' => depth -= 1,
                    _ => {}
                }
            }
            if (opened && depth <= 0) || (!opened && line.trim_end().ends_with(';')) {
                skipping = false;
            }
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

fn count(text: &str) -> usize {
    without_test_items(text)
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .map(literals_in)
        .sum()
}

#[test]
fn kit_files_do_not_add_literal_colours() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rust_files(&root, &mut files);
    let allowed: BTreeMap<&str, usize> = ALLOWED.iter().copied().collect();
    let mut problems = Vec::new();
    let mut seen = Vec::new();
    for path in files {
        let name = path
            .strip_prefix(&root)
            .unwrap()
            .to_string_lossy()
            .replace(std::path::MAIN_SEPARATOR, "/");
        if name == "tests.rs" || name.ends_with("/tests.rs") || name.contains("/tests/") {
            continue;
        }
        let n = count(&std::fs::read_to_string(&path).unwrap());
        let limit = allowed.get(name.as_str()).copied().unwrap_or(0);
        if n != limit {
            problems.push(format!("{name}: {n} literal colours, listed {limit}"));
        }
        seen.push(name);
    }
    for name in allowed.keys() {
        if !seen.iter().any(|s| s == name) {
            problems.push(format!("{name}: listed but no such file"));
        }
    }
    assert!(
        problems.is_empty(),
        "take colours from a theme token; if a copy was migrated, lower its \
         number in tests/colour_guard.rs (SDB TASK-384): {problems:#?}"
    );
}

#[test]
fn the_counter_sees_every_form() {
    assert_eq!(literals_in("let a = Color32::from_rgb(1, 2, 3);"), 1);
    assert_eq!(
        literals_in("egui::Color32::WHITE, Color32::from_white_alpha(9)"),
        2
    );
    assert_eq!(literals_in("Color32::TRANSPARENT"), 0);
    assert_eq!(
        literals_in("Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 9)"),
        1
    );
    assert_eq!(literals_in("fn f(c: Color32) -> Color32::Foo"), 0);
    assert_eq!(
        count("Color32::WHITE\n// Color32::BLACK\n#[cfg(test)]\nmod t;\nColor32::RED"),
        2
    );
    // Only the test item is skipped, not the code after it.
    let file = "#[cfg(test)]\nfn t() {\n    Color32::RED;\n}\nfn f() {\n    Color32::BLUE;\n}\n";
    assert_eq!(count(file), 1);
}
