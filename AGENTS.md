# Working on AnimationCraft with concurrent agents

Read `README.md` for the current native-app architecture and `CLAUDE.md` for the general coding guidelines. These rules keep independent
UI, engine, pipeline, and test work from overwriting one another.

## Canonical application

The application is `apps/animationcraft-shell` (Tauri). Build with `cargo build -p animationcraft-shell`; desktop menu **AnimationCraft Studio**. UI files are embedded: rebuild after edits. CLI and MCP use `animationcraft-cli` and the same Rust engine registry. Use an explicitly selected Studio `--control` port for live access; otherwise sessions are headless.

## Drive the app: CLI, MCP and skill

- Skill: `~/.agents/skills/animationcraft/SKILL.md` (workflow, tools, gotchas), like the other *craft apps.
- CLI: `animationcraft-cli help`; `animationcraft-cli exec <command> key=value`, or add
  `--bridge <port>` to drive an open `animationcraft-shell --control <port>` window.
- MCP: register command `animationcraft-cli`, args `["mcp"]` (headless engine) or
  `["--bridge", "<port>", "mcp"]` (the open window). Every engine command is a tool.

## Start with an isolated checkout

- Inspect `git status`, the current branch, recent commits, and `git worktree list`.
- Use a separate named branch and worktree for each task. Start from a committed
  baseline; uncommitted files in another checkout are not part of a branch.
- State your branch, worktree, intended files, and validation before editing.
- If another agent owns overlapping files, coordinate the overlap first. Do not
  restart its task, silently replace its implementation, or change its checkout.
- Keep build outputs and preview instances local to your worktree. Do not restart
  another agent's running app or operate its control port.

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
- Run checks appropriate to your changes. Identify inherited failures against the
  base revision and report their owner; do not hide them or duplicate another
  agent's active fix. Do not bypass required PR checks.
- During editing, use `cargo check --workspace` to verify compilation. Do not run
  `cargo test` manually in a loop; `.githooks/pre-commit` (`git config core.hooksPath .githooks`)
  runs the Rust check and test suite automatically when you `git commit`. Never
  bypass hooks with `--no-verify`.

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
- Create the PR with accurate evidence and unresolved limitations. Creating a PR
  does not authorize merging it; merge only when the user requests it and checks
  permit it.
- Updating an actively used main checkout must be a normal safe fast-forward that
  preserves concurrent edits. If Git refuses, stop and report the overlap; do not
  force the working files to match the remote.
