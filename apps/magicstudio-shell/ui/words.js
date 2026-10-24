// Design > Words: the visible lyric track, edited one verse/chorus at a time.
// Words and timing come from the artist's sheet and the hidden transcript reference; this panel
// changes only style, emphasis and motion (words.save). The stage plays the finished HyperFrames
// video itself, one section at a time, through the preview player's control messages; its
// lyric clips come from words.studio_track. One project, one clock, the song's own: the lyrics
// (left) and the film roll (right) run down the page on it, and the stage, the section's lanes and
// what goes on them stay pinned in the centre.
const wordsPanel = { read: null, align: null, studio: null, selected: null, word: null, pending: new Map(), styleEdits: {}, stage: null, time: 0, playing: false, size: null, lanes: null, lane: "text", server: null, player: null, show: "full", offset: 0, timeLabel: null, effects: null };
// The word animation library: what each move does in the composition template.
const WORD_ACTIONS = [["none", "Still", "Stays where it lands"], ["wave", "Wave", "Rocks side to side"], ["tiny", "Small", "Shrinks to two-thirds"],
  ["swim", "Swim", "Drifts along with bubbles"], ["no", "Shake", "Shakes like a no"], ["cry", "Droop", "A tear falls from it"],
  ["big", "Grow", "Grows, with drops splashing above"], ["float", "Float", "Floats up and tilts"]];
const WORD_ENTRANCES = [["pop", "Pop", "Springs in from nothing"], ["fade", "Fade", "Fades in"], ["rise", "Rise", "Slides up into place"], ["none", "Cut", "Appears at once"]];
const WORD_EXITS = [["fade", "Fade", "Fades out"], ["drop", "Drop", "Falls away"], ["none", "Cut", "Holds until its clip ends"]];
const wordsCommand = (command, params = {}) => invoke("engine_command", { id: command, params });

const wordsSongPath = () => state.data?.status?.stages?.find((s) => s.id === "song")?.inputs?.[0]?.path;
const wordsCurrent = () => wordsPanel.read?.document?.animations?.find((a) => a.animation_id === wordsPanel.selected);
const wordsMismatch = (id) => wordsPanel.align?.words?.find((w) => w.word_id === id && w.status !== "match");
const wordsDisagree = (id) => wordsPanel.align?.timing_disagreements?.find((d) => d.word_id === id);
// The same printed section in the finished video: its span on the song clock and its lyric clips.
const wordsStudioSection = (anim) => wordsPanel.studio?.sections?.find((s) => s.section === anim?.scope?.section && s.occurrence === anim?.scope?.occurrence);
const wordsClip = (id) => wordsPanel.studio?.sections?.flatMap((s) => s.clips).find((c) => c.word_id === id);
const wordsClock = (t) => `${Math.floor(t / 60)}:${(t % 60).toFixed(1).padStart(4, "0")}`;
// The one indicator, from the box state; the title says why.
const wordsDot = (state, title) => Object.assign(window.companionBoxes.dot(state), { title });

function wordsLabel(anim) {
  const s = anim.scope || {};
  const name = String(s.section || "lyrics").replace(/_(\d+)$/, " $1").replace(/_/g, " ");
  return `${name[0].toUpperCase()}${name.slice(1)}${s.section === "chorus" || s.occurrence > 1 ? ` ${s.occurrence}` : ""}`;
}

async function wordsOpen() {
  const view = $("view-design");
  if (!view) return;
  view.replaceChildren(el("p", { class: "panel-empty inline" }, "Loading the words…"));
  try {
    const [read, align, studio] = await Promise.all([
      wordsCommand("words.read"),
      wordsCommand("words.align"),
      wordsCommand("words.studio_track").catch((error) => ({ error: String(error).replace(/^.*?: /, "") })),
    ]);
    wordsPanel.read = read;
    wordsPanel.align = align;
    wordsPanel.studio = studio;
    if (!wordsCurrent()) wordsPanel.selected = read.document.animations[0]?.animation_id || null;
    wordsRender();
    wordsVideoRange();
  } catch (error) {
    view.replaceChildren(el("div", { class: "panel-empty boxed" },
      el("p", {}, "Words starts from the artist's lyric sheet."),
      el("p", { class: "muted" }, String(error).replace(/^.*?: /, "")),
      el("p", { class: "muted" }, "Attach the original in Project › Song › Lyrics & notes.")));
  }
}

