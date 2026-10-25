# Authoring primary / supplemental Blender rendering — #23

Branch: fix/scooter-authoring-primary
Worktree: /run/media/jesse/Speedy/Work/omhas-animation/.worktrees/authoring-primary
Base: fc3145dd0f14f0fa2eb32db83968b13cf2959986
Commit: 4183769 (full SHA from git rev-parse HEAD)
PR: none. No push, merge, deployment or parent checkout change.

## Concrete correction
animation.js no longer defaults/reopens to a render-only replacement view. Subject is the default authoring experience. Saved subject/composite choices stay valid; the obsolete render-only blender preference is interpreted as subject without rewriting storage on startup. Explicit authoring switches remain guarded and persisted.
The stable motion-mode-blender control now means Blender render, an explicit supplemental dialog. It does not change motion.mode or overwrite the authoring view. blender.js owns independent character/action metadata and dialog lifecycle; it no longer clears authoring images/samples/selection, disconnects its observer, rejects Theatre exports, resets Theatre state, replaces view-animation, or writes motion.meta/track.
Existing Blender start/status/cancel/resolution/range/saved-scene/preview/fallback features remain intact inside the dialog. No worker/backend code changed. Closing disposes only render preview/polling and removes the dialog. Authoring refresh stays in the Subject editor; Composite refresh redraws its own inspector/canvas instead of querying legacy rig metadata.

## Owned files
ui/animation.js, ui/blender.js, localized ui/style.css additions, tests/agent-selection.mjs and new generic tests/animation-authoring.mjs.
Parent semantic integration: the prior Legacy-to-Blender selection-clearing fixture is intentionally replaced with preservation assertions because render metadata no longer overwrites authoring metadata. Keep new fixture behavior alongside companion import; do not retain the now-obsolete render-only selection-reset expectation. No index.html change/dependency on uncommitted placement helpers.

## Evidence
Generic RED before implementation: red.log failed default-mode and editor/state replacement cases.
GREEN: green.log and harness.log. Checks default/saved mode, obsolete preference handling, actual explicit render button opening/closing, inspector/canvas/timeline DOM identity, selected part/action/time, metadata/images/sample/path identities, independent render action changes, captured generation references, unsaved guard and authoring/Composite refresh. Fixture transport only; no native render/image generation.
All standalone Node UI checks passed, including theatre-bridge: ui-checks.log. Browser-only agent-drawer not run. cargo check --workspace and cargo build -p magicstudio-shell -p mc passed; logs here. node --check and git diff --check passed. Normal unchanged commit hooks passed18security tests/1ignored plus existing UI and new authoring cases: commit.log.
No new window, screenshot loop, worker render, production write, or reserved-port access. Visual approval remains Jesse/Fozzie; no native dialog visual approval claimed.

## Other preserved checkpoints
Companions committed normally2b16f6d0e4ec0b6639d9bcabee0b062803712207 after requested one unchanged-hook retry. No dependency in this urgent branch.
Placement paused at WIP commit0ab247ad5df0b0fd3a7f0271196523b24e52aeaa on feature/scooter-place-blender-scene, .worktrees/place-blender-scene. Contains helper + generic tests only, not connected to UI and not a completed Place in scene feature. Initial WIP hook lacked its own target/debug/mc; building local mc resolved setup and normal hooks passed. No bypass. Future wiring can append its component to the supplemental render dialog and feed selected completed receipt; no placement expansion performed here.
Only generated frontend/bun.lock untracked/excluded. All prior worktrees preserved.
