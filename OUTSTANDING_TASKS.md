# AnimationCraft — Focused import and UI fixes

Status: **Revised after review; bounded implementation handoff**
Date: 2026-10-19
Replaces: the broad 33-task integration backlog in this file.

## Outcome and boundaries

Make the existing character import and artwork-review workflow dependable:
connect a character → add the correct sheets → inspect all poses or parts →
open a single item → correct an import problem → switch characters without
losing the connected artwork. CLI-driven operations must report the same outcome
as the app and must not hang silently.

This is a repair pass on the merged UI, not another redesign or a new animation
platform. Preserve **Artwork → Sheets / Poses / Parts**, full-sheet browsing,
pose selection that stays in the gallery, right-click **Open single pose**,
return navigation, synchronized selection, the existing grouped inspector cards,
and the restored expression sliders with value readouts and hover explanations.
Keep overlays off initially and one relevant View options menu per view.

Do not register extra export commands or add new Craft integrations in this pass.
A functional defect discovered outside this workflow is recorded separately,
not automatically added to the implementation scope.

## Evidence and ownership

The reimport review tested the code merged in `a376ded` using fresh output
folders, the standalone CLI, and a live app on port 19894. All 11 originals
imported and all 11 characters could be selected. Fourteen sheet manifests
contained 111 nonempty frames; that is file validation, not proof of correct
segmentation or usable animation.

Evidence: `exports/reimport-review-2026-10-19/review-report.txt`, with sibling
`live/` and `headless/` logs, screenshots, command scripts and artifact checks.
These are local review artifacts, not portable test fixtures. Embedded replay
paths still refer to the isolated `/tmp/animationcraft-reimport-*` workspaces.

Before coding, inspect current main and the owning agents' changes. The review
observed concurrent edits in:

- `apps/animationcraft-cli/src/main.rs`
- `crates/engine/src/commands.rs`
- `pipeline/guided_cutout_rig_builder.py`

Coordinate overlap before editing those files; do not duplicate ongoing fixes.
Use an isolated branch/worktree from current committed main. The original report
was committed as `8f28c8c`; its MCP description includes work that was uncommitted
when reviewed. Recheck claims against the implementation being changed.
Follow `AGENTS.md`; never stage, reset or replace another agent's working files.

## Ordered fixes

### F1. Make sheet imports work from a clean checkout

**Observed:** slicing fails when the ignored
`exports/templates/exploded_part_sheet_spec.json` is absent. The review only
continued after copying the existing template into its isolated workspaces.

**Change:** move/version that existing runtime template in an appropriate tracked
source location and update its readers. Do not add a new template system or
require users to copy files from another checkout.

**Acceptance:** run a small generated sheet through the real cutter with no
pre-existing exports. Assert the generated manifest and decoded, nonempty frame
images. The test must fail when the runtime dependency cannot be resolved.

### F2. Connect the right artwork to each character and sheet type

**Observed:** Rust presets point Sam to Rusty's poses, Maisy to Jenny's poses,
and Mr Puddles to Puddles' poses; Old Mac's pose mapping differs between catalogues.
The Python catalogue substitutes original images or illustrations for missing
sheets. Maisy's original is a presentation slide, and its pose entry repeats its
turnaround. Hayley's scene illustration is not a pose sheet. Generated Hayley and
Old Mac strips inspected in the review do not establish complete turnarounds.

**Change:** reconcile the existing mappings using visually verified sources.
Represent unavailable sheets as missing. Preserve useful slides/illustrations
as labeled references; do not silently treat them as poses or turnarounds.
Reuse existing catalogue/state structures; no new asset database or automatic
vault-wide matching service.

**Acceptance:** verify source identity for all 11 catalogue characters. Test
that switching characters preserves each one's sources, selected artwork and
loaded manifests. Include a regression that would catch cross-character reuse.
Missing sheets show a clear empty state, not fabricated frame counts.

### F3. Make incomplete slicing visible and provide a bounded correction path

**Observed:** Scout's pose import fails with “No foreground characters detected;
supply --layout.” Sam's numbered 12-pose sheet produces only six poses while
returning success. Several cutouts retain source labels or neighboring fragments.
The Python cutter accepts explicit layouts, but the engine command does not
forward that option.

**Change:** diagnose the existing detector/matting path with those sources first.
Repair the demonstrated causes without replacing the cutter. Where explicit
layout is needed, pass the existing layout-file option through `asset.cut_sheet`
and provide an **Apply layout…** action under existing Sheet tools. No interactive
mask/polygon editor or new segmentation service in this pass.

Show the actual slice count and `needs_review`/failure status beside the sheet.
Keep the source available for comparison. A failed re-cut must preserve the last
usable cutouts and selection; it must not imply the character is ready to animate.

**Acceptance:** Scout yields reviewable cutouts via the repaired automatic path
or the explicit-layout path. Sam's reviewed layout yields all 12 distinct source
poses. Compare actual crops with source bounds, including source-text and adjacent
figure contamination. Invalid layouts produce actionable errors and leave prior
usable output intact. Use generated fixtures for portable regression tests and
the real sheets for visual acceptance; do not add private vault files to Git.