function wordsSelect(id) {
  if (wordsPanel.pending.size || Object.keys(wordsPanel.styleEdits).length) {
    if (!confirm("Discard unsaved word changes?")) return;
  }
  wordsPanel.pending.clear(); wordsPanel.styleEdits = {}; wordsPanel.word = null; wordsPanel.selected = id;
  wordsRender();
  wordsVideoRange();
}

// The effective value of a word field: the pending edit if any, else the saved value.
function wordsValue(word, key) {
  const edit = wordsPanel.pending.get(word.word_id);
  return edit && key in edit ? edit[key] : word[key];
}

function wordsSetField(word, key, value) {
  const edit = { ...(wordsPanel.pending.get(word.word_id) || {}) };
  if (value === word[key]) delete edit[key]; else edit[key] = value;
  if (Object.keys(edit).length) wordsPanel.pending.set(word.word_id, edit); else wordsPanel.pending.delete(word.word_id);
}

function wordsEdit(word, key, value) {
  wordsSetField(word, key, value);
  wordsRender();
}


// The registry's text effects for the section, as cards under the lanes.
function wordsEffects(anim) {
  const current = anim.effects?.text?.name;
  const registry = wordsPanel.effects?.kinds?.text?.effects || [];
  if (!wordsPanel.effects) {
    wordsPanel.effects = { loading: true };
    wordsCommand("words.effects", { kind: "text" }).catch((error) => ({ error: String(error).replace(/^.*?: /, "") }))
      .then((found) => { wordsPanel.effects = found; if (state.view === "design") wordsRender(); });
  }
  return el("div", { class: "words-effects" },
    current ? el("button", { class: "effect", "data-effect": "", onclick: () => wordsApplyRegistry(null) },
      el("strong", {}, `Using ${current}`), el("span", { class: "muted" }, "Click to remove this effect.")) : null,
    wordsPanel.effects?.error ? el("p", { class: "muted" }, wordsPanel.effects.error)
      : wordsPanel.effects?.loading ? el("p", { class: "muted" }, "Finding effects…") : null,
    ...registry.filter((e) => e.name !== current).map((e) => el("button", { class: "effect", "data-effect": e.name, title: (e.tags || []).join(" · "), onclick: () => wordsApplyRegistry(e.name) },
      el("strong", {}, e.title || e.name), el("span", { class: "muted" }, e.description || ""))));
}

// Put a registry effect on the section's text track (null takes it off): install it if needed,
// save it with the expected revision, compose the section and show the edited result.
async function wordsApplyRegistry(name) {
  const anim = wordsCurrent();
  if (!anim) return;
  if (wordsPanel.pending.size || Object.keys(wordsPanel.styleEdits).length) return toast("Save or discard the word edits first");
  try {
    if (name) { toast(`Installing ${name}…`); await wordsCommand("words.effect_install", { name }); }
    const result = await wordsCommand("words.save", { animation_id: anim.animation_id, expected_source_revision: anim.source_revision,
      origin: "magicstudio-design", effects: { text: name ? { name } : null } });
    if (result.conflict) { toast("Someone changed this section first; reloaded the latest"); return wordsOpen(); }
    await wordsCommand("words.compose", { animation_id: anim.animation_id });
    wordsPanel.show = name ? "edited" : "full";
    wordsPanel.stage = null;
    toast(name ? `${name} on ${wordsLabel(anim)}` : `Effect removed from ${wordsLabel(anim)}`);
    await wordsOpen();
  } catch (error) { toast(String(error).replace(/^.*?: /, "")); }
}

