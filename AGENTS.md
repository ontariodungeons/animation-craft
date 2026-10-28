# Working on Magic Studio with concurrent agents

Read `README.md` for the current native-app architecture and `CLAUDE.md` for the general coding guidelines. These rules keep independent
UI, engine, pipeline, and test work from overwriting one another.

## Canonical application

The application is `apps/magicstudio-shell` (Tauri). Build with `cargo build -p magicstudio-shell -p mc`; desktop menu **Magic Studio**. UI files are embedded: rebuild after edits. CLI and MCP use `mc` and the same Rust engine registry. Use an explicitly selected Studio `--control` port for live access; otherwise sessions are headless. The product is **Magic Studio**; project files use the `.magic` extension (legacy `.acproj` files are migrated with `mc exec project.migrate path=<file>`).

## Drive the app: CLI, MCP and skill

- Skill: `~/.agents/skills/magicstudio/SKILL.md` (workflow, tools, gotchas), like the other *craft apps.
- CLI: `mc help`; `mc exec <command> key=value`, or add
  `--bridge <port>` to drive an open `magicstudio-shell --control <port>` window.
- MCP: register command `mc`, args `["mcp"]` (headless engine) or
  `["--bridge", "<port>", "mcp"]` (the open window). Every engine command is a tool.

## Validation budget

- GitHub has one lightweight sanity job, capped at three minutes, with one shared
  concurrency group for the whole repository. Do not add remote Rust builds, native
  app downloads, extra jobs, agent calls or runners without Jesse’s explicit approval.

## Review on main

- Jesse reviews the app from `main`. Commit completed changes, integrate current main,
  push them to main, and rebuild the canonical app so the visible UI matches the source.
  Jesse has authorized this integration; do not leave finished fixes only in feature
  checkouts or require another chat to find and cherry-pick them. Keep PRs as review records.
- Preserve concurrent edits. Use a normal fast-forward for the main checkout; resolve
  any overlap in the owning feature checkout before updating main. Never reset or force it.
- Use one shared build directory, `<repository>/target`, and one compiler job on this
  machine. Do not create a separate multi-GB build cache for each task. Retire merged,
  inactive, clean checkouts and disposable build files; preserve unfinished source.
- Report each check you actually ran, and whether it passed, failed, or was not run.
  Do not claim an unrun check passed.

- Active agents keep their review app open and report its real PID, control port,
  branch/worktree and build identity; Claude is exempt to conserve credits. Jesse
  reviews the canonical main app. Never claim live Studio work without a matching
  open instance and native activity evidence. Save and check dirty state before updates.
- Beaker monitors completed agent work, reviews its changes and follows each PR
  through integration to main. Do not leave a handoff dependent on an old chat.

## Static image to animatable asset

- Begin with the repository image-to-animation skill and read the applicable asset
  sheet, rigging, pose, motion and animation QA instructions completely before work.
  Use the actual current native CLI contracts; flag stale instructions instead of
  silently substituting an older renderer or application.
- A few cut image portions, pieces rotating independently, unwanted overlaps or a
  rendered scene clip do not establish an editable animatable asset. The result must
  retain connected joints/part coverage through the required poses and transitions,
  with adjustable values in both Studio and the native CLI, and survive save/reopen.
- Demonstrate the requested humanoid, insect, animal and prop workflow. Keep missing
  anatomy, sheets, controls, joint continuity or unreviewed transitions marked incomplete.
  Review source images and results before assigning the next bounded agent task.

## Visible workshop agents

- Every agent works for Magic Studio under an explicit MOP role and model choice.
  Do not launch unwrapped/default agents. Reuse the existing Muppet role-planning
  project before defining new roles.
- All delegated agents run in Herdr and appear in the Workshop/TUI. Use one-word
  MOP names. Do not use hidden background subagents or create a checkout per agent.
- Reuse the canonical source/workshop and shared build cache. Record each active
  agent’s actual task and open app identity through the existing native TUI controls.