### F4. Finish the existing sheet and pose navigation contract

**Observed:** new imports use `Pose 00` labels because the intake path forces OCR
off. The left-hand “View sheet” action selects a broad center tab rather than an
explicit sheet. This creates ambiguity now that Artwork contains several views.
The merged UI already has Sheets/Poses/Parts and additive reference attachment;
extend and verify these controls rather than building a second browser.

**Change:** make each View sheet action open the matching full sheet. Keep Add
sheet additive and Replace explicit. Preserve the originating sheet/gallery and
selection when opening and returning from a single item. Use reviewed manifest
labels when available; offer the existing OCR capability for re-cutting and keep
honest numbered labels when recognition is unavailable or uncertain. Do not guess
that an arbitrary frame is “Walking.” Do not introduce another naming service.

Retain the expression sliders, grouped cards, readable values and What/Why/How
hover help. Check the existing layout with all 11 character connections at normal
and narrow widths; fix clipping caused by this workflow without redesigning the
rest of the inspector.

**Acceptance:** an interaction test and live review cover add versus replace,
opening a specific sheet, gallery selection without an automatic view change,
context-menu single view, return to the originating gallery, and character
switching. The selected image, label and index agree. The expression sliders
remain usable and their readouts reflect the changed values.

### F5. Make the existing CLI bridge and failure reporting dependable

**Observed:** live commands paused when the app workspace was hidden and resumed
when visible. The standalone CLI continued normally. The bridge client has no
request timeout. MCP's successful-result path does not classify a returned
`ok:false` as a tool failure; Rust errors currently take a separate JSON-RPC path.

**Change:** reproduce and fix command servicing when the window is hidden, and
bound connection/response waits with clear diagnostics. Keep timeout handling
explicit: timing out must not claim the operation was cancelled, and must not
automatically retry a mutating import. A screenshot requiring a visible rendering
surface should fail clearly if it cannot be captured.

For existing commands only, preserve failed outcomes through CLI and MCP. Handle
both engine errors and successful transport responses containing `ok:false`.
Retain useful error details and input/output paths. Coordinate with the current
CLI owner before touching their MCP work.

**Acceptance:** a hidden-window state query completes without switching desktops;
a screenshot completes or returns a bounded, clear error. A stalled test server
cannot hang the CLI indefinitely. Missing source, invalid layout and failed cutter
are failures through CLI and MCP, not optimistic success messages. No duplicate
import is launched after a timeout.

### F6. Close the pass with honest, repeatable acceptance tests

This is validation of F1–F5, not a separate testing-framework project.

- Replace conditional-assertion false passes in the touched import/cutter paths
  with generated temporary fixtures that always exercise the behavior.
- Do not treat the topology parser's `loops == triangles * 3` self-check as
  independent parity evidence. Mesh parity itself is outside this pass.
- The runtime-template fix does not supply the three existing engine tests'
  separate sample-image dependencies. Give affected tests explicit generated
  fixtures or record the remaining inherited failures accurately; no silent skips.
- Run the relevant Rust/Python tests and build the app. Run the workspace suite
  for integration and distinguish unrelated baseline failures. Check formatting
  on changed code without a repository-wide formatting sweep.
- Repeat the 11-character source/import/switch review with fresh outputs. Record
  failures, actual pose/view counts and missing sheet types. Inspect images, not
  only JSON keys, file existence or exit codes.
- Validate portable paths in tests; distinguish real-sheet visual evidence from
  tests that can run on a clean checkout.

## Completion and handoff

Deliver one scoped PR, or small dependent PRs if ownership requires it. Each
handoff identifies its base and commit, changed files, test results and screenshots.
No blanket staging, force push or reset of the shared checkout.

The pass is complete when the existing import-to-artwork-review workflow meets
F1–F5's acceptance checks and F6 records the results. Missing source artwork may
remain missing when represented honestly; a failed slice or wrong character
mapping is not a completed feature. Existing artwork must remain accessible.
Do not claim this validates anatomy, rig quality, motion cycles or media exports.

## Explicitly deferred

The previous T1–T29 proposals are not an implementation queue for this pass,
except the narrowly scoped failure-reporting correction from T20:

- Extra Three.js/Remotion/Blender export registration and new export formats.
- New renderers, turntables, retiming, scene detection and costume swapping.
- New crop/trim/content-aware/boolean-mask wrappers or vector integrations.
- A new pivot editor, mask editor or asset database.
- Rebuilding the existing EffectCraft composition/keyframe generator.
- MCP resources/prompts, broad schema redesign, new inspect/probe/bench commands.
- Generated-output purge commands and repository-wide formatting.

`vision.analyze` already works through CLI and MCP, and live UI commands already
appear in CLI help. Those are not missing features. Keep broader ideas in a
separate future discussion; they must not expand this repair pass.