// The song clock of the page. A word sits at its clip in the video (else the sheet's own time); a
// section spans its stretch of the video (else its words).
const wordsAt = (w) => wordsClip(w.word_id)?.start ?? +w.start;
function wordsSpan(anim) {
  const video = wordsStudioSection(anim);
  return video ? { start: video.start, end: video.end } : { start: Math.min(...anim.words.map(wordsAt)), end: Math.max(...anim.words.map((w) => +w.end)) };
}
function wordsLines(anim) {
  const lines = [];
  for (const w of anim.words) { if (lines.at(-1)?.id !== w.line_id) lines.push({ id: w.line_id, words: [] }); lines.at(-1).words.push(w); }
  return lines;
}
// A timeline item's song time and length, as data; the timeline rule turns seconds into height.
function wordsPlace(node, at, len) {
  node.style.setProperty("--at-s", at.toFixed(3));
  if (len != null) node.style.setProperty("--len-s", Math.max(0, len).toFixed(3));
  return node;
}
// Every sung line of every section, as [section, line index, line, start, length].
function wordsRows(animations) {
  return animations.flatMap((a) => {
    const span = wordsSpan(a), lines = wordsLines(a);
    return lines.map((line, j) => { const at = wordsAt(line.words[0]); return [a, j, line, at, (lines[j + 1] ? wordsAt(lines[j + 1].words[0]) : span.end) - at]; });
  });
}
const wordsBand = (a, tag, extra = {}) => { const span = wordsSpan(a);
  return wordsPlace(el(tag, { class: `words-band${a.animation_id === wordsPanel.selected ? " active" : ""}`, "data-kind": a.scope?.section === "chorus" ? "chorus" : "verse", ...extra },
    el("strong", {}, wordsLabel(a)), el("span", { class: "meta" }, wordsClock(span.start))), span.start, span.end - span.start); };

function wordsChip(anim, w) {
  const heard = wordsMismatch(w.word_id);
  const action = wordsValue(w, "action"), disagree = wordsDisagree(w.word_id);
  const chip = el("button", { class: `chip word${wordsPanel.word === w.word_id ? " active" : ""}`, "data-word-id": w.word_id, "data-action": action,
    title: `${w.text} · ${(+w.start).toFixed(2)}–${(+w.end).toFixed(2)} s${heard ? ` · heard as “${heard.heard ?? "nothing"}”` : ""}${disagree ? ` · measured alignment says ${(+disagree.measured[0]).toFixed(2)} s` : ""}`,
    onclick: (event) => {
      event.stopPropagation();
      if (anim.animation_id !== wordsPanel.selected) wordsSelect(anim.animation_id);
      if (anim.animation_id !== wordsPanel.selected) return;
      wordsPanel.word = w.word_id; wordsPanel.lane = "motion"; wordsRender();
      const clip = wordsClip(w.word_id); if (clip) wordsSeekSong(clip.start);
    } },
    w.text, action !== "none" ? el("span", { class: "chip-action" }, WORD_ACTIONS.find((x) => x[0] === action)?.[1] || action) : null);
  // Marks are states of the word, drawn by the chip's word look.
  if (heard) chip.dataset.heard = "differs";
  if (disagree) chip.dataset.timing = "differs";
  if (wordsValue(w, "emphasis")) chip.dataset.emphasis = "";
  return chip;
}

// Left: the lyrics on the song clock, each section a band and each sung line its words.
function wordsTimeline(animations, end) {
  return wordsPlace(el("div", { class: "words-tl", "aria-label": "Lyrics on the song timeline" },
    ...animations.map((a) => wordsBand(a, "button", { "data-section": a.animation_id, onclick: () => a.animation_id === wordsPanel.selected || wordsSelect(a.animation_id) })),
    ...wordsRows(animations).map(([a, j, line, at, len]) => wordsPlace(el("div", { class: `words-line${j ? "" : " first"}` }, ...line.words.map((w) => wordsChip(a, w))), at, len)),
    el("span", { class: "words-tl-head" })), 0, end);
}

// Right: the film roll, level with the lyrics. Each sung line is a frame tagged with what it does.
function wordsRoll(animations, end) {
  const ticks = [];
  for (let t = 0; t < end; t += 10) ticks.push(wordsPlace(el("span", { class: "words-tick" }, wordsClock(t).replace(/\.\d$/, "")), t));
  return wordsPlace(el("div", { class: "words-tl words-roll", "aria-label": "Film roll of the song" }, ...ticks,
    ...animations.map((a) => wordsBand(a, "div")),
    ...wordsRows(animations).map(([a, j, line, at, len]) => {
      const moves = line.words.filter((w) => wordsValue(w, "action") !== "none").length;
      return wordsPlace(el("button", { class: `words-frame${j ? "" : " first"}${a.animation_id === wordsPanel.selected ? " active" : ""}`, title: line.words.map((w) => w.text).join(" "),
        onclick: () => { if (a.animation_id !== wordsPanel.selected) wordsSelect(a.animation_id); if (a.animation_id === wordsPanel.selected) wordsSeekSong(at); } },
        el("span", { class: "words-tag", "data-track": "text" }, a.effects?.text?.name || "As in the video"),
        moves ? el("span", { class: "words-tag", "data-track": "motion" }, `${moves} moving`) : null), at, len);
    }),
    el("span", { class: "words-tl-head" })), 0, end);
}