- Keep GitHub issues short. Humans supply the problem; agents supply files, exact
  commands and acceptance evidence. Do not require tags, boards, milestones or
  GitHub assignees from Jesse. Do not add GitHub push/review/check gates on your own.

## Isolate source only when needed

- The workshop limit is five checkouts total: canonical main for Jesse and the Review Desk,
  plus one shared checkout each for Creative, Animation, Production, and Backstage.
  Agents and tasks share their department checkout and build cache. Do not create another
  checkout while the workshop is above this limit. Preserve unfinished source when retiring old ones.
- Inspect `git status`, the current branch, recent commits, and `git worktree list`.
- Reuse the existing task checkout where possible. When isolation is needed, use a
  named branch and a checkout under this repository’s `.worktrees/` folder. Start from
  a committed baseline; uncommitted files in another checkout are not part of a branch.
- State your branch, worktree, intended files, and validation before editing.
- If another agent owns overlapping files, coordinate the overlap first. Do not
  restart its task, silently replace its implementation, or change its checkout.
- Share the canonical build directory; keep preview ownership explicit. Do not restart
  another agent's running app or operate its control port without coordinating it.

## Preserve ownership

- Stage explicit paths belonging to your task. Do not use blanket staging when
  other agents have changes in the same checkout.
- Do not reset, clean, force-push, or switch another agent's working directory.
- Keep generated media, credentials, machine configuration, and installed agent
  packages out of source commits. Runtime templates needed by a clean checkout
  are source dependencies and must be versioned deliberately.
- Preserve uncommitted work even if a baseline commit or PR has been requested.
  A baseline snapshot records a point in time; it does not claim unfinished
  features have passed acceptance tests.

## Verify behavior, not optimistic metadata

- Generate small test fixtures in temporary directories. Do not place all test
  assertions inside `if sample_file_exists`: that passes without testing anything.
- Assert the actual state change, decoded output, or output pixels relevant to the
  command. A JSON field's presence or an old file on disk does not prove success.
- Test failure behavior: unavailable tools, invalid parameters, missing or stale
  outputs, and process exit status. Do not weaken tests just to get a green run.
- For UI changes, check narrow layouts, selection synchronization, tooltips,
  automation IDs, and a rendered preview. Keep unavailable measurements distinct
  from measured zero and manual review distinct from automated validation.
- Use a preview workspace you control for every UI change. Review the actual app through mc
  at 700, 941 and 1261 logical pixels before integrating it into main. Capture and inspect the
  screens with drawers open and closed; identify the preview by its footer port and build.
- Run checks appropriate to your changes. Identify inherited failures against the
  base revision and report their owner; do not hide them or duplicate another
  agent's active fix. Do not bypass required PR checks.
- Add meaningful tests for the change, run the checks that match it, and make a normal
  `git commit`. Build the app as needed to keep the visible preview open. Report the
  real command and whether it passed, failed, or was not run.

## Commit before handing work to another agent

A handoff must include:

1. Branch and worktree path.
2. Base commit and new commit SHA(s).
3. PR URL, or an explicit statement that no PR exists yet.
4. Changed files, test commands/results, and preview evidence.
5. Known blockers, dependencies, and any uncommitted work that must be preserved.

Do not tell another agent to merge a branch while your feature exists only as
uncommitted files. Do not treat a local patch or a running preview as a commit.

## Integrate through a reviewable PR

- Fetch the latest remote state before pushing or merging.
- Bring current main into your own branch and inspect semantic conflicts as well
  as textual conflicts. Updated engine contracts may require updated UI callers
  and help text even when Git reports a clean merge.
- Keep a feature PR scoped to the feature. If the application baseline is missing,
  establish it separately with the user's authorization before opening the PR.
- Create the PR with accurate evidence and unresolved limitations. For this project, Jesse has requested integration to main; finish that integration
  rather than stopping at PR creation. Keep required checks and unresolved results visible.
- Updating an actively used main checkout must be a normal safe fast-forward that
  preserves concurrent edits. If Git refuses, stop and report the overlap; do not
  force the working files to match the remote.
