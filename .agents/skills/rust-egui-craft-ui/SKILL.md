---
name: rust-egui-craft-ui
description: Use when building, redesigning, or polishing native Rust + egui desktop UIs in the Craft suite (Magic Studio, FilmCraft, PhotoCraft, EffectCraft). Covers Craft design tokens, custom pro-app widgets (scrubbable hot-numbers, collapsible section headers, compact pill/icon toggles, vector icons), mode-based top headers, 4px-gap panel cards, background job orchestration, and live --control visual verification.
---

# Rust `egui` Craft-Suite UI Skill (`rust-egui-craft-ui`)

Use this skill whenever designing, refactoring, or fixing UI in `crates/ui-egui` for `Magic Studio` (or any sibling Craft app). Default `egui` widgets (`ui.button`, `ui.slider`, `ui.horizontal_wrapped` with emoji labels) look like debug overlays and quickly turn into unorganized button soup. Pro creative tools in `egui` require a disciplined token system, custom painter-drawn controls, and mode-scoped layout hierarchy.

---

## 1. Why Default `egui` Looks Rough (and How Craft Fixes It)

| Anti-Pattern (Raw `egui`) | Craft-Suite Pattern ([`filmcraft-codex/crates/ui-egui`](file:///home/jesse/Work/filmcraft-codex/crates/ui-egui/src/lib.rs)) |
|---|---|
| 50 buttons jammed into `ui.horizontal_wrapped` sidebars | **Top-level Workflow Mode Tabs** (`Intake & Slice`, `2D Puppet & Rig`, `3D & Export`) in a `38pt` header bar so each mode shows only relevant panels and inspectors |
| Wide `egui::Slider` bars occupying every row | **Scrubbable Hot-Text Numbers (`hot_number`)** — compact blue (`#4b9cf5`) numeric readouts that scrub horizontally on drag or open inline text edit on click |
| Emoji strings (`🎨`, `🧊`, `🧍`, `🦘`) that render inconsistently | **Vector-Painted Icons (`icons.rs`)** using `egui::Painter` strokes (`line_segment`, `circle_stroke`, `rect_stroke`, `convex_polygon`) |
| Flat grey panels bleeding together | **`4px` Pure-Black Gaps (`#000000`)** separating `#1d1d1d` rounded (`4px`) panel cards with `28pt` tab bars and `#378ef0` focus borders |
| Exposed internal state names (`part_idx`, `verified=true`) | **Human-Readable Inspector Rows** (`20pt` row height, `76pt` left-aligned muted label `#9b9b9b`, right-aligned control) |
| Blocking the UI thread on heavy image/mesh ops | **Background `std::sync::mpsc` worker threads** with non-blocking `try_recv()` in `eframe::App::update` and status progress bar |

---

## 2. Design Tokens (`theme.rs`)

Always drive colors, spacing, and typography from a single `Tokens` struct stored in `egui::Context` temp data (`ctx.data_mut(|d| d.insert_temp(Id::new("craft_tokens"), tokens))`), never hardcoded `Color32::from_rgb(...)` scattered across panel files:

```rust
#[derive(Clone, Copy, Debug)]
pub struct Tokens {
    pub app_bg: Color32,        // #000000 — 4px gaps between panels
    pub panel_bg: Color32,      // #1d1d1d — main panel surface
    pub panel_bg_deep: Color32, // #141414 — canvas / timeline well
    pub header_bg: Color32,     // #1d1d1d — 38pt top bar & 28pt tab bars
    pub border: Color32,        // #0a0a0a — subtle panel edge
    pub divider: Color32,       // #292929 — 1px section divider
    pub text_hi: Color32,       // #e6e6e6 — primary values & active labels
    pub text_mid: Color32,      // #9b9b9b — property labels & inactive tabs
    pub text_lo: Color32,       // #636363 — disabled / hint text
    pub field_bg: Color32,      // #0e0e0e — input well background
    pub field_border: Color32,  // #303030 — input well border
    pub focus: Color32,         // #378ef0 — active tab underline & panel border
    pub hot_text: Color32,      // #4b9cf5 — scrubbable numeric value color
    pub accent_ok: Color32,     // #2db870 — verified / ready badge
    pub accent_warn: Color32,   // #e89b2d — warning / needs review badge
    pub radius: f32,            // 4.0
    pub gap: f32,               // 4.0
    pub header_h: f32,          // 38.0
    pub tab_h: f32,             // 28.0
    pub row_h: f32,             // 20.0
}
```

### Typography Hierarchy
- **UI Body (`Proportional`, 12.0pt)**: Panel tabs, property labels, buttons.
- **Section Headers (`Proportional`, 11.5pt Semibold)**: Collapsible inspector groups.
- **Numeric / Timecode / Topology Badges (`Monospace`, 11.0pt)**: Hot numbers, `X/Y/Rot/Z`, vertex counts, FPS.
- **Micro Labels (`Proportional`, 10.0pt)**: Status strip, shortcut badges (`WASD`, `Space`).

---

## 3. Core Custom Widgets (`widgets.rs`)

Build and use these 5 core primitives instead of raw `egui` defaults:

1. **`section_header(ui, id, title, default_open) -> bool`**
   - Full-width `22pt` clickable bar with painted chevron (right-pointing when closed, down-pointing when open) and `1px` bottom divider.
   - Persists open/closed state in `ui.ctx().data_mut`.
2. **`prop_row(ui, label, add_control)`**
   - Fixed `20pt` row with a `78pt` muted label (`t.text_mid`) on the left and the control filling the remaining width on the right.
   - Prevents ragged, misaligned sliders and buttons.
3. **`hot_number(ui, id, val, range, speed, decimals, suffix) -> bool`**
   - Paints the formatted value + suffix in `t.hot_text` (`#4b9cf5`) with a subtle dashed underline on hover (`CursorIcon::ResizeHorizontal`).
   - Horizontal drag scrubs `val` by `drag_delta.x * speed` (`Shift` = 10x, `Alt` = 0.1x).
   - Single click switches `id` into inline `egui::TextEdit::singleline` mode; `Enter` or focus loss commits.
4. **`segmented_tabs(ui, id, options, current)`**
   - Compact pill bar in a `#0e0e0e` well with `#378ef0` active highlight for switching `2D Rig` vs `3D Preview`, or `Front / Right / Back / Left` views.
5. **`status_pill(ui, label, tone)`**
   - Compact rounded badge (`accent_ok`, `accent_warn`, `focus`) for showing `12/12 Parts Verified`, `OCR Ready`, or `GLB Exported`.

---

## 4. Layout & Workflow Architecture

Structure `Magic Studio` around **3 clear workflow modes** in the top `38pt` header bar rather than showing every tool at once:

1. **Top Header Bar (`38pt`)**:
   - **Left**: App brand mark + active Character selector dropdown (`Hopper`, `Whiskers`, `Miss Hayley`, `Old Mac`, `Puddles`, `Rusty`, etc.) + `Import Image...`.
   - **Center**: Workflow Mode Tabs with `2px` `#378ef0` active underline:
     - **`1. Cut & Verify`** (Sprite sheet auto-slice, BG tolerance, Local OCR auto-label, 2D cutout grid & part verification)
     - **`2. 2D Rig & Animate`** (Part hierarchy tree, interactive pivot/depth/transform inspector, 6-action clip preview, timeline & keyframe curves)
     - **`3. 3D Turnaround & Export`** (4-direction N/E/S/W registration, visual-hull inflation & relief sliders, live 3D orbit viewport, Blender `.glb`/`.obj` & FilmCraft `.fcproj` export)
   - **Right**: Status pill + Undo/Redo + Quick Export CTA.
2. **4-Panel Workspace with `4px` `#000000` Gaps**:
   - **Left Dock (`240pt`)**: Context-sensitive to active mode (Cast & Cutout Parts list in Mode 1/2; 4-Direction Turnaround Slots in Mode 3).
   - **Center Stage (Expanding)**: Interactive 2D/3D canvas with compact floating HUD overlay (Zoom, Grid, Onion Skin, Pivot Handles, WASD Roam toggle) and bottom contextual action bar.
   - **Right Inspector (`280pt`)**: Clean `section_header` + `prop_row` + `hot_number` property sections (never wrapped button walls).
   - **Bottom Dock (`190pt`, collapsible)**: Timeline, Action Clip switcher (`Idle`, `Walk`, `Jump`, `Wave`, `Talk`, `Celebration`), and transport controls (`Play`, `Loop`, `FPS`, frame scrubber).

---

## 5. Engineering & Verification Rules

1. **Zero Unwraps in Production (`crates/ui-egui` & `crates/engine`)**:
   - Keep `#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable))]` intact.
2. **Automation Registry (`AutomationRegistry`)**:
   - Every interactive widget must remain addressable via the TCP `--control` bridge (`ui.state`, `ui.click`, `ui.set`) or engine CLI commands so automated tests and agents can drive every workflow deterministically.
3. **Visual Verification Loop (Mandatory after every UI edit)**:
   - Build the canonical app (`CARGO_BUILD_JOBS=1 cargo build -p magicstudio-shell -p mc`) and verify project/UI state through `mc` before claiming any UI change is complete.
   - Check specifically for: text truncation, cramped row spacing, overlapping labels, low contrast, and cognitive overload.