// Centre, under the stage: the section's minimal lanes. Clip length belongs to the Studio, so a
// lane shows timing and picks what goes below it; it never moves or resizes a clip.
function wordsLanes(anim) {
  const section = wordsStudioSection(anim);
  if (!section) return null;
  const span = section.end - section.start;
  const at = (t) => `${(100 * (t - section.start) / span).toFixed(3)}%`;
  const block = (text, start, duration, wordId) => {
    const node = el("button", { class: "lane-block", title: `${text} · ${wordsClock(start)}`, "data-word-id": wordId, onclick: () => wordsSeekSong(start) }, text);
    node.style.setProperty("--start", at(start)); node.style.setProperty("--len", `${(100 * duration / span).toFixed(3)}%`);
    return node;
  };
  const lane = (key, name, current, blocks) => el("div", { class: "lane", "data-track": key, "aria-selected": String(wordsPanel.lane === key), onclick: () => { wordsPanel.lane = key; wordsRender(); } },
    el("span", { class: "lane-name" }, name, el("span", { class: "meta" }, current)),
    el("div", { class: "lane-track" }, ...blocks, el("span", { class: "lane-head" })));
  const moving = anim.words.filter((w) => wordsValue(w, "action") !== "none" && wordsClip(w.word_id));
  wordsPanel.lanes = el("div", { class: "words-lanes", "aria-label": `${wordsLabel(anim)} lanes` },
    lane("text", "Text", anim.effects?.text?.name || "As in the video", section.clips.map((c) => block(c.text, c.start, c.duration, c.word_id))),
    lane("motion", "Motion", `${moving.length} moving`, moving.map((w) => { const clip = wordsClip(w.word_id);
      return block(WORD_ACTIONS.find((x) => x[0] === wordsValue(w, "action"))?.[1] || wordsValue(w, "action"), clip.start, clip.duration, w.word_id); })));
  wordsNow(wordsPanel.time);
  return wordsPanel.lanes;
}

// The playhead, on every timeline of the page; while playing, the page keeps it in view.
function wordsNow(t) {
  const view = $("view-design"), section = wordsStudioSection(wordsCurrent());
  view?.style.setProperty("--now-s", t.toFixed(3));
  if (section && wordsPanel.lanes) wordsPanel.lanes.style.setProperty("--at", `${(100 * (Math.min(section.end, Math.max(section.start, t)) - section.start) / (section.end - section.start)).toFixed(3)}%`);
  const tl = view?.querySelector(".words-tl");
  if (!wordsPanel.playing || !tl) return;
  const y = tl.offsetTop + tl.clientHeight * t / Number(tl.style.getPropertyValue("--len-s")) - view.clientHeight / 2;
  if (Math.abs(view.scrollTop - y) > view.clientHeight / 4) view.scrollTop = y;
}

