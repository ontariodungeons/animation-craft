# Pose and Part Cutter Implementation Plan

> **Historical design reference — not the active implementation plan.**
> The current application is Rust/egui; see [README](../../../README.md) and the current user assignment.
> Statements below about inline/shared-checkout work, untracked baselines, old ports,
> tool versions, completion status, or required new features are historical, not instructions.


> Superseded workflow: the baseline is now tracked. Use an isolated worktree per AGENTS.md.

**Goal:** Review Track B and export the actual vault pose/turnaround sheets as transparent sprites, atlases, and honest, editable articulated cutouts.
**Architecture:** Offline Pillow/NumPy/SciPy connected-component segmentation with optional local Tesseract label recognition; explicit JSON layout/mask overrides for ambiguous sheets. Never invent hidden limb pixels or certify inferred masks. Keep existing Blender pipeline unchanged; add an independent CLI and unified CLI entrypoint.
**Spec:** `docs/superpowers/specs/2026-10-18-modular-2d-3d-blender-filmcraft-pipeline-design.md` plus the user's cutter requirements.

## Constraints and review focus
- Read source vault only; all outputs stay in workspace. No external generation or API calls.
- Preserve existing alpha and pale enclosed details; remove captions and margins without clipping source artwork.
- Actual sheets vary in pose count, row count, and order. Labels must come from OCR/explicit layout, never assumed action order.
- Expose source coordinates, alignment, pivots, UVs, missing actions, occlusion limitations and per-part review status.
- Do not claim a static pose selection is a temporally continuous walk/run cycle.

## Tasks
- [x] Write failing synthetic tests for transparency, two rows, five views, explicit polygon masks/pivots, missing/blank sources and exploded sheets.
- [x] Implement `cut_sheet(source, out_dir, kind='auto', layout=None, use_ocr=True)` and directory batch CLI, prompt export and atlas/runtime helpers.
- [x] Add `cut-sheet` unified CLI delegation; write review with file/line evidence and usage documentation.
- [x] Run Python and Bun suites; process every real action and turnaround source; inspect contact sheets and record rejected/ambiguous cuts.

## Execution record
- Initial inspection: 26 action sheets and 32 turnaround sheets; action sheets include 10, 11 and 12 frames, plus a 13-image sheet containing a hero reference, with nonuniform layouts. Hopper canonical sheet is 11 poses, not 10.

- Complete: 58 sheets, 458 frames, 3,175 visible-pixel part cutouts; zero batch errors. 55 frames touch crop boundaries and remain explicitly flagged for review.
- Independent review found low-opacity alpha loss and opaque-crop misclassification; fixed both under a failing regression test.
- Final verification: 14 Python tests, 5 Bun tests pass; runtime ES module bundles; all referenced PNG dimensions/nonempty alpha and part paths validated.
- Existing Blender/FilmCraft implementation was reviewed, not executed or refactored. Findings remain open in docs/reviews/2026-10-18-image-to-animation-pipeline-review.md.
