# Statler — current system and next work

Jesse wants a fresh view of the current product, not a retelling of changes.

## Product
Magic Studio is one workshop connecting existing digital tools. Tauri shell UI + Rust shared registry exposed by mc/CLI/MCP. Project files are .magic. Main source: /run/media/jesse/Speedy/Work/omhas-animation, main currently 2281160. Completed features must reach main promptly. Existing worktrees allowed; NO new worktrees or copied build caches. Keep working in your existing checkout until a clean boundary; Fozzie handles integration. No blanket staging or rewriting others work.

## User workflow
Audio + artist lyrics -> HyperFrames word alignment and timed scene skeleton with blank images -> reference artwork -> rough part breakout and motion -> static proof posed/aligned over original storyboard reference -> Jesse approval -> Studio placement/timing polish. Do not fine-time before general image/motion approval.
Three linked saves: editable animation collection/version; storyboard static proof with source revision, original underneath and approval; HyperFrames scene instance with timing/trim/transform/order. Fozzie owns shared save mapping; it is NOT yet implemented. Hidden alignment reference must remain preserved and invisible in final picture. Visible animated lyrics/drawings are separate tracks. Artist original handout + liner note are authoritative; ASR must not overwrite them.

## Your lane
Own HyperFrames/GSAP Words/Artwork library integration and Design UI. Song header play/pause; expanded artist lyric sheet by verse/chorus, discreet timing; artist note and Open artist original link; right-side read-only reader. Jesse approved this presentation. Omawrite is standalone with no embed API; no fake embedding. Preserve canonical text; do not silently fix timing.
Your existing checkout: .worktrees/song-lyrics-reader, branch feature/song-lyrics-reader. At handoff HEAD550162d, previous5c4be84 (Design Words UI), a2235ea baseline merge. Only generated frontend/bun.lock untracked. These two new UI commits are not yet confirmed integrated. Earlier9559ef1/3fc6a0d already in main as17668a1/6ddc530.

## Existing contracts
song.lyrics read-only returns canonical sections/occurrences/printed+ sung IDs, source handout, verbatim note, timing/mismatches. words.align/read/save/compose exist. Source design/words.json; source_revision guarded saves; text/timing immutable in styling save. Compose writes only derived subcomposition and returns composition_id/src/file/hash/revision/duration/anchor/dimensions. Repeated instance IDs isolate timeline roots. Source times are song seconds; compose subtracts anchor once. data-media-start is not assumed valid for nested DIV; current upstream has data-playback-start, pinned runtime needs capability confirmation. Production HF0.8.166; your scratch check used0.8.181. Do not upgrade production silently.

## Other ownership
Scooter all Animation controls, export/pose/native dialogs. Waldorf native render backends/order/MLT. Gonzo CLI/TUI routing safety; communicate through issue24 only. Beaker marketing/PhotoCraft/EffectCraft. Bunsen intended Studio theme/auth coding, previously blocked. Sam concise commit ledger; no endless audits. Fozzie film/design/project and PDF saves, integration/save mapper.

## Production
/run/media/jesse/Speedy/media/hey-ho-little-fish/project/Hey Ho Little Fish.magic. Fozzie owns live19927; do not change its project or save production. Your stale19951 preview was closed. No need to launch another app to plan. Reference assets stay original. Native Blender/Synfig render, OpenToonz dispatch (runtime missing), MLT cuts/audio and ordering commands exist. Reuse tools before custom scripting.

## Next
1. Read current main UI/commands and your two final commits; send concise committed handoff for5c4be84/550162d (behavior, files, checks, limits), no repeat research.
2. Continue usable Words/Artwork integration against actual library contracts, coordinate mapped source/instance interfaces with Fozzie. A compose file alone is NOT round trip.
3. Keep tasks concrete; no repeated visual audits or project-specific tests. Jesse judges visuals. Normal commit hooks; no bypass. Save small coherent commits and send SHA promptly.

Issues: https://github.com/ontariodungeons/animation-craft/issues/26 (Words/Artwork), #23 native tools, #24 Backstage.
