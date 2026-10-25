# Shared panel refactor and companions

Branch: `fix/panel-unification`
Worktree: `.worktrees/panel-unification`
Base: `5ab274ba7053f6f580988ae9ba21ea8137f1ccea`
Implementation through: `24e62387`
PRs: [#28 — shared refactor](https://github.com/ontariodungeons/animation-craft/pull/28), then [#29 — integrate into main](https://github.com/ontariodungeons/animation-craft/pull/29). Both are published, open, non-draft and mergeable. No merge has been performed.

## Implemented and verified

- Shared registration, stable authored IDs and per-instance numeric indices, parent references, positions and safe data. Live complete paging: 295 unique components with finite positions.
- Common configured panels and rails; UI, CLI and TUI resolve the same canonical state. Hidden indicators remain observable; missing state/header produces a red contract error.
- Original honey dog and orange cat enabled through the swappable manifest. All ten SVG poses decode. Dog follows errors, explicit attention and next steps; cat follows recent command activity and next steps. Idle homes, opt-out and reduced motion are covered.
- Explicit CLI/TUI attention, real command rejection and recovery, missing-registration red state, separate idle homes and shared-slot geometry passed live. Both pets fit their actual slots at 700, 941 and 1261 pixels.
- Submitted background jobs remain busy; later status polls retain their initiating component. Old job starts/completions cannot overwrite replacements. New-project attention cannot inherit prior failures or receipts.
- Normal commit hooks passed. Independent final review is clean after correcting its two findings and stale-poll follow-up.

## Style consolidation and scope

Claude's page design is retained. Pixel and hex literals outside root tokens, direct visual style writes and style attributes are zero; selector count is below the original target. Computed-style comparisons found no changes to displayed lyrics/animation components. Historical whole-app source-line targets remain exceeded; they are recorded in `evidence/refactor-acceptance.json`, not represented as passed. Further page rewriting to reduce those totals is outside Jesse's instruction to leave the UI to Claude.

No intake workflow, animation engine or new timeline was rewritten. Lyric-tag polish and the Pterodactyl creative spike remain Claude's work. No named TUI handoff recipient has been supplied; the durable interface is [tui-contract.md](tui-contract.md).

## Evidence

- [Acceptance results](evidence/refactor-acceptance.json)
- [Idle homes](evidence/pets-idle-homes.png)
- [TUI attention](evidence/pets-tui-attention.png)
- [Missing-state red](evidence/pets-missing-state-red.png)
- [Narrow layout](evidence/pets-700.png)

Idle acceptance deliberately injects idle facts in a disposable fixture; it does not claim that an empty project has completed intake. Error acceptance uses an actual rejected engine command. No user chat was sent or authored animation changed.

## Final verification and publication boundary

Final review repairs are committed in `21607e0e` and `24e62387`; the fresh reviewer confirmed no remaining findings. The final rebuilt shell passed the complete live pet acceptance and 295-component paging again. An actual failed command in fixture A followed by opening disposable fixture B and requesting attention produced B's idle underlying state plus yellow attention, with null request/receipt and no frontend errors. The task-owned preview was closed cleanly; its window and port 19963 are confirmed closed in [preview-close.json](evidence/preview-close.json). Other agents' windows and source edits were preserved.

The work is committed and published. The 146 existing baseline commits are exposed separately in #29; #28 contains only this refactor against that baseline. Merge #28 into the baseline branch first, then #29 into main. Concurrent uncommitted files are excluded and preserved.

## GitHub merge order

1. Merge [#28](https://github.com/ontariodungeons/animation-craft/pull/28) into `integration/committed-baseline-2026-10-24`.
2. Merge [#29](https://github.com/ontariodungeons/animation-craft/pull/29) into `main`.

GitHub reports both PRs CLEAN and MERGEABLE. There are no configured GitHub status checks; local verification is recorded above. No new worktree was created to publish them.
