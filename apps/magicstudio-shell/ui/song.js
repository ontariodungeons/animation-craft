// Song playback, lyrics and read-along for the open .magic project.
function songSection(id, title, meta, done, sectionState, body) {
  const head = window.companionBoxes.dot(done ? "ready" : "needs-attention", "", `song-${id}`);
  head.title = done ? "Ready" : "Needs work";
  return el("div", { class: "song-section", id: `song-${id}`, "data-section": id, "data-state": sectionState },
    el("div", { class: "panel-head" },
      head,
      el("span", { class: "title" }, title), el("span", { class: "meta" }, meta || "")),
    ...body);
}

function transcriptSummary(doc, ok) {
  const words = doc?.words || [];
  if (!words.length) return "No timed words yet";
  const end = doc.recording_duration_sec ?? words.at(-1).end;
  return `${words.length} words · ${clockTime(end)} · ${ok ? "timing ok" : "timing needs review"}`;
}

function clockTime(seconds) {
  const s = Math.max(0, Math.floor(seconds || 0));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}

// ---- Song playback: one audio element per project, shared by the Song header and the reader ----
const songPlayer = { audio: null, path: null };

function songAudio(path) {
  if (songPlayer.path !== path) {
    songPlayer.audio?.pause();
    songPlayer.audio = new Audio(convertFileSrc(path));
    songPlayer.path = path;
    for (const type of ["play", "pause", "ended"]) songPlayer.audio.addEventListener(type, renderSongPlay);
    songPlayer.audio.addEventListener("timeupdate", readerFollow);
  }
  return songPlayer.audio;
}

function songStop() { songPlayer.audio?.pause(); songPlayer.audio = null; songPlayer.path = null; }

function songPlayButton(path) {
  const button = el("button", { class: "icon song-play", id: "song-play", "data-path": path,
    onclick: (e) => {
      e.stopPropagation(); // play/pause never opens or closes the Song section
      const audio = songAudio(path);
      if (audio.paused) audio.play().catch((error) => toast(`Cannot play the song: ${error.message || error}`));
      else audio.pause();
    } });
  renderSongPlay(button);
  return button;
}

function renderSongPlay(button = $("song-play")) {
  if (!button?.dataset) return;
  const playing = songPlayer.path === button.dataset.path && songPlayer.audio && !songPlayer.audio.paused;
  const label = playing ? "Pause song" : "Play song";
  button.setAttribute("aria-label", label); button.title = label; button.setAttribute("aria-pressed", String(!!playing));
  button.innerHTML = playing ? ICONS.pause : ICONS.play;
}

// ---- Lyrics: the artist's sheet when the project has one, otherwise the transcript as heard ----
// One model feeds both the Song sheet and the reader, so they always show the same words.
function lyricModel(lyrics, doc, asHeard = false) {
  if (!asHeard && lyrics?.source === "canonical") return { canonical: true, sections: (lyrics.sections || []).map((s) => ({ label: s.label,
    lines: s.lines.map((l) => ({ text: l.text, start: l.start ?? null, words: l.words || [] })) })) };
  const words = (doc?.words || []).filter((w) => Number.isFinite(w.start) && Number.isFinite(w.end));
  const lines = [];
  let line = [];
  words.forEach((w, i) => {
    if (line.length && (w.start - words[i - 1].end >= 0.6 || line.length >= 10)) { lines.push(line); line = []; }
    line.push(w);
  });
  if (line.length) lines.push(line);
  return { canonical: false, sections: [{ label: null, lines: lines.map((l) => ({ text: l.map((w) => w.text).join(" "), start: l[0].start, words: l })) }] };
}

async function loadSongLyrics() {
  const project = state.project;
  try {
    const lyrics = await invoke("engine_command", { id: "song.lyrics", params: {} });
    if (state.project === project) { state.lyrics = lyrics; renderBlocks(); }
  } catch (error) { if (state.project === project) state.lyrics = null; }
}

async function attachOriginal(event) {
  const path = event?.currentTarget?.uiFiles?.[0] || await dialog.open({ title: "Attach the artist's original lyrics or liner notes" });
  if (!path) return;
  try {
    await designCommand("workshop.add", { kind: "liner_notes", paths: [path] });
    await loadSongLyrics();
    toast("Attached to liner notes; the original file is unchanged");
  } catch (error) { toast(String(error)); }
}

async function importProjectTranscript(event) {
  const path=event?.currentTarget?.uiFiles?.[0] || await dialog.open({title:'Import existing timed lyrics',filters:[{name:'Transcript JSON',extensions:['json']}]});
  if(!path)return;
  try{await designCommand('workshop.import_transcript',{path});toast('Timed lyrics imported. Review the words before creating scene pages.');}catch(error){toast(String(error));}
}

