---
name: animationcraft-shell-ui
description: Use before building or changing any screen of the AnimationCraft Tauri shell (apps/animationcraft-shell/ui). The shell's design system — calm dark surfaces, one teal accent, icons over words with tooltips, cards that lift on hover, expanding blocks, one primary action per place — with its tokens, components, motion, layout and the live verify loop.
---

# AnimationCraft shell UI

The shell is a creative tool for one person working with an agent. Every screen answers: **what is this,
what's its state, what's the one next step.** Everything else waits behind a hover, a tooltip or a click.
Source of truth: `apps/animationcraft-shell/ui/style.css` (tokens on `:root`). Reuse its classes; add a
token before you add a color.

## Principles

1. **Focused.** Each view has one job. Home picks a project, Project plans the song, Animation animates,
   Studio assembles. Don't show tools from another view's job.
2. **Icons over words.** Buttons are 18px stroke icons (`.icon`, 30×30, radius 8). The words go in `title`
   (tooltip) and `aria-label`. Keep a visible label only on the one primary action (`.start`) and on the
   names of things (projects, objects, scenes).
3. **One primary action per place.** It's the only teal-filled button (`.start`). Secondary actions are
   `.ghost` (outlined) or `.icon`. Never two teal buttons side by side.
4. **State is a light, not a sentence.** `.dot` is `.ok` (teal), `.warn` (amber), `.off` (red, with a
   halo) or `.busy` (pulsing amber). The longer explanation goes in the tooltip.
5. **Progressive disclosure.** Blocks expand (`.block.open`, a grid-rows 0fr→1fr animation). Details,
   destructive actions and "remove" crosses appear on hover (`.proj .forget`). Meta text hides under
   1000 px and moves to tooltips.
6. **Calm when empty.** An empty view is a centered card: one short line, one muted line, one `.start`
   (see `.studio-empty`, `.home-empty`). Never a broken image, raw error or blank panel.
7. **Errors are short and human.** Use `toast()` for one line about something you just did; anything
   that persists belongs inline on the card it concerns. Never toast on load.
8. **Setup stays out of the way.** Servers don't run until the user flips the switch; long jobs show
   a busy dot, not a modal.

## Tokens (`:root`)

| token | value | use |
|---|---|---|
| `--bg` | `#0d0f13` | window, wells |
| `--surface` | `#151820` | cards, blocks, chips |
| `--surface-2` | `#1b1f29` | hover, inner cards, inputs on a surface |
| `--line` | `#262b36` | 1px borders and dividers (open/active: `#34404f`) |
| `--text` / `--muted` / `--faint` | `#e8ecf3` / `#8e97a8` / `#5d6575` | names / labels / meta |
| `--accent` / `--accent-ink` | `#64d7c2` / `#0c2a25` | the one accent; text on accent |
| `--amber` | `#f0a83c` | needs attention, questions |
| `--rust` | `#d0644a` | missing or broken |
| `--radius` | `10px` | cards (pills 999px, icons 8px, sheets 14px) |
| `--ease` | `cubic-bezier(.2,.8,.2,1)` | every transition |
| `--fast` / `--slow` | `160ms` / `320ms` | hover and color / layout and enter |

Type: system-ui 13px/1.45. Titles are 600 weight. Section labels use `.group-label` (11px uppercase,
.06em tracking, faint). Numbers and paths use `ui-monospace` at 11px.

## Components (reuse these)

- **Top bar** (`.bar`, 48px): brand (click = Home) · tabs with a sliding `.tab-ink` underline · right side:
  agent toggle, studio switch (`.target` pill with a `.dot`). In Home (`body.at-home`) the tabs and the
  switch are hidden.
- **Block** (`.block`): an icon, a status dot, a title, faint meta and a chevron that rotates 90°; its body
  expands. Used for workflow stages.
- **Card** (`.proj`, `.object`): thumbnail on top (`#0b0d11` well, `object-fit: contain` for art,
  `cover` top for pages), name, faint meta. Hover: `translateY(-2px…-3px)`, teal border, soft shadow.
- **Chip** (`.chip`): pill navigation in a row that scrolls horizontally and fades out at the right
  (`mask-image`). The active chip is teal-filled.
- **Sheet** (`.sheet-backdrop` + `.sheet`): a modal for short forms, with a blurred backdrop and the sheet
  rising 8px. Labels sit above inputs. "Choose…" pickers use the system dialog (`tauri-plugin-dialog`).
  Actions sit at the bottom right: Cancel (ghost), then the primary.
- **Drawer** (`.agent`): slides in from the right over any view (`translateX(102%)` → 0) with a blurred
  surface.
- **Toast** (`.toast`): bottom center, one line, 2.4 s.
- **Overlay** (`.point-overlay`): dims the app with `#0005` and rings the target with a teal 2px border
  and a 4px glow.
- Icons are inline SVG with `viewBox 0 0 24 24`, stroke `currentColor`, width 1.8 and round caps; keep a
  small set in a `const ICONS` map. Use DesignCraft for an icon you can't draw cleanly.

## Motion

Things enter with `fade` (opacity plus 4px rise, `--slow`); hover uses `--fast`. Movement is small (2–3px
lifts, a 90° chevron); nothing bounces. Respect `prefers-reduced-motion` (already global).

## Layout

- Views are `.view` sections that switch with `.active`. Project is a left column of blocks
  (`clamp(260px, 30vw, 380px)`) plus a flexible main area. Home is a centered column up to 980px with a
  card grid (`auto-fill, minmax(190px, 1fr)`).
- Layouts must work from narrow tiled windows (~700px) to full screen. Test both.
- Big previews (storyboard pages, the stage) sit on a soft radial gradient well with a deep shadow.

## Code conventions

- One file per view or feature in `ui/` (`app.js`, `home.js`, `agent.js`, `bridge.js`). Use the shared `$`,
  `el()`, `toast()` and `invoke`. No frameworks or build step. Vendor a library (exact version) only when
  writing it yourself would be hand-rolling, e.g. markdown rendering.
- Data comes from the engine through Tauri commands in `src/main.rs`, on the window's one `Engine`
  session. The page never computes what the engine knows (motion, layout, status).
- Every interactive element has a DOM `id` or an `aria-label`, because that's how the bridge, the CLI and
  agents find it.

## Verify loop (every visible change)

```
cargo build -p animationcraft-shell            # ui/ is embedded: rebuild after edits
./target/debug/animationcraft-shell --control 19897 [<project.acproj>]
animationcraft-cli --bridge 19897 exec ui.look [region=<id|css>] [click=<id|label>] output=<project>/.agent/look.png
```
Open the PNG and check, every time: alignment, truncation, contrast, empty states, hover-only controls
still reachable, the narrow window. For layout bugs, `ui.eval js='return …getBoundingClientRect()…'`
beats guessing. (Example: a `.home` class matched both the Home column and `body.home`; renamed to
`body.at-home`.)
