# Scooter — completed controls checkpoint (#23)

Current existing checkout: `/run/media/jesse/Speedy/Work/omhas-animation/.worktrees/layer-order`, branch `feature/scooter-layer-order`, source HEAD **10f140a**. Tracked source is clean. No PR, push, deployment, production film/project/PDF edit or shared-worker submission.

Review-ready scoped UI commits are **a35f427** (layer stacking), **e46ec3f** (MLT Export controls), and **10f140a** (action-export fidelity). Hold all merges/consolidation until Jesse instructs them. Native OpenToonz/placement UI **00ae180** is already on canonical integration as bc3888c. Own history merged bc3888c via d5dd44d and 9f8a182 via3756ca1; the sole additive CSS conflict preserved all canonical Song/lyrics styles and native hidden-panel rule. These baseline merge commits are not additional feature patches to apply to integration.

Workflow: no further worktrees or target copies. Existing source/WIP worktrees remain preserved. Fozzie owns main consolidation and serialized shared builds/commits; Scooter has not touched dirty main/index. Sam should record this source checkpoint and file ownership before any move to main.

Final export acceptance fix **10f140a** changes only animation.js and the existing animation-transport.mjs generic fixture. It captures project/character/action/rest/amount/rate, requires unsaved native Theatre edits to be saved before dispatch, prevents duplicate submissions and stale-project success announcements, retains the captured receipt in read-only ui.state, and refuses empty/missing output success. Backend companion **c2ae39d** provides relevant saved-Theatre refusal before sampler/renderer and documents actual rest/amount passthrough. Both source commits passed their normal hooks in serialized slots. No native export render was submitted. Final source is10f140a; last paired native build remains e46ec3f-1792788785, and that owned window is CLOSED. No paired rebuild after the final two-file fix, as explicitly instructed; no stale binary was relaunched.

## One concise checklist

| Area | Implemented | Remaining / validation limit |
|---|---|---|
| Inspector | Independent persistent expansion, bounded icons, wrapping controls | Jesse owns visual approval; earlier actual CSS geometry contained at160–270px |
| Pickers/source | Readable action/object names, stable IDs, explicit backend, selection reconciliation, explicit choice outranks restore | None in current contract |
| Timeline/timing | Play/pause/seek/back+forward frame, rate/amount, preview loop, authored name/duration/loop, faithful captured Rest/action export and output confirmation | Source duration edit is not automatic key retiming; saved Theatre export requires backend c2ae39d and explicitly refuses unsupported evaluation |
| Parts/pose | Captured visibility/solo/show-all; persistent position/scale/rotation/pivot/reset/undo | Actual saved Theatre/standard-action limitations shown explicitly; timeline preserved |
| Paths/layers/depth | Root selection, route point editing, timing/start/end size/apply/undo; Back/Front/numeric stacking depth, captured hash/shared undo, actual metadata and CLI state | Stacking is back-to-front, zero-based, separate from spatial path/scale; parents unchanged. Sampler-ID mismatch refuses mixed stale image/transform data |
| Blender | Supplemental source/action/range/resolution/render/cancel/progress/preview/saved-scene fallback | No new Blender job in this checkpoint |
| Place in scene | Authored scenes, receipt/timing/rate/rect validation, exact CLI/readback; native animation_job needs no invented character/track | Generic UI-command tests; no production film placement |
| Static pose/storyboard | Current native Theatre export, captured time/rest/amount/reference, real pending PNG registration/preview and authored-page draft placement controls | Actual PNG capture/decoded pixels passed; storyboard placement generic checks only; no PDF mutation/approval |
| Native tools | Real Synfig/OpenToonz start controls, capability reasons, shared lifecycle/history/preview/frozen source+exact bundle; native placement | Actual OpenToonz UI unavailable due missing runtime; no native render claim. Generic lifecycle tests pass |
| MLT Export | Export entry opens existing native dialog on MLT; saved timeline JSON/resources/public start/shared lifecycle/frozen rerender, MP4 preview MIME, native picture placement | Actual runtime/discovery/UI verified; start tested at generic boundary only, no render submitted. HyperFrames picture remains muted; no soundtrack routing |
| Save/restore/parity | Bounded20 project UI sessions, route/selection/preview prefs, preserved Theatre save, read-only CLI state/hash/order/depth, separate mutation/refresh errors, correct checkbox dirty state | No automatic project save or silent Theatre clearing |

## Source/build/native evidence

- a35f427, e46ec3f, and10f140a passed normal unchanged hooks; the final Node/security slot is released. Each meaningful behavior regression was observed red→green in existing generic fixtures. Logs `layer-ui-*`, `layer-sample-*`, `mlt-ui-*`, `mlt-preview-red.log`; no new render/video suite.
- Worktree-local shell and mc rebuilt TOGETHER after the canonical9f8a182 merge and final UI commit. Build log `final-controls-build.log`; sourceSHA **e46ec3f**, observed `ui.feedback` build **e46ec3f-1792788785**, exact executable `.worktrees/layer-order/target/debug/magicstudio-shell`, owned PID1478749/port19943, project `design/animation/controls/generic-order-fixture/Generic.magic`. Identity recorded in `order-acceptance-identity.json`.
- Actual native UI layer Front changed order to `[child-card,parent-card]`, selected parent remained stable, child parent remained `parent-card`, depth became1, sampler IDs matched. Actual UI undo restored original order/depth0. See `order-native-after.json`, `order-native-undo.json`.
- Real pending pose export after reorder at0.5s decoded to expected red overlap RGBA `[220,40,40,255]`, with matching layer IDs and no frontend errors (`order-native-pose.json`, `order-native-decoded.json`). Direct canvas getImageData was blocked by browser taint security: that measurement is UNAVAILABLE, not zero or a pixel-success claim.
- Actual Export-button click opened MLT backend; only MLT fields visible; authoring timeline remained mounted; `data-commands=mlt.start`, runtime configured but codec capability explicitly unknown until bounded job; muted picture guidance visible; no frontend errors (`mlt-native-ui.json`, `mlt-native-ready.json`, `mlt-actual-discovery.json`). No worker submission.
- Previous OpenToonz UI check used actual missing-runtime discovery, disabled Start, correct native fields/public command mapping, mounted timeline and placement, no errors (`opentoonz-native-ui.json`). Exact nested frozen bundle root and no fake native IDs tested generically.
- Only isolated generated generic fixtures were saved. Guarded app.quit succeeded; PID1478749 and19943 are confirmed gone (`order-acceptance-exit.json`). Earlier owned previews also closed. Fozzie19927 and other reserved windows untouched.

## Preservation and disk correction

Excluded generated `apps/magicstudio-shell/frontend/bun.lock` is untracked. Earlier source snapshots under `.worktrees/animation-controls` and `.worktrees/inspector-usability` remain preserved even where superseded by committed source.

Speedy filled with duplicated build outputs. Scooter removed only owned inactive target directories after checking running executable use, reclaiming about90GiB. Every source worktree, staged/WIP change, evidence file and media asset was retained. Only the current layer-order target remains for Scooter; no further copies. Last measured Speedy111GiB free /76% used. Other owners' cleanup was coordinated through Fozzie.

All current contracted UI paths are wired. OpenToonz native installation/codec execution, fresh worker UI renders, production film placement and Jesse's visual approval remain unclaimed. Waldorf's readiness follow-up retains existing command parameters and render_capable/render_reason, already consumed by this UI.
