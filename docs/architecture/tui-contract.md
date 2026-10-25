# Shared UI / CLI / TUI contract

Implementation branch: `fix/panel-unification`, base `5ab274ba`.
Worktree: `.worktrees/panel-unification` inside the Magic Studio repository.
Foundation commit: `775c6a02`; latest backend checkpoint `621b9600`. This document describes the source contract; completion evidence belongs in implementation-plan.md.

## Configuration and state

`apps/magicstudio-shell/ui/components.json` is the static component configuration.
Entries define component, look, density, label, optional `indicator` (`icon` or `hidden`), canonical state `target`, and attention `priority`.
`signals` defines icons/labels; idle is 💤. `rails` and `slots` hold rail policy.
Hidden indicators still register and report state. An alias uses the target component's state.

The shared registry is `window.uiRegistry`; `window.companionBoxes` is a compatibility name for the same object.
Command observation belongs to this registry. The pet controller consumes shared records and must not own command/state interpretation.

## Discover every component

Use the explicitly selected Studio endpoint:

```sh
mc --bridge PORT exec ui.controls limit=64
mc --bridge PORT exec ui.controls offset=64 expected_instance=INSTANCE expected_revision=REVISION
mc --bridge PORT panels --json
mc --bridge PORT panels --check
```

`ui.controls` includes containers and noninteractive components, not just buttons.
The response contains `schema_version`, `instance`, `revision`, `project`, `available`, `total`, `offset`, `items`, and `next_offset`.
Continue until `next_offset` is null. A changed instance/revision fails explicitly; restart pagination.

Every item includes:

| Field | Meaning |
|---|---|
| `index` | Numeric DOM-instance identity, never reused in that page instance |
| `id` | Authored stable DOM ID, or null |
| `target` | Authored ID or `@INDEX` for CLI interaction |
| `parent_index`, `parent_id` | Containing DOM component |
| `tag`, `component`, `config`, `container`, `rail` | DOM type, shared component, config entry, canonical state owner and configured rail |
| `rect` | Actual CSS-pixel x, y, width, height in the viewport |
| `visible`, `enabled`, `selected` | Observed control state |
| `label`, `text` | Accessible label and bounded direct text; private chat contents excluded |
| `data` | Checked state, numeric/range bounds/value, or select value/options; passwords/text drafts excluded |
| `style` | Observed display, visibility, colour, background, font, overflow and position |
| `commands` | Explicitly annotated engine commands; empty means no verified annotation |
| `actions` | Generic CLI actions actually supported by the element |

Do not treat `commands: []` as a verified engine mapping. Generic `ui.click`/`ui.set`/`ui.key` routes remain discoverable separately.
Do not reuse an index after instance replacement. `@INDEX` actions require `instance=INSTANCE`:

```sh
mc --bridge PORT exec ui.click id=@42 instance=INSTANCE
mc --bridge PORT exec ui.set id=@43 instance=INSTANCE value=true
mc --bridge PORT exec ui.key id=@44 instance=INSTANCE key=ArrowRight
```

Hidden/disabled click targets refuse. Toggles accept Boolean values. Text/password drafts are not echoed by `ui.set`.
`ui.components` uses the same observation implementation, with `components` as a compatibility alias for `items`.

## Shared live state

`ui.feedback.panels` contains `revision`, `available`, `items`, `unconfigured`, totals and `truncated`.
Each item carries canonical ID, label/look, signal, busy, reason/source/request and observation/receipt metadata.
Signals: 0 idle, 1 active/ready, 2 attention, 3 error. Busy distinguishes working from ready.
The UI indicator and pets read the same record. No separate TUI colour/state inference.
Missing/stale feedback means unavailable. Truncated observations cannot pass `panels --check` or claim universal health.
`mc panels --json` includes the disk config, typed panel report and original feedback; offline static configuration remains inspectable.

## TUI requests UI attention

```sh
mc --bridge PORT exec ui.attention project=/absolute/Song.magic id=COMPONENT reason='Review this result'
mc --bridge PORT exec ui.attention project=/absolute/Song.magic id=COMPONENT clear=true
```

The exact open project and registered target are required. Attention is separate from actual engine error state; clearing attention cannot clear an error. Dog/cat movement is a presentation of the resulting shared record, not a TUI-owned animation command.

## Handoff boundary

The assigned TUI agent owns terminal presentation and consumption of these records. Codex owns UI registration, shared config, CLI/engine transport and pet/rail behavior. Send the agent the actual commit SHA and evidence before integration; this file alone is not proof that every implementation gate passed.

## Shared panel construction

A `panel` config entry names its `rail`. The common constructor mounts exactly one slot in its shared header. Rail policy and slot identity come from the same config; coordinates are measured from that slot. Missing or invisible destinations are unavailable, never redirected to a different panel. The registry publishes state changes through the existing feedback transport even when companions are disabled.

## Activated companions

The original SVG dog and cat are selected by `companions.manifest.json`; all ten pose files decode in the live shell. Companions default on, with explicit user opt-out retained. `panels.items[].attention` exposes an explicit attention request separately from the underlying engine state. Errors outrank attention; explicit TUI attention outranks routine readiness at the same signal level. Clearing attention cannot clear an engine error. Completed activity expires, and idle pets return to their configured homes. Reduced motion and disabling cancel pending pose timers.

Live acceptance: `docs/architecture/evidence/live-pets-results.json` exercises actual CLI attention, an actual rejected engine command, successful command recovery, a deliberately unregistered fixture control, disable/re-enable and controlled idle homes. `pet-layout-results.json` proves both decoded images fit their configured slots at 700, 941 and 1261 pixels. The controlled idle fixture does not claim that project intake prerequisites are complete.