function wordsRender() {
  const view = $("view-design");
  const anim = wordsCurrent();
  const animations = wordsPanel.read?.document?.animations || [];
  if (!anim) { view.replaceChildren(el("p", { class: "panel-empty inline" }, "No lyric sections yet.")); return; }
  const end = Math.max(...animations.map((a) => wordsSpan(a).end)) + 4;

  const word = anim.words.find((w) => w.word_id === wordsPanel.word);
  const inspector = word ? el("div", { class: "words-inspector", "aria-label": `Motion for ${word.text}` },
    el("div", { class: "group-label" }, `“${word.text}” · pick how it moves`),
    wordsLibrary(word, "action", "Animation", WORD_ACTIONS),
    wordsLibrary(word, "entrance", "Comes in", WORD_ENTRANCES),
    wordsLibrary(word, "exit", "Goes out", WORD_EXITS),
    el("label", { class: "field-row" },
      (() => { const box = el("input", { type: "checkbox", onchange: (e) => wordsEdit(word, "emphasis", e.target.checked) }); box.checked = !!wordsValue(word, "emphasis"); box.defaultChecked = !!word.emphasis; return box; })(), "Emphasis"))
    : el("p", { class: "panel-empty inline" }, "Choose a word on the left to set what it does.");

  const size = wordsPanel.styleEdits.size ?? anim.style?.size ?? 120;
  const sizeInput = el("input", { type: "range", min: "60", max: "220", step: "2", "aria-label": "Word size",
    oninput: (e) => { wordsPanel.styleEdits.size = Number(e.target.value); if (wordsPanel.styleEdits.size === (anim.style?.size ?? 120)) delete wordsPanel.styleEdits.size; wordsRender(); } });
  sizeInput.value = String(size);
  sizeInput.defaultValue = String(anim.style?.size ?? 120); // only a moved slider is an unsaved edit
  const changes = wordsPanel.pending.size + Object.keys(wordsPanel.styleEdits).length;
  const save = el("button", { id: "words-save", "data-commands": "words.save", onclick: wordsSave }, changes ? `Save ${changes} change${changes > 1 ? "s" : ""}` : "Saved");
  save.disabled = !changes;

  const main = el("section", { id: "words-main", "aria-label": `${wordsLabel(anim)} words` },
    el("div", { id: "words-main-head" }, el("strong", { class: "panel-title" }, wordsLabel(anim))),
    el("div", { class: "words-head" },
      el("span", { class: "meta" }, `${anim.words.length} words · ${(anim.authored_duration || 0).toFixed(1)} s · rev ${anim.source_revision}${wordsPanel.read.saved ? "" : " · not saved yet"}`),
      el("label", { class: "field-row" }, "Size", sizeInput),
      ...wordsStyleFields(anim),
      save),
    wordsPanel.read.reference_changed ? el("p", { class: "not-linked" }, "The timing reference changed since this was saved; check the words before saving.") : null,
    wordsStage(anim), wordsLanes(anim),
    wordsPanel.lane === "text" ? el("div", { class: "group-label" }, `Text effects for ${wordsLabel(anim)} · HyperFrames`) : null,
    wordsPanel.lane === "text" ? wordsEffects(anim) : inspector);
  view.replaceChildren(
    el("aside", { id: "words-lyrics" }, el("div", { id: "words-lyrics-head" }, el("strong", { class: "panel-title" }, "Lyrics · song timeline")), wordsTimeline(animations, end)),
    main,
    el("aside", { id: "words-film" }, el("div", { id: "words-film-head" }, el("strong", { class: "panel-title" }, "Film roll")), wordsRoll(animations, end)));
  wordsNow(wordsPanel.time);
  wordsFitTimeline();
  wordsFit();
}

// The section's other style settings words.save accepts: font, colours and background.
function wordsStyleFields(anim) {
  const field = (key, label, type, fallback) => {
    const saved = anim.style?.[key] ?? fallback;
    const input = el("input", { id: `words-style-${key}`, type, "aria-label": label, oninput: (e) => {
      if (e.target.value === saved) delete wordsPanel.styleEdits[key]; else wordsPanel.styleEdits[key] = e.target.value; } ,
      onchange: () => wordsRender() });
    input.value = wordsPanel.styleEdits[key] ?? saved;
    input.defaultValue = saved; // only a changed field is an unsaved edit
    return el("label", { class: "field-row" }, label, input);
  };
  return [field("font", "Font", "text", ""), field("color", "Colour", "color", "#ffffff"), field("emphasis_color", "Emphasis", "color", "#ffd078"),
    field("outline_color", "Outline", "color", "#173b43"), field("background", "Background", "text", "")];
}