function lyricSheet(model) {
  return el("div", { class: "lyric-sheet" }, ...model.sections.map((section) => el("div", { class: "lyric-stanza" },
    section.label ? el("div", { class: "stanza-label" }, section.label) : null,
    ...section.lines.map((line) => el("div", { class: "lyric-line" }, el("span", { class: "lyric-text" }, line.text),
      line.start != null ? el("span", { class: "cue", title: "Sung at" }, clockTime(line.start)) : null)))));
}

function lyricsBody(lyrics, doc, liner, songPath) {
  const model = lyricModel(lyrics, doc);
  const original = lyrics?.original;
  const name = (path) => String(path).split("/").pop();
  if (!model.sections.some(section=>section.lines.length) && !original && !liner.length) {
    return [el('button',{id:'song-attach-original',class:'ghost',title:'Attach the original lyric sheet',onclick:attachOriginal},'Attach lyric sheet…')];
  }
  return [
    el("p", { class: "lyric-source" }, model.canonical
      ? `${lyrics.artist ? `${lyrics.artist} · ` : ""}the artist's lyric sheet${lyrics.timing ? " · timing from the recording" : ""}`
      : "From the transcript, as heard and not corrected · no artist lyric sheet attached"),
    model.canonical && lyrics.artist_note ? el("blockquote", { class: "artist-note" }, el("p", {}, lyrics.artist_note), el("cite", {}, lyrics.artist || "The artist")) : null,
    el("div", { class: "lyric-actions" },
      original?.exists
        ? el("button", { id: "song-open-original", title: original.path, onclick: () => openPath(original.path) }, `Open artist's original · ${name(original.path)}`)
        : [el("span", { class: "not-linked" }, original ? `Original not found: ${name(original.path)}` : "Original not linked"),
           el("button", { id: "song-attach-original", class: "ghost", onclick: attachOriginal }, "Attach original…")],
      model.sections.some((s) => s.lines.length) ? el("button", { id: "song-read-along", onclick: () => lyricReaderOpen(lyrics, doc, songPath) }, "Read along") : null),
    lyricSheet(model),
    el("div", { class: "group-label" }, "Liner notes"),
    liner.length ? liner.map(fileButton) : el("div", { class: "why" }, "None attached yet."),
  ];
}

// ---- Reader: the same lyric model in the right workstation; read only, follows the song ----
const lyricReader = { words: [] };

function lyricReaderOpen(lyrics, doc, songPath, asHeard = false) {
  const model = lyricModel(lyrics, doc, asHeard);
  const seek = (t) => { if (songPath && t != null) songAudio(songPath).currentTime = t; };
  const words = [];
  const body = el("div", { class: "reader-body" }, ...model.sections.map((section) => el("div", { class: "lyric-stanza" },
    section.label ? el("div", { class: "stanza-label" }, section.label) : null,
    ...section.lines.map((line) => el("p", { class: "reader-line" },
      line.start != null ? el("button", { class: "reader-time", title: songPath ? "Play from here" : "", "aria-label": `Seek to ${clockTime(line.start)}`, onclick: () => seek(line.start) }, clockTime(line.start)) : null,
      ...(line.words.length
        ? line.words.map((w) => { words.push(w); return el("span", { class: "reader-word", "data-index": String(words.length - 1), title: `${(+w.start).toFixed(2)}–${(+w.end).toFixed(2)} s` }, `${w.text} `); })
        : [el("span", {}, line.text)]))))));
  const title = model.canonical ? "Lyrics" : "Transcript";
  const pane = el("section", { id: "magic-reader", "data-drawer":"reader", "aria-label": `${title}, read only` },
    el("div", { id: "magic-reader-head" }, el("span", {}, title),
      el("span", { class: "magic-target" }, model.canonical ? "artist's sheet · read only" : "as heard · read only"),
      el("button", { id: "magic-reader-close", "aria-label": `Close the ${title.toLowerCase()}`, onclick: transcriptReaderClose }, "Close")),
    body);
  $("magic-reader")?.remove();
  $("magic-workstation")?.append(pane);
  window.workshopDrawers?.open("reader");
  lyricReader.words = words;
  readerFollow();
}

function transcriptReaderClose() { if(window.workshopDrawers?.active==="reader")window.workshopDrawers.close(); $("magic-reader")?.remove(); lyricReader.words = []; }

function readerFollow() {
  const pane = $("magic-reader");
  if (!pane || !songPlayer.audio) return;
  const t = songPlayer.audio.currentTime;
  const index = lyricReader.words.findIndex((w) => t >= w.start && t < w.end);
  pane.querySelector(".reader-word.now")?.classList.remove("now");
  if (index >= 0) pane.querySelector(`.reader-word[data-index="${index}"]`)?.classList.add("now");
}
