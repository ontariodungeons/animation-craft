// Design > Words: the visible lyric track, edited one verse/chorus at a time.
// Words and timing come from the artist's sheet and the hidden transcript reference; this panel
// changes only style, emphasis and motion (words.save), and previews the composed HyperFrames
// track (words.compose) against the song itself. One project, one clock, no preview server.
const wordsPanel = { read: null, align: null, selected: null, word: null, pending: new Map(), styleEdits: {}, preview: null, frame: 0, stage: null };
const WORD_ACTIONS = [["none", "Still"], ["wave", "Wave"], ["tiny", "Small"], ["swim", "Swim"], ["no", "Shake"], ["cry", "Droop"], ["big", "Grow"], ["float", "Float"]];
const WORD_ENTRANCES = ["pop", "fade", "rise", "none"];
const WORD_EXITS = ["fade", "drop", "none"];

const wordsSongPath = () => state.data?.status?.stages?.find((s) => s.id === "song")?.inputs?.[0]?.path;
const wordsCurrent = () => wordsPanel.read?.document?.animations?.find((a) => a.animation_id === wordsPanel.selected);
const wordsMismatch = (id) => wordsPanel.align?.words?.find((w) => w.word_id === id && w.status !== "match");
const wordsDisagree = (id) => wordsPanel.align?.timing_disagreements?.find((d) => d.word_id === id);
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
    const [read, align] = await Promise.all([
      invoke("engine_command", { id: "words.read", params: {} }),
      invoke("engine_command", { id: "words.align", params: {} }),
    ]);
    wordsPanel.read = read;
    wordsPanel.align = align;
    if (!wordsCurrent()) wordsPanel.selected = read.document.animations[0]?.animation_id || null;
    wordsRender();
    wordsPreview();
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
  wordsPreview();
}

// The effective value of a word field: the pending edit if any, else the saved value.
function wordsValue(word, key) {
  const edit = wordsPanel.pending.get(word.word_id);
  return edit && key in edit ? edit[key] : word[key];
}