// The stage is the finished video, kept across re-renders so editing never reloads it.
function wordsStage(anim) {
  const section = wordsStudioSection(anim);
  if (wordsPanel.stage && wordsPanel.stage.dataset.animation === anim.animation_id) return wordsPanel.stage;
  const url = serverUrl();
  wordsPanel.server = url || null;
  if (!section || !url) {
    wordsPanel.stage = null;
    return el("p", { class: "panel-empty inline" }, !url ? "Start the Studio server to play the video here."
      : wordsPanel.studio?.error ? `The video's lyric clips could not be read: ${wordsPanel.studio.error}` : "The video has no lyric clips for this section.");
  }
  const scrub = el("input", { type: "range", id: "words-scrub", min: String(section.start), max: String(section.end), step: "0.01", "aria-label": "Video time",
    oninput: (e) => wordsSeekSong(Number(e.target.value)) });
  scrub.value = String(section.start); scrub.defaultValue = scrub.value; // a view control, never an unsaved edit
  // The preview player of the HyperFrames server, same project id as the Studio link.
  const server = new URL(url);
  const project = server.hash.split("/")[1] || "hyperframes";
  wordsPanel.player = `${server.origin}/api/projects/${project}/preview`;
  const frame = el("iframe", { id: "words-video-frame", title: "The finished video", allow: "autoplay" });
  frame.addEventListener("load", wordsVideoRange);
  const view = (show, label) => el("button", { role: "tab", "data-show": show, "aria-selected": String(wordsPanel.show === show), onclick: () => wordsShow(show) }, label);
  wordsPanel.stage = el("div", { class: "words-preview", "data-animation": anim.animation_id },
    el("div", { class: "segmented", role: "tablist", "aria-label": "What the stage shows" },
      view("full", "Full video"), view("lyrics", "Lyrics only"), anim.effects?.text ? view("edited", `Edited · ${anim.effects.text.name}`) : null),
    el("div", { id: "words-preview" }, frame),
    el("div", { class: "transport" },
      wordsPanel.playButton = el("button", { class: "icon", "aria-label": "Play this section", title: "Play this section of the video", onclick: () => wordsVideo(wordsPanel.playing ? "pause" : "play"), html: ICONS.play }),
      scrub,
      wordsPanel.timeLabel = el("span", { class: "meta" }, `${wordsClock(section.start)} – ${wordsClock(section.end)}`)));
  if (wordsPanel.show === "edited" && !anim.effects?.text) wordsPanel.show = "full";
  wordsShow(wordsPanel.show, frame);
  return wordsPanel.stage;
}

// The stage shows the whole video, a derived copy with only its lyric clips (words.lyrics_view),
// or the section as edited (its composition from words.compose). The edited section starts at its
// song_anchor, so its times are offset from the song's.
async function wordsShow(show, frame = $("words-video-frame")) {
  const anim = wordsCurrent();
  wordsPanel.show = show;
  wordsPanel.stage?.querySelectorAll('[role="tab"]').forEach((tab) => tab.setAttribute("aria-selected", String(tab.dataset.show === show)));
  if (!frame || !wordsPanel.player || !anim) return;
  let src = wordsPanel.player;
  wordsPanel.offset = show === "edited" ? anim.song_anchor || 0 : 0;
  try {
    if (show === "lyrics") src = `${wordsPanel.player}/comp/${(await wordsCommand("words.lyrics_view")).src}`;
    if (show === "edited") src = `${wordsPanel.player}/comp/${(await wordsCommand("words.compose", { animation_id: anim.animation_id })).src}`;
  } catch (error) { toast(String(error).replace(/^.*?: /, "")); return wordsShow("full", frame); }
  if (frame.getAttribute("src") !== src) frame.setAttribute("src", src);
  else wordsVideoRange();
}

// Seek the stage to a song time, whichever view it shows.
function wordsSeekSong(t) {
  wordsVideo("seek", { timeSeconds: Math.max(0, t - wordsPanel.offset), seekMode: "commit" });
}

// On each status refresh: the stage appears once the video server is up, and goes when it stops.
function wordsStatusRefresh() {
  const url = serverUrl() || null;
  if (url === wordsPanel.server || !wordsCurrent()) return;
  wordsPanel.stage = null;
  wordsRender();
  wordsVideoRange();
}

// One control message to the video's preview player.
function wordsVideo(action, params = {}) {
  $("words-video-frame")?.contentWindow?.postMessage({ source: "hf-parent", type: "control", action, ...params }, "*");
}

// Hold the player to the selected section: it plays and loops only that stretch of the song.
function wordsVideoRange() {
  const section = wordsStudioSection(wordsCurrent());
  if (!section) return;
  wordsVideo("pause");
  const o = wordsPanel.offset;
  wordsVideo("set-play-range", { startSeconds: Math.max(0, section.start - o), endSeconds: section.end - o });
  wordsSeekSong(section.start);
}

