# Recovered image-to-animation workflow

Recovered 2026-10-24. This preserves Jesse's original requirement separately from an agent's proposed backend. The newer requirement is to prove the shared process on a humanoid, insect, animal and prop, with adjustable values and the approved UI architecture.

## Original user message, verbatim

Source: Grok session `01a16e65-5099-7572-9be0-010a885aad89`, `chat_history.jsonl:103`, the tool result of a plan review. It was not a normal user-role log entry, which is why the initial user-message search missed it.

> There's more steps to understand, and maybe you're saying animate.prepare does this? It's already been proven and done on this machine, but 3 agents also tried ways that didn't work. Results must be verified by all the working animation controls working. If we can't find it we could locate the earlier app when it was called animationcraft, but I believe I saw it working here with 80% of the facial and motion, wing movements, arm motions,  and antanea bending and swaying for idle
>
> Animate.prepare steps:
> 1. choose static image to animate
> 2. script locate template scripts that will create the required sheets: turnaround, poses (walk, run, jump, etc), limps, and then I think we need a joints and/or skeletal sheet
> 3. Run image_gen yourself from the cli reponse, or ask gaffer (I think I need to change that name, I can't remember it) or use the cli to invoke something like mc animate <image>. A terminal wizard process that you can -y and skip should lead you to ask gaffer as default (hit ok?)
> 3. Asset sheets arrive and need another cli command that calls a script that will break all the images into their individual files.
> - Cleaning what it can overall from each sheet and making it transparent first
> - cutting/cropping/magic (ai cli command can likely crap nearly the exact image) each image
> - return cleaned sets of images ready to wire into animation
> 4. The next script knows the exact expected names of limbs and basic moving parts (treat a bee or even a swaying flower on initial load as if it has limbs, or have a setting for humanoid and custom. For this you must use ponytail architecture and logic to inherit a reusable method or class that allows for adding specific cropping for this new asset type "fish", "flower", "bee".
> - a cli command suggested after the initial animation
> - the animation subject and images load in the main ui panel,
> - the ui config shows green to yellow depending
>
> And that's how it goes. It's a lot to pack in 3-4 scripts but the automation is nearly scripted, I need to run tests on lots of animation subjects and open every project and let the ui backend show what works and not as you go

## Source locations

- Grok chat: `/home/jesse/.grok/sessions/%2Frun%2Fmedia%2Fjesse%2FSpeedy%2FWork%2Fomhas-animation/01a16e65-5099-7572-9be0-010a885aad89/chat_history.jsonl`.
- Grok plan: same directory, `plan.md`. That plan proposed restoring live Blender; the exact user workflow above does not establish that Blender is the best backend.
- Rizzo chat: `/home/jesse/.claude/projects/-run-media-jesse-Speedy-Work-omhas-animation/03fc8a53-4d50-42c7-b40d-ba8ef42209a5.jsonl`. Lines 5000 (Track 15 handoff), 5436 (proposed second sheet workflow), 6255 (acknowledged duplication/missing controls), 6466 (stop and historical bee findings).
- Earlier animator-order plan: `docs/superpowers/plans/2026-10-23-character-motion-pipeline.md`.
- Architecture: `docs/architecture/panel-contract.html`, with newer binding corrections in `docs/architecture/implementation-plan.md`. Old agent names do not assign today's work.
- Song: `/run/media/jesse/Speedy/media/three-times-around/project/Three Times Around.magic`; current creative authority: adjacent `intent.md`.
- Rizzo's checkout: `/run/media/jesse/Speedy/Work/omhas-animation/.worktrees/song-lyrics-reader`, branch `feature/lyrics-design-timeline`; working binary `target/debug/mc`, live bridge 19971. Do not restart or switch it for this review.
- Historical layered bee spec: `/run/media/jesse/Speedy/media/bumble-bee/project/animationcraft-bee/bee_motion_spec_v2.json`.
- Bee reconstruction evidence: `/home/jesse/Documents/Codex/2026-10-24/task/reconstruct_bee.py`, `verify_bee.py`, `bee-reconstruction/cli-map.json`. These are evidence/fixtures, not the reusable production interface to copy per subject.

## Current plan

[Track 15 command and script sequence](track15-animation-sequence.md) separates existing commands, Rizzo's branch-only additions, proposed adapters, and the checks needed before any method is selected. No animation/UI implementation was made during recovery.
