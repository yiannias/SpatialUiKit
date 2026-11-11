---
id: TASK-1.2
title: Card translucency and blur support (phase 2, kit side)
status: To Do
assignee: []
created_date: '2026-11-11T05:40:00Z'
updated_date: '2026-11-11T05:40:00Z'
labels:
  - kit
  - interface
dependencies:
  - TASK-1.1
parent_task_id: TASK-1
priority: medium
type: feature
ordinal: 3000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Backdrop blur with the option-2 fallback and the 'Blur unavailable on this display path; translucency only' note. Tests from spec section 8: renders at 0/15/100%, checkerboard blur variance, fallback without panic, perf budget. Settings ui_card_translucency / ui_card_blur live in SDB; the kit exposes the knobs.
<!-- SECTION:DESCRIPTION:END -->
