// Design > Words: the visible lyric track, edited one verse/chorus at a time.
// Words and timing come from the artist's sheet and the hidden transcript reference; this panel
// changes only style, emphasis and motion (words.save). The stage plays the finished HyperFrames
// video itself, one section at a time, through the preview player's control messages; its
// lyric clips come from words.studio_track. One project, one clock, the song's own.
const wordsPanel = { read: null, align: null, studio: null, selected: null, word: null, pending: new Map(), styleEdits: {}, stage: null, time: 0, playing: false, size: null, laneHead: null, paneTitle: null, server: null, player: null, show: "full", offset: 0, timeLabel: null, effects: null };
const WORD_ACTIONS = [["none", "Still"], ["wave", "Wave"], ["tiny", "Small"], ["swim", "Swim"], ["no", "Shake"], ["cry", "Droop"], ["big", "Grow"], ["float", "Float"]];
const WORD_ENTRANCES = ["pop", "fade", "rise", "none"];
const WORD_EXITS = ["fade", "drop", "none"];
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


// While Design is open, the workstation's lower pane holds the effects; leaving gives it back.
function wordsEffectsPane(anim) {
  const slot = $("magic-parts-slot"), title = $("magic-parts-head")?.firstElementChild;
  if (!slot || !title) return;
  wordsPanel.paneTitle ??= title.textContent;
  title.textContent = "Effects";
  const current = anim.effects?.text?.name;
  const registry = wordsPanel.effects?.kinds?.text?.effects || [];
  slot.replaceChildren(
    el("div", { class: "group-label" }, `Text effects for ${wordsLabel(anim)} · HyperFrames`),
    current ? el("button", { class: "effect", "data-effect": "", onclick: () => wordsApplyRegistry(null) },
      el("strong", {}, `Using ${current}`), el("span", { class: "muted" }, "Click to remove this effect.")) : null,
    wordsPanel.effects?.error ? el("p", { class: "muted" }, wordsPanel.effects.error)
      : !wordsPanel.effects ? el("p", { class: "muted" }, "Finding effects…") : null,
    ...registry.filter((e) => e.name !== current).map((e) => el("button", { class: "effect", "data-effect": e.name, title: (e.tags || []).join(" · "), onclick: () => wordsApplyRegistry(e.name) },
      el("strong", {}, e.title || e.name), el("span", { class: "muted" }, e.description || ""))),
    null);
  if (!wordsPanel.effects) {
    wordsPanel.effects = { loading: true };
    wordsCommand("words.effects", { kind: "text" }).catch((error) => ({ error: String(error).replace(/^.*?: /, "") }))
      .then((found) => { wordsPanel.effects = found; if (state.view === "design") wordsEffectsPane(wordsCurrent()); });
  }
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

function wordsEffectsPaneClose() {
  const title = $("magic-parts-head")?.firstElementChild;
  if (title && wordsPanel.paneTitle != null) title.textContent = wordsPanel.paneTitle;
  wordsPanel.paneTitle = null;
  if (typeof renderMagicAssetCatalog === "function") renderMagicAssetCatalog();
}

function wordsRender() {
  const view = $("view-design");
  const anim = wordsCurrent();
  const animations = wordsPanel.read?.document?.animations || [];
  const reference = wordsPanel.align?.reference || {};
  const summary = wordsPanel.align?.summary || {};
  const mismatchedIn = (a) => a.words.some((w) => wordsMismatch(w.word_id));
  const side = el("aside", { class: "words-side", "aria-label": "Lyric sections" },
    el("div", { class: "segmented", role: "tablist" },
      el("button", { role: "tab", "aria-selected": "true" }, "Words"),
      el("button", { role: "tab", "aria-selected": "false", "aria-disabled": "true", title: "Proposed: the drawing track comes after Words" }, "Artwork · proposed")),
    el("div", { class: "group-label" }, "Sections"),
    ...animations.map((a) => el("button", { class: `panel-head item${a.animation_id === wordsPanel.selected ? " active" : ""}`, "data-section": a.animation_id, onclick: () => wordsSelect(a.animation_id) },
      wordsDot(mismatchedIn(a) ? "needs-attention" : "ready", mismatchedIn(a) ? "Some words differ from what was heard" : "Words match the timing reference"),
      el("span", { class: "title" }, wordsLabel(a)),
      el("span", { class: "meta" }, (() => { const v = wordsStudioSection(a); return v ? `${wordsClock(v.start)} · ${v.clips.length} clips` : `rev ${a.source_revision}`; })()))));
  if (!anim) { view.replaceChildren(side, el("p", { class: "panel-empty inline" }, "No lyric sections yet.")); return; }

  const lines = [];
  for (const w of anim.words) { if (lines.at(-1)?.id !== w.line_id) lines.push({ id: w.line_id, words: [] }); lines.at(-1).words.push(w); }
  const sheet = el("div", { class: "words-lines" }, ...lines.map((line) => el("div", { class: "words-line" }, ...line.words.map((w) => {
    const heard = wordsMismatch(w.word_id);
    const action = wordsValue(w, "action"), disagree = wordsDisagree(w.word_id);
    const chip = el("button", { class: `chip word${wordsPanel.word === w.word_id ? " active" : ""}`, "data-word-id": w.word_id, "data-action": action,
      title: `${w.text} · ${(+w.start).toFixed(2)}–${(+w.end).toFixed(2)} s${heard ? ` · heard as “${heard.heard ?? "nothing"}”` : ""}${disagree ? ` · measured alignment says ${(+disagree.measured[0]).toFixed(2)} s` : ""}`,
      onclick: () => { wordsPanel.word = w.word_id; wordsRender(); const clip = wordsClip(w.word_id); if (clip) wordsSeekSong(clip.start); } },
      w.text, action !== "none" ? el("span", { class: "chip-action" }, WORD_ACTIONS.find((x) => x[0] === action)?.[1] || action) : null);
    // Marks are states of the word, drawn by the chip's word look.
    if (heard) chip.dataset.heard = "differs";
    if (disagree) chip.dataset.timing = "differs";
    if (wordsValue(w, "emphasis")) chip.dataset.emphasis = "";
    return chip;
  }))));

  const word = anim.words.find((w) => w.word_id === wordsPanel.word);
  const inspector = word ? el("div", { class: "words-inspector", "aria-label": `Motion for ${word.text}` },
    el("div", { class: "group-label" }, `“${word.text}”`),
    el("div", { class: "words-actions", role: "radiogroup", "aria-label": "What the word does" }, ...WORD_ACTIONS.map(([id, label]) =>
      el("button", { class: `chip${wordsValue(word, "action") === id ? " active" : ""}`, role: "radio", "aria-checked": String(wordsValue(word, "action") === id), onclick: () => wordsEdit(word, "action", id) }, label))),
    el("label", { class: "field-row" },
      (() => { const box = el("input", { type: "checkbox", onchange: (e) => wordsEdit(word, "emphasis", e.target.checked) }); box.checked = !!wordsValue(word, "emphasis"); box.defaultChecked = !!word.emphasis; return box; })(), "Emphasis"),
    el("label", { class: "field-row" }, "Entrance", wordsSelectField(WORD_ENTRANCES, wordsValue(word, "entrance"), (v) => wordsEdit(word, "entrance", v))),
    el("label", { class: "field-row" }, "Exit", wordsSelectField(WORD_EXITS, wordsValue(word, "exit"), (v) => wordsEdit(word, "exit", v))))
    : el("p", { class: "panel-empty inline" }, "Choose a word to set what it does.");

  const size = wordsPanel.styleEdits.size ?? anim.style?.size ?? 120;
  const sizeInput = el("input", { type: "range", min: "60", max: "220", step: "2", "aria-label": "Word size",
    oninput: (e) => { wordsPanel.styleEdits.size = Number(e.target.value); if (wordsPanel.styleEdits.size === (anim.style?.size ?? 120)) delete wordsPanel.styleEdits.size; wordsRender(); } });
  sizeInput.value = String(size);
  sizeInput.defaultValue = String(anim.style?.size ?? 120); // only a moved slider is an unsaved edit
  const changes = wordsPanel.pending.size + Object.keys(wordsPanel.styleEdits).length;
  const save = el("button", { class: "start", id: "words-save", onclick: wordsSave }, changes ? `Save ${changes} change${changes > 1 ? "s" : ""}` : "Saved");
  save.disabled = !changes;

  const stage = wordsStage(anim);
  const main = el("section", { class: "words-main", "aria-label": `${wordsLabel(anim)} words` },
    el("div", { class: "words-head" },
      el("strong", {}, wordsLabel(anim)),
      el("span", { class: "meta" }, `${anim.words.length} words · ${(anim.authored_duration || 0).toFixed(1)} s · rev ${anim.source_revision}${wordsPanel.read.saved ? "" : " · not saved yet"}`),
      el("label", { class: "field-row" }, "Size", sizeInput),
      save),
    wordsPanel.read.reference_changed ? el("p", { class: "not-linked" }, "The timing reference changed since this was saved; check the words before saving.") : null,
    stage, sheet, inspector);
  view.replaceChildren(side, main);
  wordsEffectsPane(anim);
  wordsFit();
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
  scrub.value = scrub.defaultValue = String(section.start); // a view control, never an unsaved edit
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
    el("div", { class: "stage", id: "words-preview" }, frame),
    el("div", { class: "transport" },
      wordsPanel.playButton = el("button", { class: "icon", "aria-label": "Play this section", title: "Play this section of the video", onclick: () => wordsVideo(wordsPanel.playing ? "pause" : "play"), html: ICONS.play }),
      scrub,
      wordsPanel.timeLabel = el("span", { class: "meta" }, `${wordsClock(section.start)} – ${wordsClock(section.end)}`)),
    wordsLane("Lyric words · in the video", section));
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

// One lane of the section: the video's lyric clips where they sit in time, a playhead on top.
// Positions are data (--start, --len, --at); clicking a clip seeks the video to it.
function wordsLane(name, section) {
  const span = section.end - section.start;
  const at = (t) => `${(100 * (t - section.start) / span).toFixed(3)}%`;
  const head = el("span", { class: "lane-head" });
  const track = el("div", { class: "lane-track" }, ...section.clips.map((clip) => {
    const block = el("button", { class: "lane-block", title: `${clip.text} · ${wordsClock(clip.start)}`, "data-word-id": clip.word_id,
      onclick: () => wordsSeekSong(clip.start) }, clip.text);
    block.style.setProperty("--start", at(clip.start)); block.style.setProperty("--len", `${(100 * clip.duration / span).toFixed(3)}%`);
    return block;
  }), head);
  wordsPanel.laneHead = (t) => head.style.setProperty("--at", at(Math.min(section.end, Math.max(section.start, t))));
  return el("div", { class: "lane" }, el("span", { class: "lane-name" }, name), track);
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
    if (scrub) scrub.value = scrub.defaultValue = String(song);
    if (wordsPanel.timeLabel) wordsPanel.timeLabel.textContent = wordsClock(song);
    wordsPanel.laneHead?.(song);
    wordsPanel.playButton?.setAttribute("aria-label", data.isPlaying ? "Pause this section" : "Play this section");
  }
});
addEventListener("resize", () => wordsFit());
// Leaving the Design view stops the video, so nothing plays behind another view.
new MutationObserver(() => {
  if ($("view-design")?.classList.contains("active")) return;
  if (wordsPanel.playing) wordsVideo("pause");
  if (wordsPanel.paneTitle != null) wordsEffectsPaneClose();
})
  .observe(document.getElementById("view-design") || document.body, { attributes: true, attributeFilter: ["class"] });

function wordsSelectField(options, value, onchange) {
  const select = el("select", { onchange: (e) => onchange(e.target.value) }, ...options.map((o) => el("option", { value: o }, o)));
  select.value = value;
  return select;
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
