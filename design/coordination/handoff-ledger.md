# Coordination ledger — Magic Studio (durable, concise)

Maintained by **Sam the Eagle** (steward). ONE ledger, file-level ownership:
each owner owns their rows/files only. No blanket staging; one commit/index
owner at a time. Fozzie owns safe consolidation. Supersedes the pre-override
steward copy on branch `coord/sam-handoff-ledger` (preserved, unused).

## Workflow rules (Jesse, 2026-10-23 — overrides worktree-per-task)

- **ONE main checkout**; isolated FILE ownership + coordinated commits. Stop
  creating worktrees; do not direct others to create them.
- Preserve all current branches/WIP. Nobody merges/resets/deletes source.
  Agents finish current scoped commit where it is; move to main only after
  Fozzie confirms consolidation (main has concurrent edits).
- One commit/index owner at a time; no blanket staging. Own scoped commits,
  why/behavior messages, normal hooks; no push/deploy/history rewrite.
- **Desktop/workspace rule (Jesse, final)**: headless CLI/source work stays
  preferred for everything. Visible acceptance launches/restarts are allowed
  ONLY **background/silent on the owner's assigned workspace**, fixed size,
  at launch, **never taking focus and never switching Jesse's workspace** —
  never launch on the current desktop then move later. Current assignments
  (Omarchy per-monitor slots on **HDMI-A-3**; record BOTH IDs to prevent
  duplicate bar labels): **Fozzie = visible 5 = internal 15** (his review
  windows now there, moved silently; Jesse's active workspace 12 unchanged);
  **Waldorf = visible 3 = internal 13** for his visible apps. Earlier bare
  "3/5" and "4/13" numbers superseded — 3/5 were visible labels only, 4/13
  the older directive. No workspace assigned = no visible launch (headless
  only). Per-app owners preserve exact app/port ownership as before; visible
  updates only when Jesse explicitly asks. Hyprland here uses Lua `hl.dsp`
  dispatchers (not old string syntax) — no config edits. Relayed once to
  agents (issue 27 + issue 24 for Gonzo); no prompt loop. Fozzie follows the
  same rule.

### Workspace assignments — DEFINITIVE (Jesse; supersedes the C17 draft)

**All future agent GUI launches: silent on the agent's own workspace, exact
PID/endpoint pinned, no `ui.focus`, no workspace navigation. Never
workspaces 1–2 (Jesse/Herdr). No current windows moved.** Launcher:
`scripts/agent_window.py` + `tests/test_agent_window.py` +
`docs/workflows/agent-workspaces.md` — same rules per agent on 3+, generic
tests green (refuses 1–2; Hyprland silent + `no_initial_focus`; exact
executable exec; no fallback; `/usr/bin/true` dispatch accepted without
workspace change or GUI).

| Agent | Workspace | Note |
|---|---|---|
| Jesse / Herdr | 1, 2 | reserved; launcher refuses |
| Scooter | 3 | |
| Waldorf | 4 | supersedes C12's visible-3 for future launches |
| Beaker | 5 | |
| Statler | 6 | |
| Bunsen | 7 | |
| Sam (steward) | 8 | headless; reserved |
| Fozzie | **9** | **preserves his existing project window 9** |
| Gonzo | unassigned | omitted by Jesse — headless / issue 24; ask before any launch |

**Focus fix (Jesse, verified)**: local `~/.config/hypr/hyprland.lua` rules
for exact class `magicstudio-shell`: `render_unfocused=true`,
`focus_on_activate=false`, `no_initial_focus=true`. Config-only reload, no
errors; effective via getprop; global 15 FPS unchanged; active workspaces
unchanged (2/12); no GUI started/moved. Backup:
`hyprland.lua.bak.agent-windows-20261023-175410`.

- Fozzie: consolidation owner, integration target, production window 19927.
  Gonzo: GitHub issue 24 only. No new recurring audit work.

## Storage cleanup (one bounded pass, 2026-10-23 — verified)

- Policy: each owner cleans ONLY own obsolete regenerable Cargo outputs after
  checking running executables + active builds. ONE active cache per owner,
  serialized builds; never clone target dirs. No rm bypass / force stops /
  rebuilds to prove it. Never touch others' outputs, shared main target,
  production/media/source/evidence.
- Scooter: ~90 GiB own inactive targets reclaimed (source/media/WIP kept).
- Waldorf: cargo-cleaned wt-waldorf-blender-worker/target 9.7 GiB,
  .worktrees/synfig-adapter/target 7.8 GiB, .worktrees/motion-authoring/
  target 8.4 GiB (25.9 GiB apparent, ~22.4 GiB du). Steward verified all 3
  absent, manifests preserved. Retains .worktrees/backend-completion/target
  4.4 GiB (now historical after his move to main — cleanup accounting
  pending). No active cargo/rustc at check; Speedy free 89 G → 111 G.
- Shared main target **57 GiB: untouchable, Fozzie's**. Deferred (not
  waited on): Beaker, Bunsen, Statler, Gonzo — one cache each when idle.

## Ownership (owner-confirmed; SHA follows their commit)

