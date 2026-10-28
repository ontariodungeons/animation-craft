---
name: image-to-animation
description: Use when carrying out image-to-animation work in Magic Studio with user-supplied artwork, pose sheets, turnaround views or part sheets.
---

# Image-to-animation

## Establish the requested result

Use the user's current request and confirmed decisions as the requirements. Identify the supplied artwork and requested output. Ask when an unresolved choice changes that output; do not assume a character, art style, anatomy, number of views, action sequence, export target or approval checkpoint.

Inspect the actual images and any accompanying manifests. Treat labels, poses and view angles as evidence to verify, not facts to infer from frame numbers. Preserve missing information as missing.

Read what is already registered in the project and already on disk before proposing steps, and confirm the character roster with the user rather than taking one from a preset list or a previous task.

## Check the current tools

Work from the canonical checkout. Inspect the installed entry points and their current help before choosing commands. Verify capabilities by their behavior and produced artifacts; registration in a command catalogue or a successful build alone does not prove a feature works.

Several entry points cover this work and their documentation does not agree: the `mc` chain (`intake.validate` → `asset.cut_sheet` → `assets.assess` → `asset.build_cutout_rig`), `image.generate rig_parts=` followed by `rig.from_sheet`, and `studio_pipeline_cli.py`. Establish which one the current project already uses, say which you chose and why, and do not mix stages across them in one run.

For cutout extraction and registration, inspect:

```bash
python3 studio_pipeline_cli.py cut-sheet --help
python3 studio_pipeline_cli.py optimize-accuracy --help
```

Use only the operations needed for the requested result. Inspect other available tools when the task needs different functionality. If a required operation fails or is unavailable, report the concrete gap and ask about alternatives that would change the result. Do not invent missing implementation or silently substitute another deliverable.

## Process the supplied assets

Use actual source paths, explicit sheet kinds and output locations appropriate to the task. The currently verified command forms are:

```bash
python3 studio_pipeline_cli.py cut-sheet "$source_image" \
  --kind "$sheet_kind" --out-dir "$cutout_dir"

python3 studio_pipeline_cli.py optimize-accuracy \
  --turnaround-dir "$turnaround_dir" \
  --poses-dir "$poses_dir" --out-dir "$registered_dir"
```

Set these variables from the selected inputs and outputs before execution. Run the optimizer only when registration or motion measurement is requested and its current required inputs exist. Read the command summaries and actual manifests for frame paths, labels, matting engine, missing inputs and review status. Choose OCR, explicit layouts and refinement settings from the source images and available tools rather than assuming a fixed layout or installed provider.

Check exit status and inspect the produced files. Review cutout outlines and any requested part masks against the source. Correct observed errors with the supported layout/mask controls and rerun the affected stage. Preserve source artwork; identify output revisions clearly.

## Verify and hand over

Check the current output schema and actual artifacts against the requested result. For the current optimizer, check `rasterRegistrationPassed`, `maxVerticalDriftPx`, the registered images and `motion_range.json`; report missing poses and unavailable values. A result at one alpha threshold establishes that measured image alignment only, not anatomical correspondence or seamless 3D geometry. Observed silhouette changes do not establish joint-angle limits or a temporal animation cycle.

Use manifest paths, labels and recorded transforms when connecting consumers. Do not apply registration twice or reuse untransformed part pivots on transformed images. Test the consumer path actually used and distinguish generated files from verified application behavior.

Return the resulting artifact paths, what was executed and inspected, and any unresolved part of the user's request. Keep these instructions synchronized with verified tool behavior as capabilities change; do not add speculative commands or turn an implementation limitation into a permanent user requirement.
