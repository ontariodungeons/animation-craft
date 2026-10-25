---
name: magicstudio-shell-ui
description: Use before building or changing any screen of the Magic Studio Tauri shell (apps/magicstudio-shell/ui). The shell's design system — calm dark surfaces, teal primary actions with restrained functional section accents, icons over words with tooltips, cards that lift on hover, expanding blocks, one primary action per place — with its tokens, components, motion, layout and the live verify loop.
---

# Magic Studio shell UI

The shell is a creative tool for one person working with an agent. Every screen answers: **what is this,
what's its state, what's the one next step.** Everything else waits behind a hover, a tooltip or a click.
Source of truth: `apps/magicstudio-shell/ui/style.css` (tokens on `:root`). Reuse its classes; add a
token before you add a color.

## Research before a new screen

Read [research.md](research.md): official tool references, the proposed Animation frame, the ordered
agent integration work and the 76-finding prevention ledger. Distinguish proposals from approved
requirements. Present a new screen's layout and behavior and wait for user approval before building.
The Animation layout was approved on 2026-10-21. The user has specified controls on the **left** and a
useful main canvas immediately after import. Progressive disclosure preserves features, not removes them.

The app's permanent agent instructions are `apps/magicstudio-shell/agent.md`. Keep that document
focused on the current app's design, technical ownership, tools and verification.

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
9. **Selection is shared.** Stage, inspector, layers, timeline and agent use the same stable IDs.
   Import selects and shows the artwork, fit to content, or shows a concrete preparation/recovery
   action alongside it. Never display a selected object and “no active character” simultaneously.
10. **States are explicit.** Rest pose, track time zero and expression swaps are different concepts.
    The engine owns them. No anatomy/filename guesses; missing state metadata needs a review step.
    Until resolved, show original/reference artwork plus that step; if unavailable, show a state-choice
    card before assembling alternatives. Layer stacking, parent hierarchy and song order stay distinct.
11. **Collaboration is core.** The agent acts through the same live commands as the user, with visible
    targets, an action trail, readback and undo. A command echo is not undo or proof of an effect.

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
- **Assistant drawer** (`.workshop-drawers`): Sproket and Gaffer open below their top-bar buttons.
  One shared controller preserves both contents and drafts; one drawer opens at a time. The main
  surface keeps its full width. Part artwork stays in the left Animation inspector.
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
- Test 700px, 941px and 1261px widths with the agent both open and closed. Collapse/disclose the left
  inspector before crushing the stage. On narrow windows show one overlay at a time, keep transport
  reachable, and fit artwork into the unobscured area without changing its authored position.
- Reserve trailing action columns, use `min-width: 0` on names and bound each scroll area. Hidden,
  clipped or occluded controls must not masquerade as visible bridge targets. View changes preserve
  the outer window size. Every relevant row of the research ledger is a release gate.

## Code conventions

- One file per view or feature in `ui/` (`app.js`, `home.js`, `agent.js`, `bridge.js`). Use the shared `$`,
  `el()`, `toast()` and `invoke`. No frameworks or build step. Vendor a library (exact version) only when
  writing it yourself would be hand-rolling, e.g. markdown rendering.
- Data comes from the engine through Tauri commands in `src/main.rs`, on the window's one `Engine`
  session. The page never computes what the engine knows (motion, layout, status).
- Every interactive element has a DOM `id` or an `aria-label`, because that's how the bridge, the CLI and
  agents find it.
- Preserve DOM identity during polling: unchanged transcript rows, model options and question inputs
  remain in place. Animate new entries once. Preserve scrolling, selections and drafts while streaming.
- Preserve composer text and attachments on send failure and retain edits typed during delivery.
  Enter sends, Shift+Enter adds a line, Ctrl+Enter also sends; IME composition never submits.
  Coordinate coding agents through their session API, never by overwriting the user's composer.

## Agent and engine contract

Follow the implementation order in research.md: persistent live MCP → visible actions/history/undo →
two-way pointing/drawings/comments → captured message context → inline questions/progress → vendored
Markdown. Use `mc --bridge <selected-endpoint> mcp` for the verified Studio window; never bind
it to a song path or guess a port. Discover its project/instance and endpoint with `mc exec ui.feedback`.
Persistent config edits require user authorization.

Capture view, selection, scene, track/instance, song/local time, range and revision when sending a
message. Show the captured context and retain it throughout the job. Ask inline when “later” is
ambiguous; don't silently use a changed selection. Notes use frame coordinates/time and stay out of
exports. Agent highlights never take the real pointer or keyboard focus. Permission/question cards
retain input across refreshes. Jobs report progress/cancellation without blocking the page.

The engine owns the Rust motion evaluator; no additional evaluator.
Compare samples to `pipeline/layered_motion.py`, the export truth. JS only draws engine samples.
Pose controls are capability-driven, with units/limits and an explicit distinction between audition
and writing keys. Unknown measurements stay unknown. Rig/matting remain CLI chains; ask the user
about the ❓ intake, pose-sheet and 3D items instead of choosing their UI form.

When a workflow requires scripts or repeated tool calls, make its reusable capability a typed engine/
CLI/MCP command, verify it, document a one-line example and use it from the UI. Keep commands generic.
Upgrade only touched dependencies to latest stable releases and record versions; Tauri remains 2.x.

## Verify loop (every visible change)

```
cargo build -p magicstudio-shell -p mc          # ui/ is embedded: rebuild after edits
./target/debug/magicstudio-shell --control <port> [<project.magic>]
mc --bridge <selected-endpoint> exec ui.look [region=<id|css>] [click=<id|label>] output=<project>/.agent/look.png
```
Open the PNG and check, every time: alignment, truncation, contrast, empty states, hover-only controls
still reachable, the narrow window. For layout bugs, `ui.eval js='return …getBoundingClientRect()…'`
beats guessing. (Example: a `.home` class matched both the Home column and `body.home`; renamed to
`body.at-home`.)

For agent features, also ask the inner agent to perform a real task, observe it through `ui.look`,
and verify state and undo. Check stale-context behavior, failed sends, typing during polling, and
in-progress question answers. Do not claim the six-step agent contract is implemented merely because
MCP connects. With an open project, `ui.look` defaults to `<project>/.agent/looks/`; open the actual PNG.

## Approved visual refinement (2026-10-21)

The user likes the shell overall and explicitly asks to retain the useful colored-slider/section
separation concept. Keep layout and density; refine rather than redesign. Teal remains the primary
action accent. Inspector categories may use motion blue `--motion-blue` (#70c4ff), pose purple
`--pose-purple` (#c3a4ff), path/depth gold `--depth-gold` (#f0be70), check green `--check-green`
(#80da9a), and neutral layers. Pair color with names and SVG icons; it never substitutes for status.
Use a faint border tint, a colored small icon/title, a quiet value badge with units and an outlined
slider handle. Avoid large colored panels. Sliders replace spinners and keep keyboard/touch control.
This user-approved category coding supersedes the earlier restriction to one heading color.
Craft processing runs headlessly by default; show previews/results inside this app.

## Native Studio and project transitions

User removed the Split tab: top-level views are Project, Animation, Studio. Studio fills the content
area below the 48px bar; an open drawer moves Studio below its measured height. On Linux use the shared GTK
allocation, not vertically packed webviews. `ui.look` must include Studio's native webview as well as
the shell, otherwise an agent sees a blank canvas while the user sees Studio. Verify with the drawer
open and closed. Clear storyboard image sources and animation selection when switching projects.