function wordsEdit(word, key, value) {
  const edit = { ...(wordsPanel.pending.get(word.word_id) || {}) };
  if (value === word[key]) delete edit[key]; else edit[key] = value;
  if (Object.keys(edit).length) wordsPanel.pending.set(word.word_id, edit); else wordsPanel.pending.delete(word.word_id);
  wordsRender();
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
      el("span", { class: "meta" }, `rev ${a.source_revision}`))),
    el("div", { class: "words-reference" },
      el("div", { class: "group-label" }, "Timing reference · hidden"),
      el("p", {}, `${reference.word_count ?? "?"} heard words · ${summary.matched ?? "?"} of ${summary.canonical_words ?? "?"} artist words match`),
      summary.timing_disagreements ? el("p", { class: "not-linked" }, `${summary.timing_disagreements} word${summary.timing_disagreements > 1 ? "s" : ""} timed differently by the measured alignment (dotted)`) : null,
      el("p", { class: "muted" }, "Never shown in the video and never edited here."),
      el("button", { class: "ghost", id: "words-inspect-reference", onclick: () => {
        const doc = state.data?.status?.stages?.find((s) => s.id === "transcript")?.document;
        lyricReaderOpen(state.lyrics, doc, wordsSongPath(), true);
      } }, "Inspect (read only)")));
  if (!anim) { view.replaceChildren(side, el("p", { class: "panel-empty inline" }, "No lyric sections yet.")); return; }

  const lines = [];
  for (const w of anim.words) { if (lines.at(-1)?.id !== w.line_id) lines.push({ id: w.line_id, words: [] }); lines.at(-1).words.push(w); }
  const sheet = el("div", { class: "words-lines" }, ...lines.map((line) => el("div", { class: "words-line" }, ...line.words.map((w) => {
    const heard = wordsMismatch(w.word_id);
    const action = wordsValue(w, "action"), disagree = wordsDisagree(w.word_id);
    const chip = el("button", { class: `chip word${wordsPanel.word === w.word_id ? " active" : ""}`, "data-word-id": w.word_id, "data-action": action,
      title: `${w.text} · ${(+w.start).toFixed(2)}–${(+w.end).toFixed(2)} s${heard ? ` · heard as “${heard.heard ?? "nothing"}”` : ""}${disagree ? ` · measured alignment says ${(+disagree.measured[0]).toFixed(2)} s` : ""}`,
      onclick: () => { wordsPanel.word = w.word_id; wordsRender(); wordsSeek((+w.start) - (anim.song_anchor || 0), true); } },
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
  if (wordsPanel.preview?.html) wordsMount();
}

// The preview stage survives re-renders of the same section, so editing never reloads it.
function wordsStage(anim) {
  if (wordsPanel.stage?.dataset.animation === anim.animation_id) return wordsPanel.stage;
  const scrub = el("input", { type: "range", id: "words-scrub", min: "0", max: String(anim.authored_duration || 1), step: "0.01", "aria-label": "Preview time",
    oninput: (e) => wordsSeek(Number(e.target.value), true) });
  scrub.value = scrub.defaultValue = "0"; // a view control, never an unsaved edit
  wordsPanel.stage = el("div", { class: "words-preview", "data-animation": anim.animation_id },
    el("div", { class: "stage", id: "words-preview" }, el("iframe", { id: "words-preview-frame", title: "Words track preview", sandbox: "allow-scripts allow-same-origin" })),
    el("div", { class: "transport" },
      el("button", { class: "icon", id: "words-play", "aria-label": "Play preview", title: "Play with the song", onclick: wordsPlay, html: ICONS.play }),
      scrub,
      el("span", { class: "meta", id: "words-time" }, `0.0 / ${(anim.authored_duration || 0).toFixed(1)} s`)));
  return wordsPanel.stage;
}

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
    const result = await invoke("engine_command", { id: "words.save", params });
    if (result.conflict) { toast("Someone changed these words first; reloaded the latest"); wordsPanel.pending.clear(); wordsPanel.styleEdits = {}; return wordsOpen(); }
    wordsPanel.pending.clear(); wordsPanel.styleEdits = {};
    toast(`Saved ${wordsLabel(anim)} · rev ${result.animation.source_revision}`);
    await wordsOpen();
  } catch (error) { toast(String(error)); }
}

// Compose the saved animation and show it on the song clock. Unsaved edits are not previewed.
async function wordsPreview() {
  const anim = wordsCurrent();
  if (!anim) return;
  try {
    const receipt = await invoke("engine_command", { id: "words.compose", params: { animation_id: anim.animation_id } });
    const html = await (await fetch(`${convertFileSrc(receipt.file)}?v=${receipt.sha256}`)).text();
    wordsPanel.preview = { receipt, html: html.replace("<head>", "<head><style>html,body{margin:0;background:transparent;overflow:hidden}</style>") };
    wordsMount();
  } catch (error) {
    wordsPanel.preview = null;
    const box = $("words-preview");
    box?.replaceChildren(el("p", { class: "panel-empty inline" }, `No preview yet: ${String(error).replace(/^.*?: /, "")}`));
  }
}

function wordsMount() {
  const frame = $("words-preview-frame"), box = $("words-preview");
  if (!frame || !box || !wordsPanel.preview) return;
  const { receipt, html } = wordsPanel.preview;
  // Geometry from the composition, as data; the stage rule sizes and scales the frame.
  frame.style.setProperty("--w", `${receipt.width}px`); frame.style.setProperty("--h", `${receipt.height}px`);
  frame.style.setProperty("--scale", String(box.clientWidth / receipt.width));
  if (frame.srcdoc !== html) frame.srcdoc = html;
}

function wordsTimeline() {
  const r = wordsPanel.preview?.receipt;
  return r && $("words-preview-frame")?.contentWindow?.__timelines?.[r.composition_id];
}

// `moveSong` is true for scrubbing; during playback the song leads and only the timeline follows.
function wordsSeek(local, moveSong = false) {
  const r = wordsPanel.preview?.receipt;
  if (!r) return;
  const t = Math.max(0, Math.min(local, r.authored_duration));
  wordsTimeline()?.seek(t);
  const audio = songPlayer.path ? songPlayer.audio : null;
  if (moveSong && audio && !audio.paused) audio.currentTime = r.song_anchor + t;
  if ($("words-scrub")) $("words-scrub").value = $("words-scrub").defaultValue = String(t);
  if ($("words-time")) $("words-time").textContent = `${t.toFixed(1)} / ${r.authored_duration.toFixed(1)} s`;
}

// Play the real song from this section's anchor and follow it with the composed timeline.
function wordsPlay() {
  const r = wordsPanel.preview?.receipt, path = wordsSongPath();
  if (!r || !path) return toast("Compose a preview and link the song first");
  const audio = songAudio(path);
  if (!audio.paused) { audio.pause(); return; }
  const start = Number($("words-scrub")?.value || 0);
  audio.currentTime = r.song_anchor + (start >= r.authored_duration - 0.05 ? 0 : start);
  audio.play().catch((error) => toast(`Cannot play the song: ${error.message || error}`));
  cancelAnimationFrame(wordsPanel.frame);
  const follow = () => {
    const t = audio.currentTime - r.song_anchor;
    if (state.view !== "design") { audio.pause(); return; }
    if (audio.paused) return;
    if (t >= r.authored_duration) { audio.pause(); wordsSeek(r.authored_duration); return; }
    wordsSeek(t);
    wordsPanel.frame = requestAnimationFrame(follow);
  };
  wordsPanel.frame = requestAnimationFrame(follow);
}