// One measured scale for both timelines keeps every word reachable without changing song timing.
function wordsFitTimeline() {
  const view = $("view-design");
  if (!view?.classList.contains("active")) return;
  view.style.removeProperty("--song-s");
  let scale = parseFloat(getComputedStyle(document.documentElement).getPropertyValue("--space-1")) * 3.5;
  for (const row of view.querySelectorAll(".words-line")) {
    const seconds = Number(row.style.getPropertyValue("--len-s"));
    if (!(seconds > 0)) continue;
    const css = getComputedStyle(row), gap = parseFloat(css.rowGap) || 0;
    scale = Math.max(scale, (row.scrollHeight + (parseFloat(css.marginTop) || 0) + gap + 1) / seconds);
  }
  view.style.setProperty("--song-s", `${Math.ceil(scale)}px`);
}

// The video's own size, as data; the stage rule scales the frame to fit.
function wordsFit(size = wordsPanel.size) {
  const frame = $("words-video-frame"), box = $("words-preview");
  if (!frame || !box || !size) return;
  wordsPanel.size = size;
  frame.style.setProperty("--w", `${size.width}px`); frame.style.setProperty("--h", `${size.height}px`);
  frame.style.setProperty("--scale", String(box.clientWidth / size.width));
}

// The player reports its time, play state and stage size; the transport follows it.
addEventListener("message", (event) => {
  const frame = $("words-video-frame");
  if (!frame || event.source !== frame.contentWindow || event.data?.source !== "hf-preview") return;
  const data = event.data;
  if (data.type === "stage-size") wordsFit({ width: data.width, height: data.height });
  if (data.type === "state") {
    const song = data.currentTime + wordsPanel.offset; // the player's time, on the song clock
    wordsPanel.time = song; wordsPanel.playing = !!data.isPlaying;
    const scrub = $("words-scrub");
    if (scrub) { scrub.value = String(song); scrub.defaultValue = scrub.value; }
    if (wordsPanel.timeLabel) wordsPanel.timeLabel.textContent = wordsClock(song);
    wordsNow(song);
    wordsPanel.playButton?.setAttribute("aria-label", data.isPlaying ? "Pause this section" : "Play this section");
  }
});
addEventListener("resize", () => { wordsFitTimeline(); wordsFit(); });
let wordsLayoutWidth = 0;
new ResizeObserver(([entry]) => {
  if (entry.contentRect.width === wordsLayoutWidth) return;
  wordsLayoutWidth = entry.contentRect.width; wordsFitTimeline(); wordsFit();
}).observe($("view-design"));
// Leaving the Design view stops the video, so nothing plays behind another view.
new MutationObserver(() => {
  if ($("view-design")?.classList.contains("active")) return;
  if (wordsPanel.playing) wordsVideo("pause");
})
  .observe(document.getElementById("view-design") || document.body, { attributes: true, attributeFilter: ["class"] });

// One library of options for a word field, as cards: its name, then what it does.
function wordsLibrary(word, key, name, options) {
  return el("div", { class: "words-library", role: "radiogroup", "aria-label": `${name} for ${word.text}` },
    el("div", { class: "group-label" }, name),
    el("div", { class: "words-effects" }, ...options.map(([id, label, does]) =>
      el("button", { class: "effect", role: "radio", "aria-checked": String(wordsValue(word, key) === id), onclick: () => wordsEdit(word, key, id) },
        el("strong", {}, label), el("span", { class: "muted" }, does)))));
}

async function wordsSave() {
  const anim = wordsCurrent();
  if (!anim) return;
  const params = { animation_id: anim.animation_id, expected_source_revision: anim.source_revision, origin: "magicstudio-design",
    words: [...wordsPanel.pending].map(([word_id, edit]) => ({ word_id, ...edit })) };
  if (Object.keys(wordsPanel.styleEdits).length) params.style = wordsPanel.styleEdits;
  try {
    const result = await wordsCommand("words.save", params);
    if (result.conflict) { toast("Someone changed these words first; reloaded the latest"); wordsPanel.pending.clear(); wordsPanel.styleEdits = {}; return wordsOpen(); }
    wordsPanel.pending.clear(); wordsPanel.styleEdits = {};
    toast(`Saved ${wordsLabel(anim)} · rev ${result.animation.source_revision}`);
    await wordsOpen();
  } catch (error) { toast(String(error)); }
}