| Owner | Owned files | Pending SHA/WIP | Active build |
|---|---|---|---|
| Fozzie | integration + consolidation; film/design; review workspace **5** | integration/blender-animation @ bc3888c; status/gaps posted on issue 23 (PR25 draft, unmerged) | integration target; window 19927 |
| Statler | NEW ui/words.js; additive: index.html (Design tab/section/script), app.js (design setView branch only; his Song sections), style.css (own end block); engine words.rs, lyrics.rs | **Committed** on .worktrees/song-lyrics-reader: 5c4be84 (Design > Words panel over words.*, #26) + 550162d (keep Words controls out of unsaved-edit guard, #26); base a2235ea; tracked clean, bun.lock only untracked. Owner's promised SHA handoff still pending | none reported |
| Waldorf | backend/proofs: OpenToonz runtime/probe/XML resource validation, planar order/depth, MLT cuts/audio; visible apps workspace **3** | **Now on shared main**: bfb19f9 + 6db652c (base 85e1da0) + ef66d63 + **648d035** (4 controls gaps closed; observed red→green, full hooks green) + f6f9bf8 (Scooter UI, via slot release). **One paired build PASSED: f6f9bf8-1792791912; SLOT RELEASED.** Fresh CLI registry/readiness checked; no live windows/production edits. Handoff: waldorf-controls-integration.md. **OpenToonz native clip acceptance RETAINS failed/unverified status — needs a valid nonempty scene; must not be reported completed** (owner request, C15) | no live port; no global env changes; backend-completion/target 4.4 G historical, cleanup pending |
| Scooter | Animation modules: animation.js, motion-authoring.js, animation-native.js, blender.js, blender-placement.js; localized inspector style/includes; generic fixtures; **prospective: main.rs window creation + native launch helper + tests (C15)** | Final source 10f140a landed as **c97d699**; a35f427/e46ec3f + c2ae39d as **2281160**; post-completion patch **f6f9bf8** (4-file render status recovery). **Window-placement task**: heads-up that feedback.rs reuse_open implicitly focuses (:244 set_focus, :265 ui.focus); main.rs ~:563 currently default compositor-positioned. Headless tracing only — no launch/focus/move; coordinate overlapping owner before edits; **awaiting Jesse clarification: Magic Studio vs Codex preview**. Evidence design/animation/controls/HANDOFF.md | No build in flight (Waldorf's paired build passed, slot released); port/PID 19943 closed; only excluded bun.lock untracked |
| Gonzo | Backstage telemetry | feat/backstage-topbar @ 5c013d0 — issue 24 only; **no reply yet** (asks stand at C1/C4; not re-polled; desktop rule sent once) | none reported |
| Beaker | **Companions (C13)**: MODIFY ui/companions.js; NEW ui/companion-rails.js, ui/companion-boxes.js, ui/companion-sprites.js, ui/companions.manifest.json, ui/media/companions/*.png, tests/companions.mjs; ADDITIVE style.css (companion/box CSS only), index.html (slot/box/script hooks only), app.js (el() ~192 + block() ~376 wrap annotations only — no rewrite), feedback.js (bounded box snapshot). **Do NOT touch**: crates/**, apps/mc/** (Gonzo TUI via issue 24), engine/native/auth/production, anything outside. Prior: marketing; native-tool research (Synfig/OpenToonz/MLT) | Base **ef66d63**, main, tracked/index clean; no new branch/worktree/cache; explicit-path commits direct to main; **claims next main commit slot — Fozzie serializes** (ledger commit also pending). Approvals: Fozzie = architecture, Jesse = implementation, start immediately, **no approval loop**. First usable SHA + migration list when the first coherent batch lands. Coordination: Statler (app.js Song/Words wrap — wrap not rewrite), Scooter (shared feedback readback) | Review: shared review slot **visible 5 = internal 15** (C12 mapping; Beaker wrote "5 internal / visible 5"), 1920x1080, silent, no focus; own port + PID, never a duplicate of 19952. TUI reads scent via existing bounded feedback/event channel (obs time + source; stale/unavailable explicit, never invented failed/zero) |
| Bunsen | rename cleanup; OpenDesign/Gaffer proposal doc — **doc COMPLETE** | 85e1da0 on main (docs/workflows/opendesign-gaffer-template-proposal.md; docs-only scope, no code/tests/install; reviewed by Fozzie). Rename cleanup fix/magicstudio-rename-cleanup @ 9dbc7a7 still no PR. Agent now on OpenCode | none reported |
| Sam (steward) | this file only | ledger edit uncommitted — awaiting Fozzie's serialized commit slot (index clean since C7) | none |

## Check-ins (concise)

- **C1 10-23 12:40** — baselines recorded; handoff asks on issue 27, issue 24
  for Gonzo.
- **C2** — canonical baseline bc3888c + no-stale-build contract. Waldorf full
  handoff: f7f7865 merge, shell build ID f7f7865-1792788528, binaries
  /tmp/magicstudio-herdr-handoffs/waldorf-backend-build.json, port NONE;
  first build SIGBUS (disk-full), recovered via own cache only. Handoff files
  collected from /tmp/magicstudio-herdr-handoffs/.
- **C3** — cleanup pass results (above); Statler ownership claim recorded;
  worktree-per-task rule overridden (above); ledger made durable here.
- **C4** — Scooter ownership claim recorded (layer-order, a35f427 + e46ec3f
  ready, 19943 closed, awaits Fozzie clean switch to main); Waldorf continued
  claim recorded (opentoonz.rs + adapter doc WIP, awaits serialized build
  slot). Report dropped once to Fozzie
  (/tmp/magicstudio-herdr-handoffs/sam-eagle-ledger-report.md); cleanup ask
  re-issued to Gonzo on issue 24.
- **C5** — Waldorf c2ae39d + 83eb2aecd2 verified in checkout (normal hooks +
  focused red→green; readiness build ID 83eb2ae-1792789276 steward-verified,
  binaries remain 83eb2ae per Fozzie). Slot used → Scooter, then released.
  AGENTS.md now documents the rename (.magic + project.migrate) — earlier
  inventory item resolved. No separate Fozzie report (SHA already with him).
- **C6** — Scooter checkpoint verified: 10f140a (final 2-file export fix) on
  feature/scooter-layer-order; tracked source clean, bun.lock only untracked;
  hooks PASS; Node/security slot released; last native build
  e46ec3f-1792788785; 19943 closed.
- **C7 — consolidation observed on shared main**: HEAD **c97d699** (Scooter
  UI final) on 2281160 (Waldorf export guard) on 8bec744 / 5e9fbf0 / 41c0849 /
  9f8a182 (readiness / MLT UI / layer UI / MLT worker) — all claimed work on
  main, steward-verified. Index clean (0 staged entries; rename work landed);
  56 dirty entries all untracked, including this ledger. Scooter FINAL:
  goal complete; future = shared main file ownership + serialized
  commits-builds; no git switch / new worktrees / Scooter builds; Fozzie owns
  one paired main build reusing the original target; layer-order checkout
  preserved until cleanup accounted; 19943/PID closed. This ledger stays
  uncommitted pending Fozzie's serialized commit slot.
- **C8 (scheduled)** — Deltas verified, no asks issued:
  · Waldorf on shared main: bfb19f9 + 6db652c (base 85e1da0), hooks green ×2,
  build 6db652c-1792790580, slot released/worker idle, no ports/collateral;
  **clip verification NOT passed** (fixture parsing / empty-timeline timeout;
  worker XML self-closing validator bug documented separately) — honest gap,
  Native_render_verified null.
  · Bunsen doc complete: 85e1da0 (OpenDesign/Gaffer proposal, docs-only,
  reviewed by Fozzie).
  · Statler's promised SHA observed: 5c4be84 + 550162d (Words panel #26) —
  owner handoff message still pending (delta recoverable from git).
  · Fozzie posted his status/gaps on issue 23 (PR25 draft unmerged; mc-rename
  changes live separately; Words/Artwork deferred to #26).
  · Gonzo: still silent on issue 24; asks stand, no re-polling.
  · Report appended once to Fozzie (same single report file).
- **C9** — Waldorf delta: main **ef66d63** (base 6db652c) — parsed native
  OpenToonz XML resource-path guard + focused regressions, files
  pipeline/native_animation_worker.py + test only; red→green, normal hooks
  green, **slot released**; no paired rebuild/native job (build
  6db652c-1792790580 predates embedded-worker fix); **native acceptance
  explicitly failed/unverified, no valid nonempty fixture yet**. Handoff:
  waldorf-opentoonz-xml.md. Jesse desktop rule v1 recorded and broadcast once
  — issue 27 (all) + issue 24 (Gonzo), no prompt loop. Report appended once
  to Fozzie.
- **C10 — desktop-rule clarification** (supersedes v1): launches allowed
  background/silent on a dedicated assigned workspace, fixed size, no focus,
  placement at launch (never launch then move). Fozzie's review windows
  moved 13→… (final numbers at C11). *(Previous note claiming C10 relay was
  premature — actual relay happened once at C11.)*
- **C11 — final workspace assignment (Jesse)**: **Waldorf = 3, Fozzie = 5**;
  supersedes 4/13. Fozzie's existing review windows moved 13→5 silently, no
  active workspace switch. Rule: background/silent on assigned workspace,
  never focus or switch Jesse away, preserve exact app/port ownership, no
  workspace assigned = headless only. Relayed once this pass (issue 27 +
  issue 24); report amended once to Fozzie. Ledger commit slot still open.
- **C12 — per-monitor mapping correction**: Omarchy slots on HDMI-A-3 —
  **Fozzie visible 5 = internal 15; Waldorf visible 3 = internal 13**. Raw
  3/5 directive superseded (3/5 were visible labels; both IDs recorded per
  Jesse to prevent duplicate bar labels). Fozzie's windows now on internal
  15, moved silently; active 12 unchanged. No new agent relay — the C11
  relay already used the correct visible numbers (no prompt loop). Report
  appended once to Fozzie; ledger commit slot still open.
- **C13 — Beaker companions ownership recorded** (row above + contract
  below): approvals locked (Fozzie architecture, Jesse implementation, no
  approval loop); base ef66d63, main index clean, no new branch/worktree/
  cache; explicit-path commits direct to main; **claims next main commit
  slot — Fozzie serializes against the pending ledger commit**. Review on
  the shared review slot (visible 5 = internal 15), 1920x1080, silent, no
  focus, own port+PID never 19952. First SHA + migration list to follow.
- **C14 — slot chain completed; Fozzie absent**: Waldorf **648d035** on main
  (4 controls gaps closed, observed red→green, full hooks green) → released →
  Scooter's four-file render status recovery patch **f6f9bf8** — both
  steward-verified, index clean. **Fozzie absent (agent_not_found)** →
  notifications continue via the single report file; no prompt, no retry
  loop. No production changes/windows/jobs.
- **C15 — build passed, slot released, placement task logged**: Waldorf's
  paired build **f6f9bf8-1792791912 PASSED**; **slot released**; fresh CLI
  registry/readiness checked; no live windows/production edits; handoff
  waldorf-controls-integration.md. **OpenToonz native clip acceptance
  RETAINS failed/unverified status — needs a valid nonempty scene; must not
  be reported completed** (owner request). Window-placement task logged:
  prospective owner Scooter — main.rs window creation (~:563, default
  compositor placement), native launch helper, tests; heads-up: feedback.rs
  reuse_open implicitly focuses (:244 set_focus, :265 ui.focus); headless
  tracing only, no launch/focus/move; coordinate overlapping owner before
  edits. Index clean; Beaker's in-progress edit to owned ui/companions.js
  observed (unstaged).
- **C16 — scope CORRECTION (supersedes the C15 placement framing)**:
  Jesse's clarified failure = **agent launches the app, then cannot find it
  until Jesse hovers that workspace**. Scope is **cross-workspace
  endpoint/window discoverability — NOT window relocation, NOT config
  changes**; **main.rs changes not yet necessary** (the C15 prospective
  claim stays contingent on this scope). Tracing studio_sessions / feedback
  / bridge read-only; no source edits, no app operations.
- **C17–C18 — definitive workspaces, focus fix, hook block**: DEFINITIVE
  assignments recorded in the Workspace section above (supersedes both the
  C12 launch pairs and the earlier draft: Fozzie 7→**9** preserving his
  project window, Bunsen 9→**7**, Gonzo unassigned). Launcher
  scripts/agent_window.py + tests + docs staged, tests green (refuses 1–2;
  silent + no_initial_focus; exact exec; no fallback). Jesse's focus fix:
  hyprland.lua class `magicstudio-shell` render_unfocused /
  focus_on_activate=false / no_initial_focus — config-only reload, no
  errors, getprop effective, 15 FPS unchanged, workspaces unchanged 2/12,
  backup hyprland.lua.bak.agent-windows-20261023-175410, no GUI
  started/moved. **Hook block**: launcher-only commit FAILED — Beaker's
  dirty C13 set (companions.js ~:119 `undefined.length`, app.js, index.html,
  style.css = exactly his owned four) fails all 13 companions generic cases
  in normal hooks; staged launcher paths (AM) left untouched; slot released
  to Beaker; nobody mid-commit (no cargo/rustc/hook processes at check).
  Distributed once: issue 27 (full) + issue 24 (Gonzo unassigned note).
  **Beaker session identified in Herdr**: pane **wG:pH** (claude, unnamed,
  title "Magic Studio companion-pet app.js coordination", idle at check) —
  hook-recovery prompt sent there once. Idle inventory at check: Bunsen
  wG:pM, Scooter wG:pK, Waldorf wG:pN; codex "Polish Magic Studio TUI"
  wG:p7 done (TUI work — likely Gonzo, unnamed); omp scan (wG:pB) and
  hermes (wG:pF) working, owners unclaimed.
- **C19 — root cause identified; owner fixing (no audit broadening)**:
  Exact blocker sent to owner pane **pB (omp — Beaker's session, working)**:
  the controller's canonical dog spelling drifted **sproket → sprocket**
  (nodes/ROLE) while shipped companions.frames.json and public fixtures/API
  still use **sproket** → frame lookup misses → `undefined.length`
  TypeError on all 13 cases. Smallest fix: restore canonical **sproket** in
  the owned controller (explicit original user requirement). No further
  diagnosis needed. Launcher author un-staged only their own 3 files —
  **index clean (0 staged)**; shared index/slot released for Beaker; source
  preserved. pH (companion-pet coordinator, not the owner) was told the
  notice already reached the owner — no forwarding needed. Team directive
  recorded: **ship small normal-hook commits; do not broaden audits**.
  Main f6f9bf8 at check; green release → launcher commits 3 paths once.
- **C20 — launcher landed (178c00b)**: exactly 3 paths —
  docs/workflows/agent-workspaces.md + scripts/agent_window.py +
  tests/test_agent_window.py (167 insertions), **normal hooks green after
  the companion owner's in-worktree correction** (sproket restore; hooks
  evaluate worktree state). Index released, 0 staged. Beaker's 4 corrected
  files (companions.js/app.js/index.html/style.css) remain dirty/unstaged
  awaiting his own single coherent commit — no babysitting of file saves.
  No extra manual test/review runs. Directive: **single coherent change →
  normal commit → next.** Local background-render/focus-stealing settings
  already active. Assignment doc distributed once (issue 27 + issue 24).
- **C21 — first-fish save/render/return workflow claim + ownership query**:
  Jesse directed plan+implementation of the complete first-fish native
  save/render/return workflow (shared source/action/native/result save
  links; UI/CLI handoff; native output proof; first-fish roundtrip;
  independent check). **animation_links.rs ownership: NOT active** — file
  absent from main (untracked), no worktree copy, absent from Fozzie's
  partial-worker.patch, no ledger entry anywhere; Fozzie absent from Herdr.
  No existing save-map file to duplicate — but save-map semantics the
  workflow defines must be recorded here for reconciliation with Fozzie on
  his return (existing in-repo save precedents: project.rs write_project_file,
  theatre save, storyboard save_version). **Hook/build reservation: FREE**
  (no cargo/rustc/commit processes; index clean; 0 staged). Beaker's dirty
  set grew to 5 — feedback.js added, still all within his C13 owned paths,
  unstaged, awaiting his single coherent commit. Requester continues
  native-source rendering independently; no production writes.
- **C22 — Waldorf native slice claim (ledger-only, no GUI)**: next slice =
  official `opentoonz/opentoonz_sample` **tga_paint.tnz** (known-populated
  6-frame scene) as the long-missing nonempty fixture — downloaded to
  `/tmp/waldorf-opentoonz-official/` only, **no production data**.
  Read-only validation exposed: required `$scenefolder` + Windows separator
  support in `opentoonz.rs`; literal-ampersand native TIStream dialect
  handling in `native_animation_worker.py`. He owns both + focused
  tests/docs — **independent of the C21 save-map workflow** (no overlap).
  Slot request: next normal-hook/paired-build slot **after Beaker's
  commit**. At check: Beaker still uncommitted (5 dirty unstaged);
  `crates/engine/src/opentoonz.rs` now dirty (his owned file, WIP);
  staged=0; no hook/build processes — slot physically unheld. Declared
  queue: Beaker → Waldorf (first-fish requester C21 also holds a
  free-slot go-ahead from their own check).
- **C23 — save-map semantics recorded + Beaker failure coordinated**:
  **Save-map contract for the new animation_links.rs** (Waldorf; Fozzie's
  stale claim confirmed inactive at C21): `.magic` `animation_sources` map
  held in Session clone/open/save; `source.set` guarded by
  expected_project/id/revision, keeping artist-owned editable
  path+backend+optional rig/action; `source.render` dispatches the existing
  bounded native backend and stamps link identity/revision into job
  metadata; `source.accept` only on a matching completed receipt — stores
  frozen receipt reference + clip provenance, no graph conversion. Commands
  mutate in-memory project links; **explicit `project.save` is the .magic
  destination**; native file Save remains its editor; film placement
  remains explicit `video.place_track`; **no hidden autosave or film
  mutation**. **Waldorf owns animation_links.rs + the animation_sources
  serialization; Scooter owns matching UI.**
  Index released: Waldorf unstaged only his 4 portable-source paths
  (opentoonz.rs, native_animation_worker.py, test file, adapter doc) — WIP
  retained, all 9 dirty files now unstaged, staged=0, Beaker free to commit.
  **Beaker's exact blocker delivered once (issue 27, async — pB was
  working, no pane interrupt)**: TypeError in his new block() rail-slot
  annotation (`head.parentElement.insertBefore` — fixture's <head>
  unparented) via imagePromptBlock at agent-selection.mjs:171; smallest fix
  inside his own files (guard the insertion), no test edit needed; the 13
  companions cases are green, this is the only remaining hook failure.
- **C24 — Scooter save-link UI claim (no edits yet)**: retains Animation UI
  ownership; next bounded source-link UI touches **animation-native.js +
  the existing native handler fixture** (blender.js only if the backend
  contract requires Blender source integration). Confirms the C23 boundary:
  **Waldorf owns animation_links.rs / session / project / registry**.
  Blocking ask: **exact readback/revision examples from Waldorf's
  implementation before wiring** — relayed to Waldorf in the steward reply.
  Hook slot requested only at a coherent commit boundary, normal hooks, no
  duplicate manual suites. **Preserve Beaker's wrappers** in any shared UI
  touched. State at check: main 178c00b, staged=0, 9 dirty tracked
  (Beaker 5 + Waldorf 4 WIP), slot unheld, Beaker's commit still pending.
- **C25 — declared slot ORDER + Beaker holds the index**: order confirmed
  by request and recorded: **Beaker → Waldorf backend → Scooter native UI →
  one paired build; no rendering until built.** At check: **Beaker's full
  companions batch is STAGED** (17 paths: modified app.js/companions.js/
  feedback.js/index.html/style.css + new companion-boxes/rails/sprites.js +
  companions.manifest.json + 8 sprite PNGs — all correctly spelled
  sproket-dog-*/gaffer-cat-*): he is the index owner, not yet committed;
  nobody else stages until that clears (Waldorf explicitly holding).
  Waldorf: source-link backend tests + workspace check **GREEN**, commit-
  ready; his 13 unstaged owned files preserved (source-link
  session/project/registry/lib/commands + native format opentoonz/worker +
  fixtures). **Scooter's DOM-stub correction (agent-selection.mjs) +
  native-handler files handed explicitly to ride Waldorf's commit.** No
  production writes; no rendering until the paired build.
- **C26 — queue amendment: Waldorf commits via `git commit --only`
  (concurrent-safe)**: to avoid holding integration behind Beaker's staged
  index, Waldorf will run a **normal `commit --only` of his explicit
  backend/docs/regression paths** (+ Scooter-granted agent-selection stub).
  Mechanics verified sound by steward: `--only` builds a **temporary index**
  from the working tree for the named paths only — **Beaker's staged
  entries remain untouched and staged**; new owned files get
  **intent-to-add only**; no blanket staging/reset/cherry-pick. Normal
  hooks still run and the backend commit must pass them. After it lands:
  **direct slot release to Scooter** (native UI), then one paired build,
  no rendering until built. No concurrent git/cargo processes observed at
  declaration. Beaker's staged 17-path commit proceeds on its own track —
  first committer lands first; neither disturbs the other's index state.
- **C27 — all three queue commits LANDED (steward-verified)**: Beaker
  **e6f20bc** "shared UI-box and companion-rail contracts" (his staged
  17-path batch, committed normally); Waldorf **b968ab2** via `--only`
  (13 files: **animation_links.rs 140L** + animation_links regression tests
  + docs/workflows/animation-source-links.md + registry/session/project/
  .magic persistence + frozen acceptance + portable native format fixes +
  Scooter's granted DOM stub). **Index safety held** — Beaker's staged
  entries were untouched until his own commit landed first; the `--only`
  temp-index plan worked exactly as scoped. mc readback preservation fixed
  by **omitting empty animation_sources — test passed unchanged** (no
  weakening). Scooter **3c8f45a** "link native sources, accepted renders
  and explicit Save" (3 stable UI paths, taken via direct slot release).
  **Worktree fully clean, staged=0.** Honest scope: no unknown native graph
  conversion claim; no production writes. Queue now: **Waldorf's paired
  build + known-supported native checks** (his) → **ledger commit (mine,
  held behind his build to avoid cargo contention on the shared target)**
  → first-fish roundtrip continues.
- **C28 — Scooter slice owner-confirmed complete**: `3c8f45a` = exactly 3
  owned UI/test paths, normal hooks green, index/slot released to Waldorf's
  paired build. All 4 native backend links wired — select / register /
  render / full accept — plus explicit Save; **no standalone dialog;
  Beaker's wrappers untouched** (C24 commitment kept). No further UI work
  pending this bounded slice; **actual native acceptance remains
  Waldorf's next** (paired build + known-supported native checks).
- **C29 (scheduled) — quiet delta, roster resolved**: no commits (main
  3c8f45a), no new drop-dir handoffs, no issue comments, worktree+index
  clean, **no cargo running — Waldorf's paired build not yet started**.
  Herdr roster resolved: **gonzo now named on wG:p7** (idle; issue-24 rule
  unchanged, standing ask not re-polled), **statler = wG:pH** (done),
  **beaker = wG:pB confirmed** (working), **rizzo = wG:pF** (new name on
  the former hermes pane, working — owner/role unclaimed), scooter + bunsen
  idle. One ask sent: direct handoff request to idle Statler (his promised
  SHA handoff never arrived; first direct ask to his pane, no repeats
  elsewhere). Report appended once to Fozzie.
- **C30 — Scooter Blender-source checkpoint (d82564c)**: exactly 3 paths
  (animation-native.js, blender.js, animation-native.mjs) — the C24
  contingency ("blender.js only if the backend contract requires Blender
  source integration") triggered and is now landed: Blender Sources/Save
  prefill + **collision-safe suggested ID, explicit Register, no
  overwrite**. Normal hooks green; index/slot clean and **released to
  Waldorf's final paired build**. No extra suite/build/render/window by
  Scooter. Steward-verified: main d82564c, tree+index clean, no cargo
  running yet.
- **C31 — acceptance progress + studios WIP query**: **05ca639 landed** —
  "refuse accepting fully transparent native results" (animation_links.rs +
  doc), directly encoding the TLV finding. Waldorf-verified acceptance
  evidence: mc-driven UI register/render/accept/Save; visible Synfig
  preview; little-fish blend 180 frames + CLI accept/save/placement
  **outside production**; OpenToonz raster camera-follow renders visibly.
  **Retained honest blocker: TLV inkpaint scene entirely transparent even
  with the complete official bundle → format/tool blocker; no full
  ToonzRaster claim** (now guarded by 05ca639's refusal). Paired build
  over 05ca639 **running as app PID 1755028, port 19955** (recorded as
  Waldorf's), Roundtrip.magic in /tmp (non-production), silent on
  **visible 4 = internal 14** (his assigned ws4, HDMI-A-3), user
  workspaces unchanged. **mc studios WIP**: dirty apps/mc main.rs +
  local_ui.rs + new studios.rs = `mc studios [--json]
  [<port>|--unpin|--check]` endpoint discovery (C16 scope; doc: "a Studio
  is a socket that answers ui.feedback"). mtimes 18:45–46 align with
  **gonzo flipping idle→working**; apps/mc is Gonzo's standing domain →
  likely owner, WORKING — no interrupt; **single ownership question sent
  via issue 24** (no pane prompt per his rule). **Slot answer**: Scooter's
  accepted-preview restoration → final paired rebuild; studios WIP lands
  separately after, explicit paths, own hooks. No production
  .magic/PDF/film saves by Waldorf.
- **C32 — split-build provenance (one correction)**: Technical correction
  accepted: a paired `-p magicstudio-shell -p mc` build WOULD compile
  Gonzo's apps/mc WIP — now **STAGED (M local_ui.rs, M main.rs, A
  studios.rs — Gonzo is the index owner)** — and touching it is forbidden;
  shell-only split is the sound path. **df7155f verified: exactly 2 files**
  (animation-native.js + animation-native.mjs — UI+fixture only,
  backend/registry untouched by it). Shell-only build **FINISHED (19.01s,
  /tmp/waldorf-preview-restore-build.log)** — but exact provenance must
  read: **shell = df7155f + uncommitted engine WIP**: log shows
  `Compiling magicstudio-engine` while crates/engine/src/part_replacement.rs
  was dirty (mtime 18:55 = build start; part-undo symlink-refusal reorder;
  **author unclaimed**) → the built engine does NOT equal committed 05ca639
  state. mc not rebuilt → mc binary provenance = 05ca639 paired ✓ as
  planned. Label rule recorded: never call this a "committed backend"
  build; the part_replacement author self-identifies at their own explicit
  commit. No integration blocked; no mislabeled builds.
- **C33 — worktree inventory understood + 4 commits landed**: Full
  65-worktree map recorded in the steward reply (canonical main; 14
  legacy .worktrees/* incl. Statler's 4 UNMERGED Words commits +
  Fozzie's reserved-but-empty animation-save-map branch; 17 sibling wt-*
  incl. detached d441250 holding unmerged Backstage/#24 commits; 7
  external; 14 /tmp with 6 git-prunable). Since df7155f, main gained:
  **50c06d5** (symlink-bound part naming — the unclaimed
  part_replacement.rs author committed), **4f53124** (Beaker companions
  control-signal refactor), **1a6f782** (Gonzo's `studios` CLI —
  issue-24 ownership confirmed in code), **39a2e93** (accepted preview +
  native select options) → main @ 39a2e93; index expected clear for the
  **final truly-clean paired rebuild**. Save-map reconciliation note: the
  Fozzie branch exists with 0 commits — Waldorf's landed animation_links.rs
  remains the only implementation.
- **C34 (scheduled) — 3 commits + a window incident**: New on main:
  **bae0108** (Beaker: boxes carry command receipts, readiness sets real
  states), **64f486c** (Gonzo: mc debug answers with live Studios — his
  studios line extended; issue-24 question fully answered in code, no
  comment reply needed, not re-polled), **c59eaef** (Beaker: companion
  sprite painted, off switch honoured) → **main c59eaef, tree/index
  clean**. Non-main build logs appeared (magic-studio-scopes/references,
  19:03/19:10 — /tmp booth trees, other owners, not chased). No new
  drop-dir handoffs; final paired rebuild not started (no cargo).
  **Incident from Statler's pane (his answer to my handoff ask)**: he
  found and closed his OWN duplicate windows (endpoints 1483803 +
  1493050, scratch Hey-Ho copy) after a refused app.quit (pre-fix build,
  leftover words-scrub/input draft fields — the 550162d false alarm).
  **Casualty: a Magic Studio window on workspace 9 (PID 1653567,
  --control 19963) vanished during his close attempts — owner unknown
  (not in steward port records; ws9 = Fozzie's slot) and must be
  reopened by its owner.** Remaining windows: Waldorf's roundtrip 19955
  (ws internal 14) + 19961 (ws 13), untouched. Statler's recorded lesson:
  close only via app.quit or exact PID, never a guessed compositor
  command; future windows = one, via scripts/agent_window.py (his ws 6).
  He still owes the structured handoff (SHAs/files/hooks) — asked once,
  not re-polled.
- **C35 (scheduled) — CLI/docs commits + a large staged batch + a quota
  warning**: New on main: **f557f50** (find commands, project objects and
  controls) + **9249ae8** (docs plans: unified panel reporting and video
  project wiring). **Index now holds a ~29-path staged batch**: mc/center/
  {booth,control,roles,workshop}.rs + {dials,scope,tui,worker}.rs +
  center/main/logo/snapshot/telemetry + README + shell main.rs + engine
  agent.rs/sheets.rs/wiring test + **three plan docs matching the three
  /tmp magic-studio-* booth trees exactly** (booth-project-overview,
  master-workshop-scoped-booths, workshop-project-cards; written 19:54).
  Author not self-declared — candidates: **Gonzo** (working; mc/Backstage
  domain) or **rizzo** (done; likely owner of those /tmp trees); awaiting
  self-identification at commit. **Index held — no other commits possible
  until it lands** (my ledger commit included). Unstaged leftovers:
  companions.mjs, companion-boxes.js, project_catalog.rs, workshop.rs.
  **⚠ Waldorf's agent: weekly model limit 6% left (resets 30 Oct 1:41
  PM), 1 queued question, still working 8m45s** — surfaced to Jesse;
  nobody answers his agent's question for him. No new handoffs/issues/
  builds; #27's 6th comment is the steward's own; Statler's structured
  handoff still owed (not re-polled); rizzo done, role unclaimed.

- **C35 — Unified panel reporting + video wiring plans (Jesse approved, go 2026-10-23)**:
  Coordinator: Claude session wG:pS. Plans: `docs/superpowers/plans/2026-10-23-unified-panel-reporting.md`
  (owners in its Who-does-what table) and `docs/superpowers/plans/2026-10-23-magic-studio-wiring-refactor.md`.
  **Claims**: Rizzo (wG:pF, new session) Tasks 1/3/4/6 — `ui/companion-boxes.js`, `ui/companions.js`,
  `ui/app.js:210-235`, `tests/companions.mjs`, `crates/engine/src/panels.rs` + `lib.rs` + `tests/panels.rs`,
  `apps/mc/src/studios.rs`, `apps/mc/src/panels.rs`, `apps/mc/src/main.rs` (panels subcommand only),
  `apps/magicstudio-shell/src/bridge.rs` (ui.panel only). Beaker (wG:pB, new session) Tasks 2/7 —
  `ui/feedback.js`, `ui/bridge.js`, `tests/panels.mjs`, `tests/ui-lint*`, then
  `ui/style.css` + `ui/*.js` dot/head/empty/status sites; `companion-boxes.js` only after Rizzo's Task 1
  commit. Gonzo (wG:p7, grok) Task 5 — `apps/mc/src/center.rs`, after Rizzo's Task 3. Claude wG:pS wiring
  Tasks 0-3 — `crates/engine/src/{workshop,video_target,project}.rs`, `crates/engine/tests/wiring_state.rs`,
  `ui/app.js:681,701` after Beaker's Task 2. **Supersedes** the C13 companions-contract boundary
  "Do NOT touch crates/**, apps/mc/**" for exactly these paths. Previous Beaker's unfinished app.js
  helper reverted (copy kept by coordinator); previous Gonzo's unmerged `b0a98f4`
  (/tmp/magic-studio-data-references) awaits Fozzie.
- **C36 (scheduled, steward) — merge shipped + shared-ledger note**:
  The ~29-path staged batch landed as **3642935 "Merge scoped Workshop and
  Booth CLI into main"** (booth-tree history incl. 760aacc/d30da95/d441250
  now in main; the three plan docs tracked), then a panel/box UI series —
  872a041, 46c304a, b65e920, edea6ea, 08529bf, 25b597f, 73ea47a,
  **e9ed7e2 "one panel head"** → **main @ e9ed7e2, index clean** (9
  commits since C35; transient 7-file UI dirty set observed then clean —
  no babysitting). **Ledger is now co-authored**: the coordinator entry
  above (Claude wG:pS) is the record of authority for the unified
  panel-reporting/wiring claims — steward preserves it and notes three
  corrections it makes: (1) **Rizzo (wG:pF) identified** — owns
  companion-boxes/companions/app.js:210-235/panels engine+CLI/bridge
  ui.panel incl. **studios.rs ongoing work** (C31's original authorship
  call to Gonzo stands for the committed code; future edits → Rizzo),
  (2) **C13's "Do NOT touch crates/**, apps/mc/**" superseded for exactly
  the listed paths**, (3) Gonzo's unmerged b0a98f4 awaits Fozzie. Panes:
  gonzo/waldorf/rizzo **done**, bunsen/statler **idle**, beaker **working**,
  wG:pS coordinating. No builds, no new handoffs/issues; **no asks issued**
  (Statler's handoff already asked once — not re-polled).
- **C37 (scheduled) — quiet: 3 commits, clean tree, one ask**: Panel
  refactor series continued: **a9e7522** (one panel empty state),
  **18b62c8** (one panel status row), **53d0fa2** (every colour is a
  token) → **main @ 53d0fa2**. Tree fully clean (0 staged / 0 unstaged),
  no builds, no new handoffs, issue counts unchanged (27 → 6, 24 → 16).
  Panes: beaker now **idle**; gonzo/rizzo/waldorf done; bunsen/statler
  idle; wG:pS idle. **One ask issued (idle owner, first for this item)**:
  Beaker asked once for the **remaining migration list** he promised with
  his C13 batch ("exact remaining migration list when the first coherent
  batch lands" — SHA landed, list never sent). Statler/Gonzo not
  re-polled. Report appended once to Fozzie.
- **C38 (scheduled) — quiet**: One new commit **4de9be6** (docs plans:
  TUI-panels task now points at the split `center` modules — coordinator
  wG:pS redirecting Gonzo's Task 5 target) → **main @ 4de9be6**. Tree
  fully clean (0/0), no builds, no new drop-dir handoffs, issue counts
  unchanged (6/16). Panes: **wG:pS working** (coordinator active);
  beaker idle (migration-list ask delivered, answer pending — not
  re-polled); gonzo/rizzo/waldorf done; bunsen/statler idle. **No asks
  issued this pass** — every owed item has exactly one open ask.
- **C39 (scheduled) — plan-doc series + one engine WIP**: Three coordinator
  docs commits — **f819560** (panel looks, one scale/palette,
  refactor-as-you-go ratchet), **85cae50** (one universal stylesheet in
  three layers; components locked), **3d1dd76** (unconfigured UI carries a
  reason; TUI maps untracked/unstyled) → **main @ 3d1dd76**. Unstaged
  **crates/engine/src/lib.rs** — active edit, likely **Rizzo's Task
  panels.rs+lib.rs registration** (he is working; author confirmed at his
  commit). Panes: beaker **working** (engaged after the migration-list
  ask), rizzo **working**, wG:pS **working**, waldorf now **idle**,
  bunsen/statler idle, gonzo done. No builds, no handoffs/issues new.
  **Queue note**: the **final paired rebuild (C25's last step) remains
  outstanding** — shell + mc have both moved far past 05ca639; any build
  taken now would compile Rizzo's dirty lib.rs (same provenance caveat as
  C32) — build owner (Waldorf) times it, not the steward. No asks issued.
- **C40 (scheduled) — panel-config refactor series + mid-commit index**:
  Four new commits: **fc477e5c** (panel flags derive from what the panel
  IS), **f59dcf42** (definition of done; TUI reads components.json;
  open/dirty/setter dropped), **cbeaf522** (panel record =
  id/label/look/signal/busy), **e3806730** (config measured: el() reads
  look/density, lint ratchets migration) → **main @ e3806730**.
  **Index mid-commit: 4 staged + 5 unstaged** (companions.js +
  ui-lint.baseline.json both-staged-and-unstaged; app.js/components.json/
  index.html/companions.mjs unstaged) — iterate-then-stage pattern; author
  likely Beaker or Rizzo (Tasks 1/2/7), self-identifies at commit. Panes:
  **? pane now named `coordinator`** (working), **statler working**
  (first activity since the handoff ask), beaker/rizzo/waldorf/bunsen
  idle, gonzo done. No builds, no handoffs/issues new (6/16). **No asks
  issued — all three open asks stand** (Beaker migration list, Statler
  handoff, Gonzo issue-24).
- **C41 (scheduled) — Gonzo's TUI task + Statler's handoff COLLECTED**:
  Four commits → **main @ 5f937b8f**: **940ea792** (feat(tui): Booth
  shows every panel's flags + unconfigured UI — Gonzo's Task 5 shipped)
  + three coordinator motion-pipeline plans (**fc799ca6** map pipeline to
  tools/commands, **2193f58e** four installed tools compared/Blender does
  the bones, **5f937b8f** animator's order: image→turnaround→poses→limbs→
  animate). Index: 1 staged (ui-lint.baseline.json). **Statler's owed
  handoff collected from his pane (answer to the one ask — now closed)**:
  his Task 10 work is on main — **381edf6** "words report timing
  disagreements (#26)" + **3a1c433** "Design > Words panel on the panel
  contract (#26)", **both steward-verified in main ancestry; pre-commit
  hooks passed for both** (his report); he moved onto shared parts
  (shared dot(), panel head, empty state; new shared components
  segmented/…) and the coordinator holds his SHAs. **Open item for
  Fozzie**: feature/song-lyrics-reader still **5 commits ahead** of main
  (550162d/5c4be84/3fc6a0d + merge) — reconcile whether 3a1c433
  supersedes the branch's panel commits or unique content remains.
  Panes: gonzo idle, statler **done (handoff satisfied)**, coordinator
  working (new plan series), beaker/rizzo/waldorf/bunsen idle. No builds,
  no handoffs files/issues new (6/16). **No asks issued**; Beaker +
  Gonzo asks stand.
- **C42 — ESCALATION: principal-directed takeover of the finish push**:
  Jesse reports he told **three agents to relay "handle this" to the
  steward — none of the three relays arrived** (steward received zero;
  recorded honestly so the broken relay chain is visible). With a
  benefit-of-the-doubt warning that muppets are facing termination, the
  principal direction supersedes tonight's passive cadence: **steward now
  actively drives the remaining Definition-of-Done work to completion**.
  Immediate action: direct task-kicks sent to every idle owner with
  queued work — **Rizzo** (Task 3 remainder → Task 4 mc panels from disk →
  Task 6 per-pet timer), **Beaker** (finish staged Task 7 baseline →
  Task 8 components.json/style.css), **Waldorf** (final paired build after
  Beaker's staged commit for clean provenance, or labeled dirty per C32 —
  closes C25's last queue step + DoD item 7). Coordinator wG:pS left
  undisturbed (working, wiring tasks flowing). Each kick: one message,
  exact task numbers, commit-only discipline, SHA to coordinator. Record
  the 3 lost relays; if they carried content beyond "take over", Jesse to
  supply it.

- **C36 — Statler's Words work and the panel contract (coordinator wG:pS, 2026-10-23)**:
  Checked by patch content: `9559ef1`/`3fc6a0d` are in main as `17668a1`/`6ddc530`;
  **`5c4be84` (Design > Words panel + words.rs timing_disagreements) and `550162d`
  (words.js unsaved-guard) are NOT merged** (corrected per Statler) — branch `feature/song-lyrics-reader`, 63 behind main.
  They predate the contract and would fail ui-lint as is. Task 10 in the unified plan:
  Statler ports them itself and keeps ownership of `words.js`, the Song/Words region of
  `app.js` (`songSection`, lyric model/sheet/body) and the Design tab; nobody else
  rewrites them. Beaker: leave those regions to Statler in Tasks 7/8.

- **C37 — Hey Ho Little Fish reopened on the refactor build (coordinator wG:pS, Jesse's go)**:
  Port 19955 (was Waldorf's roundtrip window, old build b65e920, clean: no unsaved state, Theatre
  saved) quit via app.quit and relaunched from target/debug built at cbeaf52, same args
  (--view animation --control 19955). Ready, no errors, nothing unsaved, 64 panels. The
  coordinator's own SprocketAd test Studio (19981) is closed.

- **C38 — Bunsen → Fozzie handoff, routed by the coordinator (Fozzie's Herdr pane is gone)**:
  `docs/workflows/opendesign-gaffer-template-proposal.md`, branch `docs/bunsen-opendesign-gaffer`
  (worktree /run/media/jesse/Speedy/Work/wt-bunsen-opendesign-gaffer, base 2babf5b), corrected commit
  `b5d4047` (fixes 4 errors in 6525525). Main had the uncorrected text as 85e1da0; the coordinator
  copied the corrected file onto main as **73555d1c** (one docs file, no merge, nothing pushed).
  Branch/worktree left for Fozzie. Open points for review: Words template cites `words.*` — the
  engine half is now on main (381edf6) and Statler's Words panel landed (3a1c433), so re-check the
  template against main; naming question raised: code says **Sproket**, task vocabulary says
  "Sproken" (no rename made). Full handoff: /tmp/magicstudio-herdr-handoffs/bunsen-opendesign-gaffer-template.md.

## Companions contract (Beaker — Fozzie + Jesse approved; recorded C13)

- **Box**: stable ID + project/context + state (idle / working / ready /
  needs-attention / failed) with reason / priority / request / job /
  resolution; optional light; all scented. Facts come only from actual
  command/job/render events; readiness data-state may inform prerequisite
  status; **empty ≠ error**; stale/unavailable explicit, never invented
  failed/zero.
- **Rails**: ALL instances are ONE reusable horizontal component
  (eligibility + slots + priority, stable target IDs, deterministic
  selection preserving existing failed-pin + latest-work behaviour). Slots
  live on stable headers / dedicated surfaces OUTSIDE clickable controls.
  Geometry: page-declared visible top/footer/dock rails using ACTUAL
  measured rects after scoped resize/scroll/layout events; the nested Studio
  webview occludes content and never z-indexes above, so rails live only in
  truly visible strips; content targets resolve to the nearest declared
  visible boundary slot. **No Rust IPC, no new poller.**
- **Motion**: finite travel + finite pose burst; RAF/timers stop when still,
  off, hidden, or reduced-motion; geometry reads bounded during travel and
  retarget, never a perpetual per-frame scan. Tiny at rest, larger for
  finite "news"; no reflow; nothing obscuring a control. `pointer-events:
  none`, inert, non-interactive; text explanation elsewhere for
  accessibility.
- **Assets**: swappable, versioned manifest with valid-state fallback;
  original lightweight local artwork only — no huge atlas, no editor
  dependency.
- **Semantics retained**: Sproket stays the DOG (issue/chat companion);
  Gaffer stays the CAT (artwork companion) — no species swap from ASCII
  shapes. Project/job correlation, bounded 64 dedupe, and
  error-resolution-only-on-matching-event all stay. Old ASCII renderer
  remains the fallback until the new path is covered; sources migrate
  gradually through a wrapper, no all-at-once rewrite.
- **Scent channel**: readable by Gonzo's TUI through the existing bounded
  feedback/event channel, including observation time and source; stale /
  unavailable explicit.
- **Coordination / boundaries**: app.js Song/Words region — wrap Statler's
  bodies only after coordinating with him, never rewrite; shared feedback
  readback — coordinate with Scooter. Do NOT touch crates/**, apps/mc/**
  (Gonzo owns TUI display via issue 24), engine/native/auth/production, or
  anything outside the owned paths.
- **C37 — Statler claims Task 10 engine step (2026-10-23)**: `crates/engine/src/words.rs` only
  (words.rs hunk from 5c4be84 + unit test `timing_disagreements_are_reported_not_chosen`).
  Commit `git commit --only crates/engine/src/words.rs`. UI port claim follows after this lands.
- **C38 — Statler Task 10 landed (2026-10-23)**: engine 381edf6 (words.rs timing_disagreements + test);
  UI 3a1c433 (Design > Words on the contract: words.js, app.js setView line + design-placement custom
  properties, index.html Design tab, style.css components segmented/field-row/transport/stage + chip word +
  panel-head item, components.json view-design). style.css/components.json released. Song sub-section heads
  were already on panel-head (no third commit). Open: el() class-from-entry (Beaker) before containers get entries.
- **C39 — Gonzo claims Task 5 (2026-10-23)**: `apps/mc/src/center.rs` and new `apps/mc/src/center/panels.rs`.
  Rows from `Config::load` of `ui/components.json`; live `signal`/`busy` only. This ledger line stays unstaged.
