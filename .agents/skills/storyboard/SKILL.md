---
name: storyboard
description: Use when working on a song's storyboard in Magic Studio — reading, searching or editing scenes, adding or replacing panels and prompts, recording notes or lyric corrections, or preparing a scene for approval. The storyboard PDF is one shared document of chapters (scenes) for the user and every agent.
---

# Storyboard (Magic Studio)

A song project is one folder with an `<Song>.acproj` file. Its storyboard is ONE shared document:
`storyboard/storyboard.pdf` (portrait): page 1 lists every caption (number, time, lyric); then the
**Cast & sets** board (master prompt, locks, a card per character and location); then one page per
scene (image box, lyric cue, planned hold, visible cast, story beat, continuity lock, motion,
editable caption). A scene is one caption, or an instrumental stretch (1 s or more with no caption).
Every box is a form field: type into it, or put an image in an image box, in Magic Studio or
PDFCraft. `workshop.build_storyboard` redraws the pages and keeps the text typed into fields.

## Sources (the template)
```
<song>/
  <target>/transcript.srt       the target's captions: one scene each (from audio.analyze)
  <target>/transcript.json       timed words (what was heard; never edit)
  <target>/audiomap.json         the song's structure (energy, rolls, silences, phrases)
  <target>/STORYBOARD.md         the target's frame skeleton (music-to-video)
  storyboard/scene-N.md          status, notes, lyric corrections for scene N
  storyboard/panel-NN.png        panel N illustrates sung line N of every scene
  storyboard/panel-NN.prompt.md  the exact prompt that made panel N
  assets/characters/<name>/parts/  a character's layers
  assets/backgrounds/<name>/       a background's layers
  liner-notes/                   the artist's notes
```
Read the studio intent first: `intent.md` in the song folder, or `STUDIO.md` beside the projects.

## Commands (same for the CLI, MCP and the app)
Run `mc run -` with one JSON per line. Open the project first.
```
{"id":"project.open","params":{"path":"<song>/<Song>.acproj"}}
{"id":"storyboard.toc","params":{}}                      chapters: scene, title, page, status, lines, panels
{"id":"storyboard.read","params":{"scene":1}}            times, lyrics (and what was heard), panel + prompt per line, notes
{"id":"storyboard.search","params":{"query":"bell"}}     hits in lyrics, prompts and notes
{"id":"storyboard.edit","params":{"scene":1,"field":"panel","line":4,"value":"/path/new.png"}}
{"id":"storyboard.edit","params":{"scene":1,"field":"prompt","line":4,"value":"exact prompt text"}}
{"id":"storyboard.edit","params":{"scene":1,"field":"lyric","line":1,"value":"corrected words"}}
{"id":"storyboard.edit","params":{"scene":1,"field":"note","value":"what changed and why"}}
{"id":"audio.analyze","params":{}}                       song -> transcript.json, transcript.srt, audiomap.json, STORYBOARD.md
{"id":"storyboard.init","params":{}}                     create scene-N.md files from the captions (run audio.analyze first)
{"id":"workshop.build_storyboard","params":{}}           rebuild the PDF
```
Every edit puts the scene back to **needs review** and rebuilds the PDF. Only the user approves
(`storyboard.approve`). Agents never approve their own work.

## The loop for an agent
1. `storyboard.toc`, then `storyboard.read` the scene you were given. Work one scene at a time.
2. Make the image or text in the template's place (a panel per sung line, with its prompt).
3. Record it with `storyboard.edit`: the panel, its prompt, and a short note of what you changed.
4. Stop and report. The user reviews that scene in Magic Studio's storyboard pane.

## Rules
- Keep layers separate: a background, each character, each prop and each drawn track as its own file. One flattened image never stands in for several layers.
- Lyrics and on-screen text are animated in the video tool; follow the studio intent for image rules.
- Never edit the transcript. Record corrections as `lyric` edits.
