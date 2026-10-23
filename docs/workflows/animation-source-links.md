# Native source round trip (#23)

`animation.sources` returns `{project,sources:[...],unsaved,save_destinations}`.
Each source contains `id`, `backend`, canonical `source`, nullable canonical
`bundle`, nullable paired `character`/`track`, integer `revision`, and nullable
`accepted`. IDs are stable, project-local, and use letters/numbers/underscore/hyphen.
Backends: `blender` (.blend), `synfig` (.sif/.sifz), `opentoonz` (.tnz), and `mlt`
(assembly JSON, not arbitrary editable MLT XML import).

- `animation.source.set {expected_project,id,backend,source,bundle?,character?,track?,expected_revision}`
  connects an existing artist-owned native source. Revision 0 creates; an update
  requires the current revision. A successful update increments it and clears the
  accepted-result link; original native files and historical job receipts remain.
- `animation.source.render {expected_project,id,expected_revision,options:{...}}`
  uses the existing bounded worker, global slot, status and cancellation commands.
  Options allow only that backend's documented render controls plus memory/timeout.
  Source paths, bundle, character and source identity cannot be overridden. Returns
  ordinary job identity plus `source_id`/`source_revision`, also persisted in job
  metadata. Rendering does not change the registration revision.
- `animation.source.accept {expected_project,id,expected_revision,job}` accepts
  only a matching completed full job. Partial previews, entirely transparent clips, stale source revisions,
  different backends and changed clip hashes are rejected. Clip hashing streams
  from disk; files over 512 MiB are refused. Success increments the source revision.
  `accepted` includes job/clip/hash, receipt path, timing/canvas/world rectangle and
  frozen provenance. Full dependency manifests stay in the immutable job receipt.
  Acceptance links a rendered snapshot; it does not assert the native source has
  remained unchanged since rendering or translate native edits into rig keys.

Mutations return `{ok:true,source:<updated row>,revision,project_saved:false,unsaved:true}`.
Explicit `project.save {expected_project}` persists the source map in `.magic`;
`project.open` restores it. Optional expected-project guarding on project.save also
protects existing callers that opt in. Native file Save belongs to its editor;
film placement remains a separate explicit `video.place_track animation_job=...`
operation. Accept never changes the native source path or places media in a film.

The UI and CLI use these same registry commands. Ordinary native source creation,
conversion of rig actions into new Synfig/OpenToonz projects, arbitrary MLT XML
reimport, and conversion of native graph edits back into Studio rig keys are not
provided by this source-registration slice.
