// Design > Lyrics: design each sung line of the song, one verse/chorus at a time.
// Words come from the artist's sheet and times from the video's lyric track (words.studio_track);
// neither is edited here. Each line gets a design (where its words sit, their lettering, how they
// move on a sung word, and strokes that draw on), saved with words.save. The lyrics (left) and a
// film roll of the designed lines (right) run down the page on the song clock; the stage, the
// line's lanes and the options for what is picked stay pinned in the centre. The Video view plays
// the finished HyperFrames video through its preview player.
const wordsPanel = { mode:"lyrics", links:{color:null,front:true,pulse:true}, linkDrag:null, editor: null, cue: null, cueDrag: null, previewRange: null, autoPlay: false, read: null, align: null, studio: null, selected: null, line: null, item: null, picked: [], designs: new Map(), styleEdits: {}, stage: null, time: 0, playing: false, size: null, lanes: null, view: "design", tab: "look", scene: true, sceneAt: null, followed: null, canvas: null, audio: null, drag: null, server: null, player: null, show: "still", offset: 0, timeLabel: null, effects: null };
try {const links=JSON.parse(localStorage.getItem("magicstudio.lyrics.links")||"{}");
  if(/^#[0-9a-f]{6}$/i.test(links.color))wordsPanel.links.color=links.color;
  for(const key of ["front","pulse"])if(typeof links[key]==="boolean")wordsPanel.links[key]=links[key];
}catch{}
// How a designed lyric item moves on its accent word (the engine's MOVES), and what each does.
const WORDS_MOVES = [["quiet", "Quiet", "Fades in and holds"], ["sleep", "Sleep", "Letters sink one by one"], ["settle", "Settle", "Drops and squashes into place"],
  ["fall", "Fall", "Drops down"], ["pop", "Pop", "A quick swell on the beat"], ["rise", "Rise", "Arcs up and away"], ["east", "East", "Slides right"],
  ["west", "West", "Slides left"], ["loop", "Loop", "Makes one loop"]];
// Lettering: [key, name, what it is for, font (the song's own, a serif or a sans), weight, italic].
const WORDS_LETTERING = [["song", "Song font", "The video's own lettering", "song", 400, false], ["song-bold", "Song bold", "The video's lettering, heavy", "song", 700, false],
  ["serif-italic", "Serif italic", "A hero word, expressive", "serif", 400, true], ["serif-bold", "Serif bold", "A hero word, strong", "serif", 700, false],
  ["sans", "Sans, quiet", "Context words, small and calm", "sans", 400, false]];
// Hand-drawn strokes that draw on in time with the line (shapes from the Pterodactyl Song's lyric scenes).
const WORDS_STROKES = [
  { name: "Arrow down", does: "A wandering arrow pointing down", view_box: "0 0 160 330", w: 130, h: 300, draw: 1.3, d: "M 73 13 C 27 67 104 129 66 196 Q 47 231 61 289 M 30 264 L 61 294 L 87 260" },
  { name: "Arrow right", does: "Travels right", view_box: "0 0 670 65", w: 520, h: 52, draw: 0.9, d: "M 15 33 Q 208 17 409 30 T 650 26 M 625 6 L 655 26 L 626 49" },
  { name: "Arrow left", does: "Travels left", view_box: "0 0 670 65", w: 520, h: 52, draw: 0.9, d: "M 655 33 Q 462 17 261 30 T 20 26 M 45 6 L 15 26 L 44 49" },
  { name: "Rising swoop", does: "A flight trail from low to high", view_box: "0 0 790 160", w: 600, h: 120, draw: 1.8, d: "M 10 145 Q 126 170 217 93 T 438 58 Q 578 29 756 7 M 730 5 L 761 5 L 746 29" },
  { name: "Loop", does: "One playful loop", view_box: "0 0 620 110", w: 500, h: 90, draw: 2.1, d: "M 15 20 C 110 95 425 95 455 40 C 485 -4 350 -3 352 37 C 355 95 515 85 602 50 M 579 33 L 608 49 L 586 73" },
  { name: "Underline", does: "Comes to rest under a word", view_box: "0 0 280 30", w: 280, h: 30, draw: 0.55, d: "M 5 14 Q 61 5 125 13 T 269 10" },
  { name: "Hill", does: "A rounded hill beneath the words", view_box: "0 0 410 95", w: 410, h: 95, draw: 1.7, d: "M 7 77 Q 73 72 118 32 Q 171 -8 228 31 Q 279 75 400 76" }];
const wordsCommand = (command, params = {}) => invoke("engine_command", { id: command, params });

const wordsSongPath = () => state.data?.status?.stages?.find((s) => s.id === "song")?.inputs?.[0]?.path;
const wordsCurrent = () => wordsPanel.read?.document?.animations?.find((a) => a.animation_id === wordsPanel.selected);
// The same printed section in the finished video: its span on the song clock and its lyric clips.
const wordsStudioSection = (anim) => wordsPanel.studio?.sections?.find((s) => s.section === anim?.scope?.section && s.occurrence === anim?.scope?.occurrence);
const wordsClip = (id, anim = wordsCurrent()) => wordsStudioSection(anim)?.clips.find((c) => c.word_id === id);
const wordsClock = (t) => `${Math.floor(t / 60)}:${(t % 60).toFixed(1).padStart(4, "0")}`;

function wordsLabel(anim) {
  const s = anim.scope || {};
  const name = String(s.section || "lyrics").replace(/_(\d+)$/, " $1").replace(/_/g, " ");
  return `${name[0].toUpperCase()}${name.slice(1)}${s.section === "chorus" || s.occurrence > 1 ? ` ${s.occurrence}` : ""}`;
}

async function wordsOpen() {
  if(wordsPanel.editor?.dirty) return;
  const view = $("view-design");
  if (!view) return;
  view.replaceChildren(el("p", { class: "panel-empty inline" }, "Loading the words…"));
  try {
    const [read, align, studio, backgrounds] = await Promise.all([
      wordsCommand("words.read"),
      wordsCommand("words.align"),
      wordsCommand("words.studio_track").catch((error) => ({ error: String(error).replace(/^.*?: /, "") })),
      wordsCommand("storyboard.backgrounds").catch(error=>({images:[],error:String(error).replace(/^.*?: /, "")})),
    ]);
    wordsPanel.read = read;
    wordsPanel.backgrounds = backgrounds;
    wordsPanel.align = align;
    await wordsLoadFonts(read.fonts || []);
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

// The video's own lettering, loaded once so the designer letters in the same type.
const wordsFontsLoaded = new Set();
async function wordsLoadFonts(fonts) {
  await Promise.all(fonts.filter((f) => !wordsFontsLoaded.has(f.path)).map(async (f) => {
    wordsFontsLoaded.add(f.path);
    try { document.fonts.add(await new FontFace(f.family, `url(${JSON.stringify(convertFileSrc(f.path))})`, { weight: f.weight || "400" }).load()); }
    catch (error) { toast(`The video's font ${f.family} did not load: ${error}`); }
  }));
}
const wordsSongFont = (anim) => anim.style?.font || wordsPanel.read?.fonts?.[0]?.family || "sans-serif";

function wordsSelect(id) {
  if (wordsPanel.designs.size || Object.keys(wordsPanel.styleEdits).length) {
    if (!confirm("Discard unsaved lyric changes?")) return;
  }
  wordsPanel.designs.clear(); wordsPanel.styleEdits = {}; wordsPanel.line = null; wordsPanel.item = null; wordsPanel.selected = id;
  wordsRender();
  wordsVideoRange();
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
  if (wordsPanel.designs.size || Object.keys(wordsPanel.styleEdits).length) return toast("Save or discard the lyric changes first");
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
const wordsAt = (w) => wordsClip(w.word_id, wordsPanel.read?.document?.animations?.find((a) => a.words.includes(w)) || wordsCurrent())?.start ?? +w.start;
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

// The lines of a section that hold a given word, and a line's words.
const wordsLineWords = (anim, lineId) => anim.words.filter((w) => w.line_id === lineId);
// The composition's size: the video's, once the player reports it.
const wordsWorld = () => [wordsPanel.size?.width || 1920, wordsPanel.size?.height || 1080];
// A line's span on the song clock: from its first sung word to the next line (or the section's end).
function wordsLineSpan(anim, lineId) {
  const lines = wordsLines(anim), k = lines.findIndex((l) => l.id === lineId);
  const start = wordsAt(lines[k].words[0]);
  return { start, end: lines[k + 1] ? wordsAt(lines[k + 1].words[0]) : wordsSpan(anim).end };
}
// The design a line shows: an unsaved edit, else the saved one, else a plain line.
function wordsDesign(anim, lineId) {
  const pending = wordsPanel.designs.get(lineId);
  return pending !== undefined ? pending ?? wordsLayout(anim, lineId, "plain") : anim.design?.[lineId] ?? wordsLayout(anim, lineId, "plain");
}
function wordsDesignEdit(anim, lineId, change) {
  const design = structuredClone(wordsDesign(anim, lineId));
  change(design);
  wordsPanel.designs.set(lineId, design);
  wordsRender();
}

// Ready-made layouts for a whole line. Sizes and places are fractions of the composition, so a
// layout fits any video; every word stays its own item unless the layout groups context words.
function wordsLayout(anim, lineId, kind) {
  const words = wordsLineWords(anim, lineId), [W, H] = wordsWorld();
  const item = (ws, x, y, size, look = "song", motion = "quiet") => {
    const [, , , font, weight, italic] = WORDS_LETTERING.find((l) => l[0] === look);
    return { word_ids: ws.map((w) => w.word_id), x: Math.round(x), y: Math.round(y), size: Math.round(size), font, weight, italic, color: anim.style?.color || "#1b2a33",
      outline: null, rotate: 0, motion, accent: motion === "quiet" ? null : ws.at(-1).word_id };
  };
  const width = (ws, size, look = "song") => wordsMeasure(anim, ws.map((w) => w.text).join(" "), size, look);
  if (kind === "poster") {
    // The longest word carries the line; the words around it stay small.
    const hero = words.reduce((a, b) => (b.text.length > a.text.length ? b : a)), k = words.indexOf(hero), small = H * 0.04;
    return { items: [
      ...(k ? [item(words.slice(0, k), W * 0.45, H * 0.1, small, "sans")] : []),
      item([hero], W * 0.44, H * 0.15, H * 0.15, "serif-italic", "pop"),
      ...(k < words.length - 1 ? [item(words.slice(k + 1), W * 0.45 + width([hero], H * 0.15, "serif-italic") * 0.6, H * 0.33, small, "sans")] : [])], draws: [] };
  }
  if (kind === "stack") {
    const size = H * 0.085;
    return { items: words.map((w, i) => item([w], W * 0.08, H * 0.12 + i * size * 1.1, size, i === words.length - 1 ? "serif-bold" : "song", i === words.length - 1 ? "settle" : "quiet")), draws: [] };
  }
  // plain: one line along the bottom, each word in place
  const size = H * 0.06, gap = size * 0.35, total = words.reduce((sum, w) => sum + width([w], size) + gap, -gap);
  let x = (W - total) / 2;
  return { items: words.map((w) => { const it = item([w], x, H * 0.82, size); x += width([w], size) + gap; return it; }), draws: [] };
}

// A word's drawn width, measured in the lettering it will use.
const wordsMeasurer = document.createElement("canvas").getContext("2d");
function wordsMeasure(anim, text, size, look) {
  const [, , , font, weight, italic] = WORDS_LETTERING.find((l) => l[0] === look);
  wordsMeasurer.font = `${italic ? "italic " : ""}${weight} ${size}px ${font === "song" ? JSON.stringify(wordsSongFont(anim)) : font === "serif" ? "serif" : "sans-serif"}`;
  return wordsMeasurer.measureText(text).width;
}
const wordsEase = { out2: (x) => 1 - (1 - x) ** 2, out3: (x) => 1 - (1 - x) ** 3, sin: (x) => 0.5 - 0.5 * Math.cos(Math.PI * x) };
const wordsClamp = (x) => Math.max(0, Math.min(1, x));

// One frame of a designed line at song time t, drawn into a composition-sized world. Every value
// the stylesheet needs goes in as a custom property; the motion follows the Pterodactyl lettering.
function wordsDrawLine(world, anim, lineId, t, interactive) {
  const [W, H] = wordsWorld(), design = wordsDesign(anim, lineId), span = wordsLineSpan(anim, lineId), words = wordsLineWords(anim, lineId);
  world.style.setProperty("--w", `${W}px`); world.style.setProperty("--h", `${H}px`);
  world.style.setProperty("--family", JSON.stringify(wordsSongFont(anim)));
  const local = t - span.start;
  const strokes = design.draws.map((d) => {
    const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
    svg.setAttribute("class", "lyric-draw");
    svg.setAttribute("viewBox", d.view_box);
    for (const [key, value] of [["--x", d.x], ["--y", d.y], ["--dw", d.w], ["--dh", d.h]]) svg.style.setProperty(key, `${value}px`);
    svg.innerHTML = d.paths.map((p) => `<path d="${p.d}" pathLength="1" fill="none" stroke="${p.stroke}" stroke-width="${p.width}" stroke-linecap="round" stroke-linejoin="round" stroke-dasharray="1" stroke-dashoffset="${1 - wordsClamp((local - p.from) / Math.max(0.01, p.to - p.from))}"/>`).join("");
    return svg;
  });
  const items = design.items.map((it, i) => {
    const text = it.word_ids.map((id) => words.find((w) => w.word_id === id)?.text).join(" ");
    const accentWord = words.find((w) => w.word_id === it.accent);
    const a = (accentWord ? wordsAt(accentWord) : span.start) - span.start;
    const settled = wordsEase.out3(wordsClamp((local - a) / 0.65)), arc = wordsClamp((local - a) / 0.75);
    const pulse = local < a ? 1 : local < a + 0.13 ? 1 + 0.12 * (local - a) / 0.13 : local < a + 0.43 ? 1.12 - 0.12 * (local - a - 0.13) / 0.3 : 1;
    const [tx, ty] = { settle: [0, 6 * settled], fall: [0, 20 * settled], rise: [24 * settled, -22 * Math.sin(arc * Math.PI) - 18 * settled], east: [18 * settled, 0], west: [-18 * settled, 0],
      loop: [12 * Math.sin(arc * Math.PI * 2), 7 * (Math.cos(arc * Math.PI * 2) - 1)] }[it.motion] || [0, 0];
    const node = el("div", { class: `lyric-item${interactive && wordsPanel.item === i ? " active" : ""}`, "data-weight": String(it.weight), "data-italic": String(it.italic), "data-font": it.font || "song" });
    if (it.outline) { node.dataset.outline = ""; node.style.setProperty("--outline", it.outline); }
    for (const [key, value] of [["--x", `${it.x}px`], ["--y", `${it.y}px`], ["--size", `${it.size}px`], ["--ink", it.color], ["--rot", `${it.rotate}deg`],
      ["--o", String(wordsEase.out2(wordsClamp((local + 0.4) / 0.4)))], ["--tx", `${tx}px`], ["--ty", `${ty}px`],
      ["--sx", String(it.motion === "pop" ? pulse : 1)], ["--sy", String(it.motion === "pop" ? pulse : it.motion === "settle" ? 1 - 0.035 * settled : 1)]]) node.style.setProperty(key, value);
    if (it.motion === "sleep") node.append(...[...text].map((ch, k) => {
      const u = wordsEase.sin(wordsClamp((local - a - k * 0.035) / 0.8)), letter = el("span", {}, ch);
      letter.style.setProperty("--ty", `${7 * u + 3 * Math.sin(k * 0.6) * Math.sin(u * Math.PI)}px`); letter.style.setProperty("--lr", `${-0.6 * u}deg`);
      return letter;
    }));
    else node.append(el("span", {}, text));
    if (interactive) node.onpointerdown = (event) => {
      event.preventDefault();
      wordsPanel.item = i; wordsPanel.drag = { i, x: event.clientX, y: event.clientY, from: [it.x, it.y], scale: world.getBoundingClientRect().width / W, moved: false };
      wordsRender();
    };
    return node;
  });
  world.replaceChildren(...strokes, ...items);
}

// The designer's stage: the selected line on plain paper, the song under it.
function wordsCanvas(anim) {
  const [W, H] = wordsWorld(), world = el("div", { class: "lyric-world" });
  const scene = wordsPanel.scene && serverUrl() ? el("iframe", { class: "lyric-scene", title: "The scene behind the lyrics", tabindex: "-1" }) : null;
  const box = el("div", { class: "lyric-canvas", "aria-label": "Line designer" }, scene, world);
  box.style.setProperty("--w", `${W}px`); box.style.setProperty("--h", `${H}px`);
  wordsPanel.canvas = { box, world, scene };
  wordsFitter.observe(box);
  if (scene) wordsCommand("words.lyrics_view", { show: "scene" }).then((view) => {
    const server = new URL(serverUrl()), project = server.hash.split("/")[1] || "hyperframes";
    scene.addEventListener("load", () => wordsSceneSeek(wordsPanel.time));
    scene.setAttribute("src", `${server.origin}/api/projects/${project}/preview/comp/${view.src}`);
  }).catch((error) => toast(String(error).replace(/^.*?: /, "")));
  return box;
}
// The scene shows the video's picture at a song time, without its lyric clips.
function wordsSceneSeek(t) {
  wordsPanel.canvas?.scene?.contentWindow?.postMessage({ source: "hf-parent", type: "control", action: "seek", timeSeconds: Math.max(0, t), seekMode: "commit" }, "*");
}
function wordsCanvasDraw(t = wordsPanel.time) {
  const anim = wordsCurrent(), canvas = wordsPanel.canvas;
  if (!anim || !canvas || !wordsPanel.line) return;
  wordsDrawLine(canvas.world, anim, wordsPanel.line, t, true);
  if (!wordsPanel.playing || Math.abs(t - (wordsPanel.sceneAt ?? -9)) > 0.25) { wordsSceneSeek(t); wordsPanel.sceneAt = t; }
}
// A world fits its box whole: scaled by width or height, whichever is tighter, and kept fitted as
// the box changes size (the page settles after the first frame, and windows resize).
const wordsFitter = new ResizeObserver((entries) => {
  const [W, H] = wordsWorld();
  for (const { target } of entries) target.style.setProperty("--scale", String(Math.min(target.clientWidth / W, target.clientHeight / H)));
});
// The settled look of a line: every move done and every stroke drawn.
const wordsPose = (anim, lineId) => wordsLineSpan(anim, lineId).end - 0.05;

// Play the song from the line, on through the whole song: the stage follows each line as it is sung
// and the lyrics and film roll roll up past it, like film. A section with unsaved changes stays put.
function wordsPlay() {
  const anim = wordsCurrent();
  if (!anim || !wordsPanel.line) return;
  const path = wordsSongPath();
  if (!path) return toast("Add the song in Project › Song to hear it here");
  wordsPanel.audio ??= new Audio();
  const audio = wordsPanel.audio, span = wordsLineSpan(anim, wordsPanel.line);
  if (!audio.paused) { audio.pause(); return; }
  if (audio.getAttribute("src") !== convertFileSrc(path)) audio.src = convertFileSrc(path);
  audio.currentTime = Math.max(0, span.start - 0.4);
  audio.play();
  const tick = () => {
    if (audio.paused || audio.ended || state.view !== "design") { audio.pause(); wordsPanel.playing = false; return; }
    const t = audio.currentTime, animations = wordsPanel.read.document.animations;
    const here = animations.find((a) => { const s = wordsSpan(a); return t >= s.start - 0.4 && t < s.end; });
    const unsaved = wordsPanel.designs.size || Object.keys(wordsPanel.styleEdits).length;
    if (here && here.animation_id !== wordsPanel.selected && !unsaved) { wordsPanel.selected = here.animation_id; wordsPanel.line = null; wordsPanel.item = null; wordsPanel.followed = `${here.animation_id}/playing`; wordsRender(); }
    const current = wordsCurrent(), line = wordsLines(current).findLast((l) => wordsAt(l.words[0]) <= t + 0.4);
    if (line && line.id !== wordsPanel.line) { wordsPanel.line = line.id; wordsPanel.item = null; wordsPanel.followed = `${current.animation_id}/${line.id}`; wordsRender(); }
    wordsPanel.time = t; wordsPanel.playing = true;
    wordsCanvasDraw(t); wordsNow(t);
    requestAnimationFrame(tick);
  };
  requestAnimationFrame(tick);
}

// Lyric tags: each sung word of the line, laid on a grid; a hero word shows its lettering and move.
function wordsChip(anim, w) {
  const design = wordsDesign(anim, w.line_id), index = design.items.findIndex((it) => it.word_ids.includes(w.word_id)), it = design.items[index];
  const on = anim.animation_id === wordsPanel.selected && wordsPanel.line === w.line_id && (wordsPanel.picked.includes(w.word_id) || (!wordsPanel.picked.length && wordsPanel.item === index));
  const move = it && it.motion !== "quiet" ? WORDS_MOVES.find((m) => m[0] === it.motion)?.[1] : "";
  // The word in its own lettering: the line design's font, weight, slant, colour and relative size.
  const chip = el("button", { class: `word lettered${on ? " active" : ""}`, "data-word-id": w.word_id,
    title: `${w.text} · ${wordsClock(wordsAt(w))}${move ? ` · ${move}` : ""} · Shift-click adds words`,
    onclick: (event) => {
      event.stopPropagation();
      if(wordsPanel.editor){wordsAfterEdit(()=>wordsChip(wordsCurrent(),w).onclick(event));return;}
      if (anim.animation_id !== wordsPanel.selected) wordsSelect(anim.animation_id);
      if (anim.animation_id !== wordsPanel.selected) return;
      const same = wordsPanel.line === w.line_id && event.shiftKey;
      wordsPanel.picked = same ? (wordsPanel.picked.includes(w.word_id) ? wordsPanel.picked.filter((id) => id !== w.word_id) : [...wordsPanel.picked, w.word_id]) : [w.word_id];
      wordsPanel.line = w.line_id; wordsPanel.item = index; wordsPanel.time = wordsLineSpan(anim, w.line_id).start; wordsPanel.previewRange = null; wordsPanel.autoPlay = true; wordsRender(); wordsVideoRange();
      if(!event.shiftKey)requestAnimationFrame(()=>wordsBeginEdit(wordsCurrent(),w.line_id,event));
    } }, w.text);
  if (it) {
    chip.style.setProperty("--word-font", it.font === "song" || !it.font ? wordsSongFont(anim) : it.font);
    chip.style.setProperty("--word-weight", String(it.weight || 400));
    chip.style.setProperty("--word-slant", it.italic ? "italic" : "normal");
    chip.style.setProperty("--word-color", /^#[0-9a-f]{6}$/i.test(it.color) ? it.color : "currentColor");
    chip.style.setProperty("--word-scale", Math.min(1.7, Math.max(0.8, (it.size || 110) / 110)).toFixed(2));
  }
  if (anim.kinetic) {
    const st = anim.kinetic.style || {}, hero = !!anim.kinetic.acts?.[w.word_id];
    chip.style.setProperty("--word-font", hero ? st.hero_font || st.font || "sans-serif" : st.font || "sans-serif");
    chip.style.setProperty("--word-weight", String(hero ? st.hero_weight || 700 : st.weight || 700));
    chip.style.setProperty("--word-slant", hero && st.hero_italic ? "italic" : "normal");
    chip.style.setProperty("--word-color", hero ? st.hero_color || "currentColor" : st.color || "currentColor");
    chip.style.setProperty("--word-scale", String(hero ? Math.min(1.7, (st.hero_size || 110) / (st.size || 64)) : 1));
  }
  if (wordsPanel.designs.has(w.line_id)) chip.dataset.changed = "";
  return chip;
}
// Keep every original word ID and audio anchor. Inserted wording shares the nearest existing beat.
function wordsPresentationEdits(words, text) {
  const old = words.flatMap((w, slot) => w.text.split(/\s+/).filter(Boolean).map((text) => ({ text, slot })));
  const next = text.trim().split(/\s+/).filter(Boolean);
  if (next.length > 256 || text.length > 4096) throw new Error("Keep a lyric line under 256 words");
  const dp = Array.from({ length: old.length+1 }, () => Array(next.length+1).fill(0));
  for (let i=old.length; i>=0; i--) for (let j=next.length; j>=0; j--) {
    if (i === old.length) dp[i][j] = next.length-j;
    else if (j === next.length) dp[i][j] = old.length-i;
    else dp[i][j] = Math.min((old[i].text === next[j] ? 0 : 1)+dp[i+1][j+1], 1+dp[i+1][j], 1+dp[i][j+1]);
  }
  const slots = words.map(() => []); let i=0, j=0;
  while (i<old.length || j<next.length) {
    if (i<old.length && j<next.length && dp[i][j] === (old[i].text === next[j] ? 0 : 1)+dp[i+1][j+1]) { slots[old[i].slot].push(next[j]); i++; j++; }
    else if (i<old.length && dp[i][j] === 1+dp[i+1][j]) i++;
    else { slots[old[Math.max(0,i-1)]?.slot || 0].push(next[j]); j++; }
  }
  return words.map((w, slot) => ({ word_id:w.word_id, display_text:slots[slot].join(" ") }));
}
function wordsBeginEdit(anim, lineId, event) {
  if (wordsPanel.mode !== "lyrics" || wordsPanel.editor) return;
  const node = [...$("view-design").querySelectorAll(".words-line")].find((n) => n.dataset.animationId === anim.animation_id && n.dataset.lineId === lineId);
  if (!node) return;
  const words = wordsLineWords(anim,lineId), original = words.map(w=>w.text).join(" ");
  const spans = [...node.querySelectorAll(".word")].map((chip) => {
    const span = el("span", { class:"word lettered" }, `${chip.textContent} `);
    for (let i=0;i<chip.style.length;i++) { const key=chip.style[i]; if(key.startsWith("--word-"))span.style.setProperty(key,chip.style.getPropertyValue(key)); }
    return span;
  });
  node.replaceChildren(...spans); node.setAttribute("contenteditable","true"); node.setAttribute("role","textbox");
  node.setAttribute("aria-label","Edit lyric line"); node.setAttribute("spellcheck","true");
  const editor = wordsPanel.editor = { node, animation:anim.animation_id, lineId, words, original, text:original, revision:anim.source_revision, project:state.project, dirty:false };
  node.oninput = () => { editor.text=node.innerText.trim().replace(/\s+/g," "); editor.dirty=editor.text!==original; wordsPanel.autoPlay=false; wordsVideo("pause"); };
  node.onpaste = (e) => { e.preventDefault(); document.execCommand("insertText",false,e.clipboardData.getData("text/plain").replace(/\s+/g," ")); };
  node.onkeydown = (e) => {
    if (e.isComposing) return;
    if(e.key === "Escape") { e.preventDefault(); e.stopPropagation(); wordsPanel.editor=null; wordsRender(); }
    else if(e.key === "Enter") { e.preventDefault(); e.stopPropagation(); wordsCommitEdit(editor); }
  };
  node.onblur = () => wordsCommitEdit(editor);
  node.focus();
  const selection = getSelection(), range = event && document.caretRangeFromPoint?.(event.clientX,event.clientY);
  if(range && node.contains(range.startContainer)){selection.removeAllRanges();selection.addRange(range);}
  else {const end=document.createRange();end.selectNodeContents(node);end.collapse(false);selection.removeAllRanges();selection.addRange(end);}
}
async function wordsCommitEdit(editor) {
  if (editor.promise) return editor.promise;
  if (wordsPanel.editor !== editor) return true;
  if (!editor.dirty) { wordsPanel.editor=null; editor.node.removeAttribute("contenteditable"); return true; }
  editor.promise = (async () => {
    try {
      const result=await wordsCommand("words.save",{animation_id:editor.animation,expected_source_revision:editor.revision,words:wordsPresentationEdits(editor.words,editor.text),origin:"lyrics-inline",_expected_project:editor.project});
      if(result.conflict) throw new Error("This line changed elsewhere. Your text is still here; copy it before reloading.");
      if(wordsPanel.read.in_video?.includes(editor.animation)) await wordsCommand("words.apply",{animation_id:editor.animation,_expected_project:editor.project});
      if(wordsPanel.editor===editor) wordsPanel.editor=null;
      if(state.project===editor.project){wordsPanel.stage=null;await wordsOpen();}
      return true;
    } catch(error) {editor.promise=null;editor.node.setAttribute("aria-invalid","true");toast(String(error).replace(/^.*?: /,""));return false;}
  })();
  return editor.promise;
}
function wordsAfterEdit(action) {
  const editor=wordsPanel.editor;
  if(!editor){action();return;}
  wordsCommitEdit(editor).then((saved)=>{if(saved)action();});
}

// The design items the picked words belong to (or the one clicked item).
function wordsPickedItems(design) {
  const picked = design.items.flatMap((it, i) => it.word_ids.some((id) => wordsPanel.picked.includes(id)) ? [i] : []);
  return picked.length ? picked : wordsPanel.item == null ? [] : [wordsPanel.item];
}

// Stretches of the song with no sung line (the intro and the breaks between sections): instrumentals.
function wordsGaps(animations) {
  const spans = animations.map(wordsSpan).sort((a, b) => a.start - b.start), gaps = [];
  let at = 0;
  for (const s of spans) { if (s.start - at > 1.5) gaps.push([at, s.start]); at = Math.max(at, s.end); }
  return gaps.map(([a, b]) => wordsPlace(el("div", { class: "words-gap", title: `No singing from ${wordsClock(a)} to ${wordsClock(b)}` },
    el("span", {}, "Instrumental"), el("span", { class: "meta" }, `${wordsClock(a)} – ${wordsClock(b)}`)), a, b - a));
}

// Left: the lyrics on the song clock, each section a band and each sung line its tags.
function wordsTimeline(animations, end) {
  return wordsPlace(el("div", { class: "words-tl", "aria-label": "Lyrics on the song timeline" }, ...wordsGaps(animations),
    ...animations.map((a) => wordsBand(a, "button", { "data-section": a.animation_id, onclick: () => a.animation_id === wordsPanel.selected || wordsSelect(a.animation_id) })),
    ...wordsRows(animations).map(([a, j, line, at, len]) => wordsPlace(el("div", { class: `words-line${j ? "" : " first"}${a.animation_id === wordsPanel.selected && wordsPanel.line === line.id ? " active" : ""}`,
      "data-kind": a.scope?.section === "chorus" ? "chorus" : "verse", "data-animation-id": a.animation_id, "data-line-id":line.id, tabindex:"0",
      ondblclick:(event)=>{event.preventDefault();wordsBeginEdit(wordsCurrent(),line.id,event);},
      onkeydown:(event)=>{if(event.target.isContentEditable)return;if(event.key==="Enter" || (event.key.length===1&&!event.ctrlKey&&!event.metaKey)){event.preventDefault();wordsBeginEdit(wordsCurrent(),line.id);if(event.key!=="Enter")document.execCommand("insertText",false,event.key);}},
      title: "Click around the words to design the whole line",
      onclick: (event) => { if(event.currentTarget.isContentEditable)return; if(wordsPanel.editor){const target=event.currentTarget;wordsAfterEdit(()=>target.click());return;} if (a.animation_id !== wordsPanel.selected) wordsSelect(a.animation_id); if (a.animation_id !== wordsPanel.selected) return;
        wordsPanel.line = line.id; wordsPanel.item = null; wordsPanel.picked = []; wordsPanel.time = wordsLineSpan(a, line.id).start; wordsPanel.previewRange = null; wordsPanel.autoPlay = true; wordsRender(); wordsVideoRange();
        requestAnimationFrame(()=>wordsBeginEdit(wordsCurrent(),line.id,event)); } },
      ...line.words.map((w) => wordsChip(a, w))), at, len)),
    el("span", { class: "words-tl-head" })), 0, end);
}

// Right: independently timed visual teaching cues. Concurrent cues occupy separate lanes.
function wordsRoll(animations, end) {
  const cues = wordsPanel.read.graphics || [], laneEnds = [];
  const placed = cues.map((cue) => {
    let lane = laneEnds.findIndex((t) => t <= cue.start);
    if (lane < 0) lane = laneEnds.length;
    laneEnds[lane] = cue.end; return { cue, lane };
  });
  const frames = placed.map(({ cue, lane }) => {
    const anim = animations.find((a) => a.animation_id === cue.animation_id);
    const thumb = (progress) => {
      const image = cue.kind === "drawing" ? wordsStroke(cue.stroke) : wordsOptionPreview("overlays", cue.overlay);
      image.setAttribute("class", "words-cue-preview");
      if (cue.kind === "drawing") {
        const path = image.querySelector("path"); path.setAttribute("pathLength", "1");
        path.setAttribute("stroke-dasharray", "1"); path.setAttribute("stroke-dashoffset", String(1-progress));
      }
      return image;
    };
    const handle = (edge, label) => cue.index == null && cue.kind !== "overlay" ? null : el("button", { class: `words-cue-handle ${edge}`, "aria-label": `${label} ${cue.name}`, title: `${label} · drag or use ↑ ↓ (Shift for fine timing)`,
      onpointerdown: (event) => wordsCueDrag(event, cue, edge), onclick: (event) => event.stopPropagation(),
      onkeydown: (event) => {
        if (!["ArrowUp", "ArrowDown"].includes(event.key)) return;
        event.preventDefault(); event.stopPropagation();
        if (cue.animation_id !== wordsPanel.selected) wordsSelect(cue.animation_id);
        const delta = (event.key === "ArrowUp" ? -1 : 1) * (event.shiftKey ? .01 : .1);
        const start = edge === "start" ? Math.max(0, cue.start+delta) : cue.start;
        wordsRetimeCue(cue,start,edge === "start" ? start+cue.end-cue.start : Math.max(start+.05,cue.end+delta));
      } }, edge === "start" ? "●" : "━");
    const frame = wordsPlace(el("div", { class: `words-graphic${wordsPanel.cue === cue.id ? " active" : ""}`, "data-cue-id": cue.id, role: "button", tabindex: "0",
      "aria-label": `${cue.name} at ${wordsClock(cue.start)}`, title: `${cue.name}${cue.word ? ` · “${cue.word}”` : ""} · ${wordsClock(cue.start)} – ${wordsClock(cue.end)} · Click to replay this connection`,
      onclick: () => wordsPreviewCue(cue), onkeydown: (event) => { if (event.key === "Enter" || event.key === " ") { event.preventDefault(); wordsPreviewCue(cue); } } },
      el("div", { class: "words-cue-film" }, thumb(.35), thumb(.7), thumb(1)),
      handle("start", "Move"), handle("end", "Trim end of"),
      el("span", { class: "words-cue-caption" }, cue.name)), cue.start, cue.end - cue.start);
    frame.style.setProperty("--cue-lane", String(lane));
    frame.style.setProperty("--cue-ink", cue.color || "var(--accent)");
    if (cue.draw_end != null) frame.style.setProperty("--draw-share", `${100 * (cue.draw_end - cue.start) / (cue.end - cue.start)}%`);
    return frame;
  });
  const roll = wordsPlace(el("div", { class: "words-tl words-roll", "aria-label": "Motion graphics on the song timeline" },
    ...Array.from({ length: Math.ceil(end / 10) }, (_, i) => wordsPlace(el("span", { class: "words-tick" }, wordsClock(i * 10)), i * 10)),
    ...frames, !cues.length ? el("p", { class: "words-graphics-empty" }, "Pick a word, then add a drawing to show its meaning here.") : null,
    el("span", { class: "words-tl-head" })), 0, end);
  roll.style.setProperty("--cue-lanes", String(Math.max(1, laneEnds.length)));
  return roll;
}

// The same library path is shown in the picker, motion-graphics track and rendered video.
function wordsStroke(stroke) {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg"), path = document.createElementNS("http://www.w3.org/2000/svg", "path");
  svg.setAttribute("viewBox", stroke.view_box); svg.setAttribute("aria-hidden", "true");
  path.setAttribute("d", stroke.d); path.setAttribute("fill", "none"); path.setAttribute("stroke", "currentColor");
  path.setAttribute("stroke-width", "2"); path.setAttribute("vector-effect", "non-scaling-stroke"); path.setAttribute("stroke-linecap", "round");
  svg.appendChild(path); return svg;
}

function wordsCueIntersects(cue, animationId, start, end, wordIds) {
  return cue.animation_id === animationId && (cue.word_ids?.length
    ? cue.word_ids.some((id) => wordIds.includes(id)) : cue.start < end && cue.end > start);
}
function wordsCueWindow(cue, line) {
  return { start: Math.max(0, Math.min(cue.start, line.start) - .25), end: Math.max(cue.end, line.end) + .25 };
}
function wordsPreviewCue(cue) {
  const anim = wordsPanel.read.document.animations.find((a) => a.animation_id === cue.animation_id);
  if (!anim) return;
  if (anim.animation_id !== wordsPanel.selected) wordsSelect(anim.animation_id);
  if (anim.animation_id !== wordsPanel.selected) return;
  wordsPanel.line = cue.line_id || wordsLines(anim)[0]?.id;
  wordsPanel.picked = cue.word_ids || []; wordsPanel.cue = cue.id;
  wordsPanel.previewRange = wordsCueWindow(cue, wordsLineSpan(anim, wordsPanel.line));
  wordsPanel.autoPlay = true; wordsPanel.time = wordsPanel.previewRange.start;
  wordsRender(); wordsVideoRange();
}
async function wordsRetimeCue(cue, start, end) {
  const anim = wordsCurrent();
  if (!anim?.kinetic || (cue.index == null && cue.kind !== "overlay")) return;
  start = Math.max(0,start);
  end = Math.max(start + .05, end);
  wordsPanel.previewRange = wordsCueWindow({ ...cue, start, end }, wordsLineSpan(anim, cue.line_id || wordsLines(anim)[0]?.id));
  wordsPanel.autoPlay = false;
  // An overlay is timed from its anchor (the engine's overlay_offset / overlay_duration).
  if (cue.kind === "overlay") return wordsKinetic({ overlay_offset: +(start - cue.anchor).toFixed(2), overlay_duration: +(end - start).toFixed(2) });
  const duration = Math.min((cue.draw_end ?? end) - cue.start, end - start);
  const draws = anim.kinetic.draws.map((d, i) => i === cue.index ? { ...d, offset: +(start - cue.anchor).toFixed(2), duration: +duration.toFixed(2), hold: +(end - start - duration).toFixed(2) } : d);
  await wordsKinetic({ draws });
}
function wordsCueDrag(event, cue, edge) {
  event.preventDefault(); event.stopPropagation(); wordsVideo("pause");
  if (cue.animation_id !== wordsPanel.selected) wordsSelect(cue.animation_id);
  if (cue.animation_id !== wordsPanel.selected) return;
  const view = $("view-design");
  view.setPointerCapture(event.pointerId);
  wordsPanel.cueDrag = { cue, before: { ...cue }, edge, y: event.clientY, scale: parseFloat(getComputedStyle(view).getPropertyValue("--song-s")) || 18 };
}
addEventListener("pointermove", (event) => {
  const d = wordsPanel.cueDrag;
  if (!d) return;
  const delta = (event.clientY - d.y) / d.scale;
  if (d.edge === "start") {
    d.cue.start = Math.max(0, Math.min(d.before.anchor + 600, Math.max(d.before.anchor - 600, d.before.start + delta)));
    d.cue.end = d.cue.start + d.before.end - d.before.start;
    if (d.before.draw_end != null) d.cue.draw_end = d.cue.start + d.before.draw_end - d.before.start;
  } else d.cue.end = Math.max(d.cue.start + .05, Math.min(d.cue.start + (d.cue.kind === "overlay" ? 600 : 30), d.before.end + delta));
  const frame = [...$("view-design").querySelectorAll(".words-graphic")].find((f) => f.dataset.cueId === d.cue.id);
  if (frame) wordsPlace(frame, d.cue.start, d.cue.end - d.cue.start);
  wordsArrows($("view-design"));
});
addEventListener("pointerup", () => {
  const d = wordsPanel.cueDrag;
  if (!d) return;
  wordsPanel.cueDrag = null;
  const { start, end } = d.cue; Object.assign(d.cue, d.before);
  wordsRetimeCue(d.before, start, end);
});
addEventListener("pointercancel", () => {
  const d = wordsPanel.cueDrag;
  if (!d) return;
  Object.assign(d.cue, d.before); wordsPanel.cueDrag = null; wordsRender();
});


function wordsNearestAnchor(time) {
  let nearest=null;
  for(const anim of wordsPanel.read?.document?.animations||[])for(const word of anim.words) {
    const at=wordsAt(word);
    if(!Number.isFinite(at))continue;
    if(!nearest||Math.abs(at-time)<nearest.distance)nearest={anim,word,at,distance:Math.abs(at-time)};
  }
  return nearest;
}
async function wordsRelinkCue(cue,target) {
  if(!target||cue.index==null)return;
  const source=wordsPanel.read.document.animations.find(a=>a.animation_id===cue.animation_id),project=state.project;
  if(!source)return;
  wordsVideo("pause");
  try {
    const result=await wordsCommand("words.link",{animation_id:source.animation_id,index:cue.index,word_id:target.word.word_id,target_animation_id:target.anim.animation_id,
      expected_source_revision:source.source_revision,expected_target_revision:target.anim.source_revision,_expected_project:project});
    if(result.conflict){toast("These lyrics changed elsewhere; reloaded before moving the link");return wordsOpen();}
    for(const id of result.changed_sections)if(wordsPanel.read.in_video?.includes(id))await wordsCommand("words.apply",{animation_id:id,_expected_project:project});
    if(state.project!==project)return;
    wordsPanel.selected=result.animation_id;wordsPanel.line=target.word.line_id;wordsPanel.picked=[target.word.word_id];
    wordsPanel.cue=`${result.animation_id}-draw-${result.index}`;wordsPanel.stage=null;wordsPanel.previewRange=null;await wordsOpen();
  } catch(error){toast(String(error).replace(/^.*?: /,""));if(state.project===project)await wordsOpen();}
}
function wordsAnchorDrag(event,cue) {
  event.preventDefault();event.stopPropagation();wordsVideo("pause");
  const view=$("view-design");view.setPointerCapture(event.pointerId);wordsPanel.linkDrag={cue,target:null};
}
addEventListener("pointermove",event=>{
  const drag=wordsPanel.linkDrag;if(!drag)return;
  const view=$("view-design"),tl=view.querySelector("#words-lyrics .words-tl"),r=tl.getBoundingClientRect();
  const scale=parseFloat(getComputedStyle(view).getPropertyValue("--song-s"))||18;
  drag.target=event.clientX>=r.left-24&&event.clientX<=r.right+36?wordsNearestAnchor((event.clientY-r.top)/scale):null;
  wordsArrows(view);
});
addEventListener("pointerup",()=>{
  const drag=wordsPanel.linkDrag;if(!drag)return;wordsPanel.linkDrag=null;
  if(drag.target)wordsRelinkCue(drag.cue,drag.target);else wordsArrows($("view-design"));
});
addEventListener("pointercancel",()=>{if(wordsPanel.linkDrag){wordsPanel.linkDrag=null;wordsArrows($("view-design"));}});

// Connections describe meaning and timing, never matching row numbers.
function wordsArrows(view) {
  view.querySelector(":scope > svg.words-arrows")?.remove();
  const lines = [...view.querySelectorAll(".words-tl > .words-line")], frames = [...view.querySelectorAll(".words-roll > .words-graphic")];
  if (!lines.length || !frames.length || !frames[0].offsetParent) return;
  const box = view.getBoundingClientRect(), svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("class", `words-arrows${wordsPanel.links.pulse?" blending":""}${wordsPanel.links.front?" front":""}`);
  svg.setAttribute("role","group");svg.setAttribute("aria-label","Lyric and drawing connections");
  svg.style.setProperty("--arrows-width", `${view.scrollWidth}px`); svg.style.setProperty("--arrows-height", `${view.scrollHeight}px`);
  const at = (r, side) => [r[side] - box.left + view.scrollLeft, r.top + r.height / 2 - box.top + view.scrollTop];
  for (const frame of frames) {
    const cue = wordsPanel.read.graphics.find((c) => c.id === frame.dataset.cueId);
    if (!cue) continue;
    for (const line of lines) {
      const start = Number(line.style.getPropertyValue("--at-s")), end = start + Number(line.style.getPropertyValue("--len-s"));
      const chips = [...line.querySelectorAll("[data-word-id]")];
      if (!wordsCueIntersects(cue, line.dataset.animationId, start, end, chips.map((c) => c.dataset.wordId))) continue;
      const source = chips.find((c) => cue.word_ids?.includes(c.dataset.wordId)) || line;
      const [edge] = at(line.getBoundingClientRect(), "right"), [x2] = at(frame.getBoundingClientRect(), "left");
      const scale=parseFloat(getComputedStyle(view).getPropertyValue("--song-s"))||18;
      const timeline=line.parentElement.getBoundingClientRect();
      const target=wordsPanel.linkDrag?.cue.id===cue.id?wordsPanel.linkDrag.target:null;
      const x1=edge+5,y1=timeline.top-box.top+view.scrollTop+(target?.at??cue.anchor??start)*scale;
      const r = frame.getBoundingClientRect(), fraction = (Math.max(start, cue.start) - cue.start) / (cue.end - cue.start);
      const y2 = r.top - box.top + view.scrollTop + Math.min(r.height - 4, Math.max(4, r.height * fraction));
      const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
      path.setAttribute("d", `M ${x1} ${y1} C ${(x1 + x2) / 2} ${y1}, ${(x1 + x2) / 2} ${y2}, ${x2 - 6} ${y2} l -12 -6 m 12 6 l -12 6`);
      const active=wordsPanel.cue?wordsPanel.cue===cue.id:line.classList.contains("active");
      path.setAttribute("class",active?"active":"");path.setAttribute("data-kind",line.dataset.kind);
      path.style.setProperty("--link-a",wordsPanel.links.color||(line.dataset.kind==="chorus"?"var(--tone-lyrics)":"var(--tone-transcript)"));
      path.style.setProperty("--link-b",`color-mix(in srgb, ${cue.color||"var(--accent)"} 55%, var(--text))`);
      path.setAttribute("aria-hidden","true");path.onclick=()=>wordsPreviewCue(cue);svg.appendChild(path);
      if(cue.index!=null&&wordsPanel.mode==="timing") {
        const handle=document.createElementNS("http://www.w3.org/2000/svg","circle");
        for(const [key,value] of Object.entries({cx:x1,cy:y1,r:7,class:"words-link-anchor",role:"button",tabindex:0,"aria-label":`Relink ${cue.name} from ${cue.word||"this word"}`}))handle.setAttribute(key,String(value));
        handle.onpointerdown=event=>wordsAnchorDrag(event,cue);handle.onclick=event=>{event.stopPropagation();wordsPreviewCue(cue);};
        handle.onkeydown=event=>{
          if(event.key==="Enter"){event.preventDefault();wordsPreviewCue(cue);return;}
          if(!["ArrowUp","ArrowDown"].includes(event.key))return;event.preventDefault();
          const all=wordsPanel.read.document.animations.flatMap(anim=>anim.words.map(word=>({anim,word,at:wordsAt(word)}))).filter(x=>Number.isFinite(x.at)).sort((a,b)=>a.at-b.at);
          const index=all.findIndex(x=>x.anim.animation_id===cue.animation_id&&x.word.word_id===cue.word_ids?.[0]);
          wordsRelinkCue(cue,all[index+(event.key==="ArrowUp"?-1:1)]);
        };svg.appendChild(handle);
      }
    }
  }
  view.appendChild(svg);
}

// Centre, under the stage: the line's minimal lanes. Timing only; the Studio owns clip length.
function wordsLanes(anim) {
  if (!wordsPanel.line) return null;
  const span = wordsLineSpan(anim, wordsPanel.line), from = span.start - 0.4, length = span.end - from;
  const words = wordsLineWords(anim, wordsPanel.line), design = wordsDesign(anim, wordsPanel.line);
  const at = (t) => `${(100 * (t - from) / length).toFixed(3)}%`;
  const block = (text, start, duration, onclick) => {
    const node = el("button", { class: "lane-block", title: `${text} · ${wordsClock(start)}`, onclick }, text);
    node.style.setProperty("--start", at(start)); node.style.setProperty("--len", `${(100 * Math.max(0.15, duration) / length).toFixed(3)}%`);
    return node;
  };
  const lane = (key, name, blocks) => el("div", { class: "lane", "data-track": key }, el("span", { class: "lane-name" }, name),
    el("div", { class: "lane-track" }, ...blocks, el("span", { class: "lane-head" })));
  const accent = (it) => words.find((w) => w.word_id === it.accent);
  wordsPanel.lanes = el("div", { class: "words-lanes", "aria-label": "Line timing" },
    lane("text", "Sung words", words.map((w) => { const clip = wordsClip(w.word_id); return block(w.text, wordsAt(w), clip?.duration ?? (+w.end - +w.start), () => { wordsPanel.time = wordsAt(w); wordsCanvasDraw(); wordsNow(wordsPanel.time); }); })),
    lane("motion", "Accents", design.items.flatMap((it, i) => it.motion === "quiet" || !accent(it) ? [] : [block(WORDS_MOVES.find((m) => m[0] === it.motion)?.[1] || it.motion, wordsAt(accent(it)), 0.65, () => { wordsPanel.item = i; wordsPanel.tab = "move"; wordsRender(); })])),
    lane("draw", "Drawings", design.draws.flatMap((d) => d.paths.map((p) => block(d.name, span.start + p.from, p.to - p.from, () => { wordsPanel.tab = "draw"; wordsRender(); })))));
  return wordsPanel.lanes;
}

// The playhead, on every timeline of the page; while playing, the page keeps it in view.
function wordsNow(t) {
  const view = $("view-design"), anim = wordsCurrent();
  view?.style.setProperty("--now-s", t.toFixed(3));
  if (anim && wordsPanel.line && wordsPanel.lanes) {
    const span = wordsLineSpan(anim, wordsPanel.line), from = span.start - 0.4;
    wordsPanel.lanes.style.setProperty("--at", `${(100 * wordsClamp((t - from) / (span.end - from))).toFixed(3)}%`);
  }
  if (wordsPanel.playing) wordsFollow(t);
}
// The lyrics and the film roll move with the song: the moment t sits level with the transport under the
// stage, so the playheads track playback there. While playing this tracks every frame; a new pick glides.
function wordsFollow(t, smooth = false) {
  const view = $("view-design"), tl = view?.querySelector(".words-tl");
  const eye = view?.querySelector(".words-preview .transport, .words-toolbar") || view?.querySelector(".lyric-canvas, .words-preview");
  if (!tl || !eye) return;
  const seconds = Number(tl.style.getPropertyValue("--len-s")), box = view.getBoundingClientRect();
  // Both measured in the view's scroll space (offsetTop is relative to the column, not the view).
  const tlTop = tl.getBoundingClientRect().top - box.top + view.scrollTop, eyeY = eye.getBoundingClientRect().top - box.top;
  const target = tlTop + tl.clientHeight * t / seconds - eyeY;
  view.scrollTo({ top: Math.max(0, target), behavior: smooth ? "smooth" : "instant" });
}

const wordsCard = (on, title, does, onclick) => el("button", { class: "effect", role: "radio", "aria-checked": String(!!on), title: `${title} · ${does}`, "aria-label": title, onclick },
  el("strong", {}, title), el("span", { class: "muted" }, does));

// What the centre offers for the pick: Look, Move or Draw, for the whole line or one item.
function wordsOptions(anim) {
  const lineId = wordsPanel.line, design = wordsDesign(anim, lineId), it = design.items[wordsPanel.item], words = wordsLineWords(anim, lineId);
  const itemText = (x) => x.word_ids.map((id) => words.find((w) => w.word_id === id)?.text).join(" ");
  const edit = (change) => wordsDesignEdit(anim, lineId, change);
  const tab = wordsPanel.tab;
  if (tab === "look" && !it) return [el("div", { class: "group-label" }, "Line designs"), el("div", { class: "words-effects" },
    wordsCard(false, "Plain line", "Every word along the bottom, quiet", () => edit((d) => Object.assign(d, wordsLayout(anim, lineId, "plain")))),
    wordsCard(false, "Poster", "The longest word big and italic, the rest small around it", () => edit((d) => Object.assign(d, wordsLayout(anim, lineId, "poster")))),
    wordsCard(false, "Stack", "One word per row, the last one bold and settling", () => edit((d) => Object.assign(d, wordsLayout(anim, lineId, "stack")))),
    anim.design?.[lineId] || wordsPanel.designs.has(lineId) ? wordsCard(false, "Clear the design", "Back to a plain line when saved", () => { wordsPanel.designs.set(lineId, null); wordsRender(); }) : null)];
  const picks = wordsPickedItems(design);
  const each = (d, change) => picks.forEach((i) => change(d.items[i]));
  if (tab === "look") {
    const look = WORDS_LETTERING.find((l) => l[3] === (it.font || "song") && l[4] === it.weight && l[5] === it.italic)?.[0];
    const range = (label, key, min, max, unit = "") => {
      const input = el("input", { type: "range", min: String(min), max: String(max), step: "1", "aria-label": label,
        oninput: (e) => {
          const held = wordsPanel.designs.get(lineId) ?? structuredClone(design);
          each(held, (x) => { x[key] = Number(e.target.value); }); wordsPanel.designs.set(lineId, held); wordsCanvasDraw();
        }, onchange: () => wordsRender() });
      input.value = input.defaultValue = String(it[key]);
      return el("label", { class: "field-row" }, `${label} ${unit}`, input);
    };
    const colour = el("input", { type: "color", "aria-label": "Colour", onchange: (e) => edit((d) => each(d, (x) => { x.color = e.target.value; })) });
    colour.value = colour.defaultValue = /^#[0-9a-f]{6}$/i.test(it.color) ? it.color : "#000000";
    // An outline keeps a word readable over the scene.
    const outline = el("input", { type: "color", "aria-label": "Outline colour", onchange: (e) => edit((d) => each(d, (x) => { x.outline = e.target.value; })) });
    outline.value = outline.defaultValue = /^#[0-9a-f]{6}$/i.test(it.outline || "") ? it.outline : "#173b43";
    const outlined = el("input", { type: "checkbox", "aria-label": "Outline on", onchange: (e) => edit((d) => each(d, (x) => { x.outline = e.target.checked ? outline.value : null; })) });
    outlined.checked = outlined.defaultChecked = !!it.outline;
    return [el("div", { class: "group-label" }, `Lettering for “${picks.map((i) => itemText(design.items[i])).join(" ")}”`), el("div", { class: "words-effects" },
      ...WORDS_LETTERING.map(([key, name, does, font, weight, italic]) => wordsCard(look === key, name, does, () => edit((d) => each(d, (x) => Object.assign(x, { font, weight, italic })))))),
      el("div", { class: "words-head" }, el("label", { class: "field-row" }, "Colour", colour), el("label", { class: "field-row" }, outlined, "Outline", outline),
        range("Size", "size", 8, 600, "px"), range("Tilt", "rotate", -45, 45, "°"))];
  }
  if (tab === "move" && !it) return [el("div", { class: "group-label" }, "Moves in this line"), el("div", { class: "words-effects" },
    ...design.items.map((x, i) => wordsCard(false, `“${itemText(x)}”`, x.motion === "quiet" ? "Quiet" : `${WORDS_MOVES.find((m) => m[0] === x.motion)?.[1]} on “${words.find((w) => w.word_id === x.accent)?.text ?? itemText(x)}”`,
      () => { wordsPanel.item = i; wordsRender(); })))];
  if (tab === "move") {
    const accent = el("select", { "aria-label": "Accent word", onchange: (e) => edit((d) => { d.items[wordsPanel.item].accent = e.target.value || null; }) },
      el("option", { value: "" }, "the line's start"), ...it.word_ids.map((id) => el("option", { value: id }, `“${words.find((w) => w.word_id === id)?.text}”`)));
    accent.value = it.accent || "";
    return [el("div", { class: "group-label" }, `How “${itemText(it)}” moves`), el("div", { class: "words-effects" },
      ...WORDS_MOVES.map(([key, name, does]) => wordsCard(it.motion === key, name, does, () => edit((d) => {
        const x = d.items[wordsPanel.item]; x.motion = key; if (key !== "quiet" && !x.accent) x.accent = x.word_ids.at(-1);
      })))),
      el("div", { class: "words-head" }, el("label", { class: "field-row" }, "Lands on", accent))];
  }
  // draw: the line's strokes, then the library
  const near = it || design.items[0];
  return [el("div", { class: "group-label" }, "Drawings in this line · click one to remove it"), el("div", { class: "words-effects" },
    ...design.draws.map((d, i) => wordsCard(true, d.name, `draws ${wordsClock(wordsLineSpan(anim, lineId).start + d.paths[0].from)}`, () => edit((x) => { x.draws.splice(i, 1); }))),
    design.draws.length ? null : el("p", { class: "muted" }, "No drawings yet.")),
    el("div", { class: "group-label" }, `Add a drawing ${it ? `by “${itemText(it)}”` : "to this line"}`), el("div", { class: "words-effects" },
      ...WORDS_STROKES.map((s) => wordsCard(false, s.name, s.does, () => edit((d) => {
        const accentWord = words.find((w) => w.word_id === near.accent), start = (accentWord ? wordsAt(accentWord) : wordsLineSpan(anim, lineId).start) - wordsLineSpan(anim, lineId).start;
        d.draws.push({ name: s.name, view_box: s.view_box, x: Math.round(near.x), y: Math.round(near.y + near.size * 1.15), w: s.w, h: s.h,
          paths: [{ d: s.d, stroke: near.color, width: 6, from: Math.max(0, +start.toFixed(2)), to: +(Math.max(0, start) + s.draw).toFixed(2) }] });
      }))))];
}

const WORDS_VIEW_ICONS = {
  timing:'<svg viewBox="0 0 24 24"><path d="M4 5v14M20 5v14M4 9h16M4 15h16"/><circle cx="9" cy="9" r="2"/><circle cx="16" cy="15" r="2"/></svg>',
  lyrics:'<svg viewBox="0 0 24 24"><path d="M4 5h16M12 5v14M8 19h8"/></svg>',
  motion:'<svg viewBox="0 0 24 24"><path d="M3 17c4-13 8 6 13-8l4-4M14 5h6v6"/></svg>'
};
function wordsModeTabs() {
  return el("div",{id:"words-view-modes",class:"segmented",role:"tablist","aria-label":"Lyrics view"},
    ...Object.entries(WORDS_VIEW_ICONS).map(([mode,html])=>el("button",{role:"tab","data-words-mode":mode,"aria-selected":String(wordsPanel.mode===mode),
      "aria-label":`${mode[0].toUpperCase()+mode.slice(1)} view`,title:`${mode[0].toUpperCase()+mode.slice(1)} controls`,onclick:()=>wordsSetMode(mode)},
      el("span",{class:"words-view-icon",html}),mode[0].toUpperCase()+mode.slice(1))));
}
function wordsSetMode(mode) {
  if(!Object.hasOwn(WORDS_VIEW_ICONS,mode))return;
  if(wordsPanel.editor){wordsAfterEdit(()=>wordsSetMode(mode));return;}
  if(mode!=="motion"){document.querySelector(".words-sketch")?.remove();document.querySelector(".words-sketch-cancel")?.remove();}
  wordsPanel.mode=mode;
  const view=$("view-design");if(view)view.dataset.wordsMode=mode;
  $("words-view-modes")?.querySelectorAll("[data-words-mode]").forEach(n=>n.setAttribute("aria-selected",String(n.dataset.wordsMode===mode)));
  const controls=$("words-workspace-controls"),anim=wordsCurrent();
  if(controls&&anim)controls.replaceChildren(...wordsInnerControls(anim));
  if(view)requestAnimationFrame(()=>{wordsWidth(view);wordsFit();wordsArrows(view);});
}
function wordsLinkControls() {
  const links=wordsPanel.links;
  const refresh=()=>{try{localStorage.setItem("magicstudio.lyrics.links",JSON.stringify(links));}catch{}wordsArrows($("view-design"));};
  const colour=el("input",{type:"color","aria-label":"Connection colour",title:"Connection colour",oninput:e=>{links.color=e.target.value;e.target.defaultValue=e.target.value;refresh();}});
  colour.value=colour.defaultValue=links.color||"#70c4ff";
  const toggle=(key,label,icon)=>el("button",{class:"icon","aria-label":label,title:label,"aria-pressed":String(links[key]),html:icon,onclick:e=>{links[key]=!links[key];e.currentTarget.setAttribute("aria-pressed",String(links[key]));refresh();}});
  return el("div",{id:"words-link-controls",class:"transport"},el("span",{class:"group-label"},"Connections"),colour,
    toggle("front","Bring connections in front",'<svg viewBox="0 0 24 24"><rect x="4" y="8" width="12" height="12"/><path d="M8 4h12v12M11 13l8-8M13 5h6v6"/></svg>'),
    toggle("pulse","Slow connection colour blend",'<svg viewBox="0 0 24 24"><path d="M3 12c3-8 6 8 9 0s6 8 9 0"/></svg>'));
}
function wordsMotionActions(anim) {
  return el("div",{class:"transport","aria-label":"Add visual cue"},
    el("button",{class:"start","aria-label":"Draw cue on scene",onclick:()=>wordsDrawCue(anim)},"Draw cue"),
    el("button",{class:"ghost","aria-label":"Add scene effect",onclick:()=>{wordsPanel.motionAdd="effect";wordsRender();}},"Add effect"));
}
function wordsDrawingEdit(anim,cue) {
  if(cue.index==null)return null;
  const draw=anim.kinetic.draws[cue.index];
  const change=patch=>wordsKinetic({draws:anim.kinetic.draws.map((d,i)=>i===cue.index?{...d,...patch}:d)});
  const field=(key,label)=>{const input=el("input",{type:"number",placeholder:"Auto","aria-label":`${label} of drawing`,onchange:e=>{
    if(e.target.value!==""&&Number.isFinite(Number(e.target.value))){e.target.defaultValue=e.target.value;change({[key]:Number(e.target.value)});}}});
    input.value=input.defaultValue=draw[key]??"";return el("label",{class:"field-row"},label,input);};
  const color=el("input",{type:"color","aria-label":"Drawing colour",onchange:e=>{e.target.defaultValue=e.target.value;change({color:e.target.value});}});
  color.value=color.defaultValue=draw.color||"#e4572e";
  return el("div",{class:"words-drawing-edit"},field("x","X"),field("y","Y"),field("width","Size"),color,
    el("button",{class:"ghost","aria-label":"Redraw selected cue",onclick:()=>wordsDrawCue(anim,cue.index)},"Redraw"));
}
function wordsDrawCue(anim,replaceIndex=null) {
  const stage=$("words-preview");if(!stage||stage.querySelector(".words-sketch"))return;
  const previous=replaceIndex==null?null:anim.kinetic?.draws?.[replaceIndex];
  const word=anim.words.find(w=>w.word_id===(previous?.word_id||wordsPanel.picked[0]))||wordsLineWords(anim,wordsPanel.line)[0];
  if(!word)return toast("Select a lyric line to attach the drawing");
  const project=state.project,size=wordsPanel.size||{width:1920,height:1080};wordsVideo("pause");
  const overlay=document.createElementNS("http://www.w3.org/2000/svg","svg"),path=document.createElementNS(overlay.namespaceURI,"path");
  overlay.setAttribute("viewBox",`0 0 ${size.width} ${size.height}`);overlay.setAttribute("class","words-sketch");
  overlay.setAttribute("tabindex","0");overlay.setAttribute("aria-label","Draw a cue on the scene; Escape cancels");
  path.setAttribute("fill","none");path.setAttribute("stroke",previous?.color||"#e4572e");path.setAttribute("stroke-width","7");path.setAttribute("stroke-linecap","round");overlay.appendChild(path);
  const cancel=el("button",{class:"words-sketch-cancel","aria-label":"Cancel drawing",onclick:()=>{overlay.remove();cancel.remove();}},"Cancel drawing");
  stage.appendChild(overlay);stage.appendChild(cancel);overlay.focus();
  let points=[],active=false,pending=false,stored=false;
  const savedIndex=replaceIndex??anim.kinetic?.draws?.length??0;
  const point=e=>{const r=overlay.getBoundingClientRect();return [Math.round(Math.max(0,Math.min(size.width,(e.clientX-r.left)*size.width/r.width))),Math.round(Math.max(0,Math.min(size.height,(e.clientY-r.top)*size.height/r.height)))];};
  const paint=()=>path.setAttribute("d",points.map((p,i)=>`${i?"L":"M"}${p[0]} ${p[1]}`).join(" "));
  overlay.onpointerdown=e=>{if(pending)return;e.preventDefault();overlay.setPointerCapture(e.pointerId);points=[point(e)];active=true;paint();};
  overlay.onpointermove=e=>{if(!active||points.length>=500)return;const p=point(e),last=points.at(-1);if(Math.hypot(p[0]-last[0],p[1]-last[1])>2){points.push(p);paint();}};
  const save=async()=>{
    if(points.length<2)return;
    if(state.project!==project||wordsPanel.selected!==anim.animation_id)return toast("Selection changed; drawing was not saved");
    pending=true;
    const drawing={...previous,word_id:word.word_id,track:"graphics",drawing:{name:"Drawing",view_box:`0 0 ${size.width} ${size.height}`,d:path.getAttribute("d")},
      x:0,y:0,width:size.width,color:previous?.color||"#e4572e",duration:previous?.duration||1.2,hold:previous?.hold??1};
    const draws=[...(anim.kinetic?.draws||[])];if(replaceIndex==null)draws.push(drawing);else draws[replaceIndex]=drawing;
    try {
      const kinetic={layout:"karaoke",entrance:"fade-up",overlay:"none",...anim.kinetic,draws};
      if(!stored){
        const result=await wordsCommand("words.save",{animation_id:anim.animation_id,expected_source_revision:anim.source_revision,kinetic,_expected_project:project});
        if(result.conflict)throw new Error("This section changed; cancel and reload before drawing again");
        anim.source_revision=result.animation.source_revision;anim.kinetic=kinetic;stored=true;
      }
      if(wordsPanel.read.in_video?.includes(anim.animation_id))await wordsCommand("words.apply",{animation_id:anim.animation_id,_expected_project:project});
      wordsPanel.cue=`${anim.animation_id}-draw-${savedIndex}`;wordsPanel.stage=null;wordsPanel.motionAdd=null;
      overlay.remove();cancel.remove();await wordsOpen();
    }catch(error){pending=false;cancel.textContent="Retry save";cancel.setAttribute("aria-label","Retry drawing save");cancel.onclick=save;toast(String(error));}
  };
  overlay.onpointerup=()=>{if(!active)return;active=false;save();};
  overlay.onpointercancel=()=>{active=false;points=[];paint();};
  overlay.onkeydown=e=>{if(e.key==="Escape"){overlay.remove();cancel.remove();}};
}

// The picked cue (drawing or effect): exact Start and End seconds and a replay, in Timing and Motion.
function wordsCueEditor(anim) {
  const cue=wordsPanel.read.graphics?.find(c=>c.id===wordsPanel.cue&&c.animation_id===anim.animation_id);
  if(!cue)return el("p",{class:"muted"},"Select a drawing or effect clip on the right. Drag its top to move it, its lower edge to trim it, or type exact seconds here.");
  const field=(label,value,apply)=>{const input=el("input",{type:"number",step:"0.01",min:"0","aria-label":`${label} of ${cue.name} (seconds)`,
    onchange:e=>{const v=Number(e.target.value);if(e.target.value!=="" && Number.isFinite(v) && v>=0){e.target.defaultValue=e.target.value;apply(v);}}});input.value=input.defaultValue=value.toFixed(2);return el("label",{class:"field-row"},label,input);};
  return el("div",{class:"words-timing-summary","data-cue-kind":cue.kind},el("strong",{},cue.name),
    field("Start",cue.start,v=>wordsRetimeCue(cue,v,v+cue.end-cue.start)),field("End",cue.end,v=>wordsRetimeCue(cue,cue.start,v)),
    el("button",{class:"icon","aria-label":"Replay linked learning moment",title:"Replay the lyric and this cue together",html:ICONS.play,onclick:()=>wordsPreviewCue(cue)}),
    wordsPanel.mode==="motion" ? wordsDrawingEdit(anim,cue) : null,
    el("button",{class:"icon","aria-label":"Delete selected visual cue",title:"Delete this cue",onclick:()=>{wordsPanel.cue=null;wordsKinetic(cue.kind==="overlay"?{overlay:"none"}:{draws:anim.kinetic.draws.filter((_,i)=>i!==cue.index)});},html:'<svg viewBox="0 0 24 24"><path d="M4 6h16M9 6V3h6v3M6 6l1 15h10l1-15M10 10v7M14 10v7"/></svg>'}),
    el("p",{class:"muted"},cue.word?`Linked to “${cue.word}” · drag the left point to choose another word.`:"This effect overlaps the connected lyric lines."));
}
function wordsTimingPanel(anim) {
  return el("section",{class:"words-timing","aria-label":"Timing controls"},wordsLinkControls(),wordsCueEditor(anim));
}
function wordsInnerControls(anim) {
  if(wordsPanel.mode==="timing")return [wordsTimingPanel(anim)];
  if(anim.kinetic)return wordsPanel.mode==="motion"?[wordsMotionActions(anim),wordsCueEditor(anim),wordsLayers(anim)]:[wordsLayers(anim)];
  wordsPanel.tab=wordsPanel.mode==="motion"?"draw":"look";
  return [wordsLanes(anim),...wordsOptions(anim)];
}

function wordsRender() {
  if(wordsPanel.editor?.dirty) return;
  const view = $("view-design");
  const anim = wordsCurrent();
  const animations = wordsPanel.read?.document?.animations || [];
  if (!anim) { view.replaceChildren(el("p", { class: "panel-empty inline" }, "No lyric sections yet.")); return; }
  if (!wordsLines(anim).some((l) => l.id === wordsPanel.line)) { wordsPanel.line = wordsLines(anim)[0]?.id || null; wordsPanel.item = null; wordsPanel.time = wordsPanel.line ? wordsPose(anim, wordsPanel.line) : 0; }
  const end = Math.max(...animations.map((a) => wordsSpan(a).end)) + 4;
  const design = wordsPanel.line ? wordsDesign(anim, wordsPanel.line) : null, it = design?.items[wordsPanel.item];
  const lineNo = wordsLines(anim).findIndex((l) => l.id === wordsPanel.line) + 1;
  const itemText = it ? it.word_ids.map((id) => anim.words.find((w) => w.word_id === id)?.text).join(" ") : "";

  const changes = wordsPanel.designs.size + Object.keys(wordsPanel.styleEdits).length;
  const saveLabel=changes ? `Save ${changes} change${changes > 1 ? "s" : ""}` : "Saved";
  const save = el("button", { id:"words-save", class:"icon", title:saveLabel, "aria-label":saveLabel, onclick:wordsSave,
    html:'<svg viewBox="0 0 24 24"><path d="M5 3h12l4 4v14H3V3h2zM7 3v6h9V3M7 21v-8h10v8"/></svg>' });
  save.disabled = !changes;
  const segmented = (label, current, options, pick) => el("div", { class: "segmented", role: "tablist", "aria-label": label },
    ...options.map(([key, name]) => el("button", { role: "tab", "aria-selected": String(current === key), onclick: () => pick(key) }, name)));

  const designer = wordsPanel.view === "design" && wordsPanel.line && !anim.kinetic;
  const main = el("section", { id: "words-main", "aria-label": `${wordsLabel(anim)} lyrics` },
    el("div", { id: "words-main-head" },
      el("span", { class: "words-crumb", title:`${wordsLabel(anim)} · line ${lineNo}${it ? ` · ${itemText}` : ""}` }, el("strong", {}, wordsLabel(anim)), ` · ${lineNo}`),
      wordsModeTabs(),save),
    wordsPanel.read.reference_changed ? el("p", { class: "not-linked" }, "The timing reference changed since this was saved; check the words before saving.") : null,
    designer ? wordsCanvas(anim) : wordsStage(anim),
    // The stage's controls sit in one row under it, so the stage stays high in the column.
    designer ? el("div", { class: "words-toolbar" },
      designer ? el("button", { class: "icon", "aria-label": "Play this line", title: "Play from this line", onclick: wordsPlay, html: ICONS.play }) : null,
      designer ? el("span", { class: "meta" }, wordsClock(wordsLineSpan(anim, wordsPanel.line).start)) : null,

      designer ? segmented("Behind the lyrics", wordsPanel.scene ? "scene" : "paper", [["scene", "Scene"], ["paper", "Paper"]], (key) => { wordsPanel.scene = key === "scene"; wordsRender(); }) : null,
      el("span", { class: "words-spacer" }),
      wordsPanel.read.in_video?.includes(anim.animation_id)
        ? el("button", { class: "ghost", title: "Put the video's original lyrics back for this section", onclick: () => wordsApply(false) }, "In the video · Restore original")
        : el("button", { class: "ghost", title: "Replace this section's lyrics in the video with the designed lines", onclick: () => wordsApply(true) }, "Use in the video")) : null,
    el("div",{id:"words-workspace-controls"},...wordsInnerControls(anim)));

  view.replaceChildren(
    el("aside", { id: "words-lyrics" }, el("div", { id: "words-lyrics-head" }, el("strong", { class: "panel-title" }, "Lyrics · on the song timeline")), wordsTimeline(animations, end)),
    main,
    el("aside", { id: "words-film" }, el("div", { id: "words-film-head" }, el("strong", { class: "panel-title" }, "Visual cues · motion graphics")), wordsRoll(animations, end)));
  view.dataset.wordsMode=wordsPanel.mode;
  wordsViewFitter.observe(view); if($("magic-workstation"))wordsViewFitter.observe($("magic-workstation")); wordsWidth(view);
  requestAnimationFrame(() => {
    wordsCanvasDraw(); wordsFitClock(view); wordsArrows(view);
    // A new pick brings its line level with the stage.
    const pick = `${wordsPanel.selected}/${wordsPanel.line}`;
    if (pick !== wordsPanel.followed && wordsPanel.line && !wordsPanel.playing) { wordsPanel.followed = pick; wordsFollow(wordsLineSpan(anim, wordsPanel.line).start, true); }
  });
  wordsNow(wordsPanel.time);
  wordsFit();
}

// The song clock grows until every line's tags fit in its stretch of time; timing never changes.
function wordsFitClock(view) {
  view.style.removeProperty("--song-s");
  const base = parseFloat(getComputedStyle(view).getPropertyValue("--song-s-px")) || 14;
  let scale = base;
  for (const row of view.querySelectorAll(".words-tl > .words-line")) {
    const seconds = Number(row.style.getPropertyValue("--len-s"));
    if (seconds > 0) scale = Math.max(scale, (row.scrollHeight + parseFloat(getComputedStyle(row).marginTop || 0) + 4) / seconds);
  }
  if (scale > base) view.style.setProperty("--song-s", `${Math.ceil(scale)}px`);
}

function wordsSizeInput(anim) {
  const input = el("input", { type: "range", min: "60", max: "220", step: "2", "aria-label": "Word size",
    oninput: (e) => { wordsPanel.styleEdits.size = Number(e.target.value); if (wordsPanel.styleEdits.size === (anim.style?.size ?? 120)) delete wordsPanel.styleEdits.size; wordsRender(); } });
  input.value = String(wordsPanel.styleEdits.size ?? anim.style?.size ?? 120);
  input.defaultValue = String(anim.style?.size ?? 120); // only a moved slider is an unsaved edit
  return input;
}

// Dragging a word on the stage moves it; the edit is held until Save.
addEventListener("pointermove", (event) => {
  const drag = wordsPanel.drag, anim = wordsCurrent();
  if (!drag || !anim) return;
  const design = wordsPanel.designs.get(wordsPanel.line) ?? structuredClone(wordsDesign(anim, wordsPanel.line));
  design.items[drag.i].x = Math.round(drag.from[0] + (event.clientX - drag.x) / drag.scale);
  design.items[drag.i].y = Math.round(drag.from[1] + (event.clientY - drag.y) / drag.scale);
  wordsPanel.designs.set(wordsPanel.line, design); drag.moved = true;
  wordsCanvasDraw();
});
addEventListener("pointerup", () => { const drag = wordsPanel.drag; wordsPanel.drag = null; if (drag?.moved) wordsRender(); });
addEventListener("keydown", (event) => {
  if (state.view !== "design" || event.key !== "Escape" || wordsPanel.item == null || /INPUT|SELECT|TEXTAREA/.test(event.target.tagName)) return;
  wordsPanel.item = null; wordsRender();
});

// The section's other style settings words.save accepts: font, colours and background.
function wordsStyleFields(anim) {
  const field = (key, label, type, fallback) => {
    const saved = anim.style?.[key] ?? fallback;
    const input = el("input", { type, "aria-label": label, oninput: (e) => {
      if (e.target.value === saved) delete wordsPanel.styleEdits[key]; else wordsPanel.styleEdits[key] = e.target.value; } ,
      onchange: () => wordsRender() });
    input.value = wordsPanel.styleEdits[key] ?? saved;
    input.defaultValue = saved; // only a changed field is an unsaved edit
    return el("label", { class: "field-row" }, label, input);
  };
  return [field("font", "Font", "text", ""), field("color", "Colour", "color", "#ffffff"), field("emphasis_color", "Emphasis", "color", "#ffd078"),
    field("outline_color", "Outline", "color", "#173b43"), field("background", "Background", "text", "")];
}

function wordsUseButton(anim) {
  const used=wordsPanel.read.in_video?.includes(anim.animation_id);
  const label=used?"Restore original video lyrics":"Use this lyric design in the video";
  return el("button",{class:"icon",title:label,"aria-label":label,"aria-pressed":String(!!used),onclick:()=>wordsApply(!used),
    html:'<svg viewBox="0 0 24 24"><rect x="3" y="4" width="18" height="16" rx="2"/><path d="m7 12 3 3 7-7"/></svg>'});
}

function wordsBackdropPicker(anim) {
  const backgrounds=wordsPanel.backgrounds||{images:[]};
  return el("div",{class:"words-backdrop",role:"group","aria-label":"Storyboard backgrounds"},
    el("span",{class:"meta"},"Scene"),
    backgrounds.images?.length ? el("div",{class:"words-backgrounds"},...backgrounds.images.map(image=>el("button",{
      class:"words-background",title:image.name,"aria-label":`Use background ${image.name}`,"aria-pressed":String(anim.background_asset===image.asset_id),
      onclick:async()=>{
        try {const result=await wordsCommand("words.save",{animation_id:anim.animation_id,expected_source_revision:anim.source_revision,background_asset:image.asset_id,_expected_project:state.project});
          if(result.conflict)toast("This verse changed elsewhere; reloaded");
          wordsPanel.stage=null;await wordsOpen();
        }catch(error){toast(String(error));}
      }
    },el("img",{src:convertFileSrc(image.path),alt:image.name,loading:"lazy"}))))
      : el("span",{class:"muted",title:backgrounds.error||"Assign images in the storyboard’s Backgrounds & locations section"},
        backgrounds.error ? "Storyboard backgrounds unavailable" : "Add backgrounds in Storyboard"));
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
    oninput: (e) => { e.target.defaultValue = e.target.value; wordsSeekSong(Number(e.target.value)); } });
  scrub.value = scrub.defaultValue = String(section.start); // a view control, never an unsaved edit
  // The preview player of the HyperFrames server, same project id as the Studio link.
  const server = new URL(url);
  const project = server.hash.split("/")[1] || "hyperframes";
  wordsPanel.player = `${server.origin}/api/projects/${project}/preview`;
  const frame = el("iframe", { id: "words-video-frame", title: "Lyric and drawing preview", allow: "autoplay" });
  frame.addEventListener("load", wordsVideoRange);
  const view = (show, label) => el("button", { role: "tab", "data-show": show, "aria-selected": String(wordsPanel.show === show), onclick: () => wordsShow(show) }, label);
  wordsPanel.stage = el("div", { class: "words-preview", "data-animation": anim.animation_id },
    anim.effects?.text ? el("div", { class: "segmented", role: "tablist", "aria-label": "What the stage shows" },
      view("still", "Scene image"), view("edited", `Edited · ${anim.effects.text.name}`)) : null,
    wordsBackdropPicker(anim),
    el("div", { class: "stage", id: "words-preview" }, frame),
    el("div", { class: "transport" },
      wordsPanel.playButton = el("button", { class: "icon", "aria-label": "Play this section", title: "Play this section of the video", onclick: () => wordsVideo(wordsPanel.playing ? "pause" : "play"), html: ICONS.play }),
      scrub,
      wordsPanel.timeLabel = el("span", { class: "meta" }, `${wordsClock(section.start)} – ${wordsClock(section.end)}`),wordsUseButton(anim)));
  if (wordsPanel.show === "edited" && !anim.effects?.text) wordsPanel.show = "still";
  wordsShow(wordsPanel.show, frame);
  return wordsPanel.stage;
}

// The stage shows the scene's backdrop with the music and the lyrics (words.lyrics_view show=stage), the whole video,
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
    if (["lyrics", "stage", "still"].includes(show)) {
      if(show === "still" && !anim.background_asset) { frame.removeAttribute("src"); return; }
      src = `${wordsPanel.player}/comp/${(await wordsCommand("words.lyrics_view", { show, animation_id:anim.animation_id })).src}`;
    }
    if (show === "edited") src = `${wordsPanel.player}/comp/${(await wordsCommand("words.compose", { animation_id: anim.animation_id })).src}`;
  } catch (error) { toast(String(error).replace(/^.*?: /, "")); return; }
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
  const anim = wordsCurrent();
  const section = wordsPanel.previewRange || (anim && wordsPanel.line ? wordsLineSpan(anim, wordsPanel.line) : wordsStudioSection(anim));
  if (!section) return;
  wordsVideo("pause");
  const o = wordsPanel.offset;
  wordsVideo("set-play-range", { startSeconds: Math.max(0, section.start - o), endSeconds: section.end - o });
  wordsSeekSong(section.start);
  if (wordsPanel.autoPlay) wordsVideo("play");
}

// The video's own size, as data; the stage rule scales the frame to fit.
function wordsFit(size = wordsPanel.size) {
  const frame = $("words-video-frame"), box = $("words-preview");
  if (!frame || !box || !size) return;
  wordsPanel.size = size;
  frame.style.setProperty("--w", `${size.width}px`); frame.style.setProperty("--h", `${size.height}px`);
  const scale=Math.min(box.clientWidth/size.width,box.clientHeight/size.height);
  frame.style.setProperty("--scale",String(scale));
  frame.style.setProperty("--frame-x",`${(box.clientWidth-size.width*scale)/2}px`);
  frame.style.setProperty("--frame-y",`${(box.clientHeight-size.height*scale)/2}px`);
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
    if (data.isPlaying) wordsPanel.autoPlay = false;
    const scrub = $("words-scrub");
    if (scrub) { scrub.value = String(song); scrub.defaultValue = scrub.value; }
    if (wordsPanel.timeLabel) wordsPanel.timeLabel.textContent = wordsClock(song);
    wordsNow(song);
    wordsPanel.playButton?.setAttribute("aria-label", data.isPlaying ? "Pause this section" : "Play this section");
  }
});
// The view's own width (not the window's) picks its layout; see #view-design.narrow / .tight.
function wordsWidth(view = $("view-design")) {
  if (!view) return;
  view.style.setProperty("--words-available", `${Math.max(220, innerHeight-Math.max(0,view.getBoundingClientRect().top)-48)}px`);
  view.classList.toggle("narrow", view.clientWidth < 1100);
  view.classList.toggle("tight", view.clientWidth < 600);
}
let wordsObservedWidth = -1;
const wordsViewFitter = new ResizeObserver((entries) => {
  const view=$("view-design");if(!view)return;
  const width=`${view.clientWidth}:${innerHeight}:${view.getBoundingClientRect().top}`;
  if(width===wordsObservedWidth)return; wordsObservedWidth=width;
  requestAnimationFrame(()=>{const view=$("view-design");if(view?.classList.contains("active")){wordsWidth(view);wordsFit();wordsFitClock(view);wordsArrows(view);}});
});
addEventListener("resize", () => { wordsWidth(); wordsFit(); const view = $("view-design"); if (view) wordsArrows(view); });
// Leaving the Design view stops the video, so nothing plays behind another view.
new MutationObserver(() => {
  if ($("view-design")?.classList.contains("active")) return;
  if (wordsPanel.playing) wordsVideo("pause");
})
  .observe(document.getElementById("view-design") || document.body, { attributes: true, attributeFilter: ["class"] });

// Put the section's designed lines into the video, or take them back out.
// Kinetic sections: the layers under the stage, each a row of library cards. A pick saves the section and,
// when it is in the video, composes it again so the stage shows the change.
async function wordsKinetic(change) {
  if(wordsPanel.editor){const saved=await wordsCommitEdit(wordsPanel.editor);if(!saved)return;}
  const anim = wordsCurrent();
  if (!anim?.kinetic) return;
  const kinetic = { ...anim.kinetic, ...change };
  try {
    const result = await wordsCommand("words.save", { animation_id: anim.animation_id, expected_source_revision: anim.source_revision, origin: "magicstudio-design", kinetic });
    if (result.conflict) { toast("Someone changed these lyrics first; reloaded the latest"); return wordsOpen(); }
    if (wordsPanel.read.in_video?.includes(anim.animation_id)) { await wordsCommand("words.apply", { animation_id: anim.animation_id }); wordsPanel.stage = null; }
    await wordsOpen();
  } catch (error) { toast(String(error).replace(/^.*?: /, "")); }
}
// Shuffle (or first make kinetic) one section from the project's lyric recipe, with a fresh seed.
async function wordsShuffle(kinetic = true) {
  const anim = wordsCurrent();
  if (!anim) return;
  try {
    if(kinetic && anim.kinetic) {
      const lib=wordsPanel.read.kinetic_library||{};
      const pick=(items,current)=>{const keys=Object.keys(items||{});return keys[Math.floor(Math.random()*keys.length)]||current;};
      return wordsKinetic({layout:pick(lib.layouts,anim.kinetic.layout),entrance:pick(lib.entrances,anim.kinetic.entrance),seed:Math.floor(Math.random()*1000)});
    }
    await wordsCommand("words.design", { section: anim.animation_id, seed: Math.floor(Math.random() * 1000), kinetic });
    wordsPanel.stage = null; await wordsOpen();
  } catch (error) { toast(String(error).replace(/^.*?: /, "")); }
}
// Tiny diagrams explain the choice before it is applied; motion plays only on hover/focus.
function wordsOptionPreview(kind, key) {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("viewBox", "0 0 160 64"); svg.setAttribute("aria-hidden", "true");
  svg.setAttribute("class", "words-option-preview"); svg.setAttribute("data-kind", kind); svg.setAttribute("data-choice", key);
  const add = (tag, attrs, text) => {
    const node = document.createElementNS("http://www.w3.org/2000/svg", tag);
    for (const [k, v] of Object.entries(attrs)) node.setAttribute(k, String(v));
    if (text) node.textContent = text; svg.appendChild(node); return node;
  };
  if (kind === "layouts") {
    const places = { karaoke: [[36,35],[68,35],[112,35]], stack: [[60,24],[99,24],[80,44]],
      cascade: [[28,19],[73,34],[122,49]], scatter: [[32,26,-9],[80,44,7],[126,23,-5]],
      push: [[36,35],[70,35],[116,35]], giant: [[27,18],[60,18],[87,48]] };
    (places[key] || places.karaoke).forEach(([x,y,angle], i) => add("text", { x, y, "text-anchor": "middle",
      class: i === 2 ? "hero" : "", "font-size": key === "giant" && i === 2 ? 29 : 12,
      transform: `rotate(${angle || 0} ${x} ${y})` }, ["Sing", "your", "song"][i]));
    if (key === "push") add("path", {d:"M 17 22 V 12 H 30 M 130 12 H 143 V 22 M 17 42 V 52 H 30 M 130 52 H 143 V 42", class:"cue"});
  } else if (kind === "entrances") {
    add("text", {x:80,y:40,"text-anchor":"middle","font-size":25,class:"preview-word"}, "Sing");
    const cues = { "fade-up":"M 80 59 V 47 m -4 4 4 -4 4 4", slide:"M 10 34 H 43 m -5 -4 5 4 -5 4",
      "drop-in":"M 80 5 V 17 m -4 -4 4 4 4 -4", pop:"M 38 18 l 7 5 M 122 18 l -7 5 M 38 49 l 7 -5 M 122 49 l -7 -5",
      typewriter:"M 112 20 V 43", "blur-in":"M 37 24 V 40 M 31 27 V 37 M 123 24 V 40 M 129 27 V 37" };
    add("path", {d:cues[key] || "",class:"cue"});
  } else {
    add("path", {d:"M 0 43 Q 20 31 40 43 T 80 43 T 120 43 T 160 43 M 0 54 Q 20 42 40 54 T 80 54 T 120 54 T 160 54",class:"scene"});
    if (key === "godrays") for (const x of [30,65,100,135]) add("path",{d:`M 20 0 L ${x} 64`,class:"light"});
    if (key === "lens-flare") for (const [x,y,r] of [[37,18,12],[73,30,7],[112,44,4]]) add("circle",{cx:x,cy:y,r,class:"light"});
    if (key === "ripples") for (const r of [10,21,32]) add("ellipse",{cx:80,cy:32,rx:r*1.7,ry:r*.5,class:"cue"});
    if (["snow","starfield","paper-grain","sparkle-lines"].includes(key)) for (let i=0;i<18;i++) {
      const x=(i*47+13)%160,y=(i*23+9)%60;
      if (key === "sparkle-lines") add("path",{d:`M ${x-3} ${y} h 6 M ${x} ${y-3} v 6`,class:"cue"});
      else add("circle",{cx:x,cy:y,r:key === "paper-grain" ? .7 : key === "snow" ? 2 : 1,class:"speck"});
    }
  }
  return svg;
}

function wordsLayers(anim) {
  const lib = wordsPanel.read.kinetic_library || {}, k = anim.kinetic;
  const row = (label, kind, current, pick) => [el("div", { class: "group-label" }, label), el("div", { class: "words-effects" },
    ...Object.entries(lib[kind] || {}).map(([key, item]) => {
      const card = wordsCard(current === key, item.name, item.does, () => pick(key));
      if (["layouts", "entrances", "overlays"].includes(kind)) card.prepend(wordsOptionPreview(kind, key));
      if (kind === "letterings") {
        const sample = el("span", { class: "words-lettering-sample", "aria-hidden": "true" }, "Sing ");
        const hero = el("span", {}, "out");
        sample.style.setProperty("--sample-font", `"${item.style.font}", sans-serif`);
        sample.style.setProperty("--sample-weight", String(item.style.weight || 700));
        sample.style.setProperty("--sample-color", item.style.color);
        sample.style.setProperty("--sample-outline", item.style.outline || "transparent");
        hero.style.setProperty("--hero-font", `"${item.style.hero_font || item.style.font}", sans-serif`);
        hero.style.setProperty("--hero-weight", String(item.style.hero_weight || 700));
        hero.style.setProperty("--hero-slant", item.style.hero_italic ? "italic" : "normal");
        hero.style.setProperty("--hero-color", item.style.hero_color);
        hero.style.setProperty("--hero-size", `${Math.min(1.7, (item.style.hero_size || 110) / (item.style.size || 64))}em`);
        sample.appendChild(hero); card.prepend(sample);
      }
      return card;
    }))];
  if(wordsPanel.mode==="motion" && wordsPanel.motionAdd==="effect")return el("section",{class:"words-layers"},...row("Add scene effect","overlays",k.overlay||"none",overlay=>wordsKinetic({overlay})));
  const picked = wordsPanel.picked.filter((id) => anim.words.some((w) => w.word_id === id));
  const text = (id) => anim.words.find((w) => w.word_id === id)?.text || id;
  const actsFor = (act) => { const acts = { ...k.acts }; for (const id of picked) { if (act) acts[id] = act; else delete acts[id]; } return acts; };
  return el("section", { class: "words-layers", "aria-label": `${wordsLabel(anim)} layers` },
    el("div", { class: "words-head" }, el("strong", {}, `${wordsPanel.mode === "motion" ? "Motion · drawings & effects" : "Lyrics · text & lettering"}`), el("span", { class: "words-spacer" }),
      wordsPanel.mode === "lyrics" ? el("button", { class: "ghost", title: "Shuffle the lyric layout and entrance; keep drawings", onclick: () => wordsShuffle(true) }, "Shuffle") : null),
    ...(wordsPanel.mode === "lyrics" ? [
    ...row("Layout", "layouts", k.layout, (layout) => wordsKinetic({ layout })),
    ...row("Words arrive", "entrances", k.entrance, (entrance) => wordsKinetic({ entrance })),
    el("div", { class: "group-label" }, "Meaning words · pick words in the lyrics, then an act"),
    // One chip per meaning word and act; a word sung several times picks every time it is sung.
    el("div", { class: "words-meaning" }, ...Object.values(Object.entries(k.acts || {}).reduce((groups, [id, act]) => {
      const key = `${text(id).toLowerCase().replace(/[^a-z]/g, "")}|${act}`;
      (groups[key] ??= { ids: [], act, word: text(id).replace(/[^\p{L}'-]/gu, "") }).ids.push(id);
      return groups;
    }, {})).map((g) => el("button", { class: `chip${g.ids.some((id) => picked.includes(id)) ? " active" : ""}`, title: "Pick every time this word is sung",
      onclick: () => { wordsPanel.picked = g.ids; wordsPanel.line = anim.words.find((w) => w.word_id === g.ids[0])?.line_id; wordsRender(); } },
      `${g.word}${g.ids.length > 1 ? ` ×${g.ids.length}` : ""} · ${lib.acts?.[g.act]?.name || g.act}`))),
    picked.length ? el("div", { class: "words-effects" },
      wordsCard(false, "No act", "Arrives like the other words", () => wordsKinetic({ acts: actsFor(null) })),
      ...Object.entries(lib.acts || {}).map(([key, item]) => wordsCard(picked.every((id) => k.acts?.[id] === key), item.name, item.does, () => wordsKinetic({ acts: actsFor(key) }))))
      : el("p", { class: "muted" }, "Click a word in the lyrics on the left to give it an act."),
    ...row("Lettering", "letterings", Object.entries(lib.letterings || {}).find(([, l]) => Object.entries(l.style).every(([key, value]) => k.style?.[key] === value))?.[0],
      (key) => wordsKinetic({ style: lib.letterings[key].style })),
    ] : []),
    ...(["lyrics","motion"].includes(wordsPanel.mode) && wordsPanel.motionAdd!=="effect" ? [
    // Drawings: a stroke anchored to a word, drawn on at its sung time (picked words get the stroke).
    el("div", { class: "group-label" }, wordsPanel.mode === "lyrics" ? "Letter marks · underline and decorate words" : "Drawings · select a cue to edit it"),
    el("div", { class: "words-meaning" }, ...(k.draws || []).map((d,i)=>({...d,index:i})).filter(d=>(d.track||lib.strokes?.[d.stroke]?.track||"graphics")===(wordsPanel.mode==="lyrics"?"lettering":"graphics")).map((d) => el("button", { class: "chip", title: `Edit ${d.drawing?.name || lib.strokes?.[d.stroke]?.name || d.stroke} for ${text(d.word_id)}`, "aria-label": `Edit drawing for ${text(d.word_id)}`,
      onclick: () => {wordsPanel.cue=`${anim.animation_id}-draw-${d.index}`;wordsPanel.picked=[d.word_id];wordsRender();} }, `${text(d.word_id)} · ${d.drawing?.name || lib.strokes?.[d.stroke]?.name || d.stroke}`))),
    picked.length ? el("div", { class: "words-effects" }, ...Object.entries(lib.strokes || {}).filter(([,item])=>(item.track||"graphics")===(wordsPanel.mode==="lyrics"?"lettering":"graphics")).map(([key, item]) => {
      const card = wordsCard(picked.every((id) => k.draws?.some((d) => d.word_id === id && d.stroke === key)), item.name, item.does,
        () => wordsKinetic({ draws: [...(k.draws || []).filter((d) => !(picked.includes(d.word_id)&&d.stroke===key)), ...picked.map((word_id) => ({ word_id, stroke: key, place: key === "down" ? "beside" : "under" }))] }));
      const svg = wordsStroke(item); svg.setAttribute("class", "words-stroke-sample"); card.prepend(svg); return card;
    }))
      : null,
    ...(wordsPanel.mode === "motion" ? row("Overlay · a transparent layer over the scene", "overlays", k.overlay || "none", (overlay) => wordsKinetic({ overlay })) : [])] : []));
}

async function wordsApply(use) {
  const anim = wordsCurrent();
  if (!anim) return;
  if (use && (wordsPanel.designs.size || Object.keys(wordsPanel.styleEdits).length)) return toast("Save the changes first");
  try {
    const result = await wordsCommand("words.apply", { animation_id: anim.animation_id, use });
    toast(use ? `${wordsLabel(anim)} now uses the designed lyrics in the video` : `${wordsLabel(anim)} has its original lyrics again`);
    if (result.changed) wordsPanel.stage = null;
    await wordsOpen();
  } catch (error) { toast(String(error).replace(/^.*?: /, "")); }
}

async function wordsSave() {
  const anim = wordsCurrent();
  if (!anim) return;
  const params = { animation_id: anim.animation_id, expected_source_revision: anim.source_revision, origin: "magicstudio-design" };
  if (wordsPanel.designs.size) params.design = Object.fromEntries(wordsPanel.designs);
  if (Object.keys(wordsPanel.styleEdits).length) params.style = wordsPanel.styleEdits;
  try {
    const result = await wordsCommand("words.save", params);
    if (result.conflict) { toast("Someone changed these lyrics first; reloaded the latest"); wordsPanel.designs.clear(); wordsPanel.styleEdits = {}; return wordsOpen(); }
    wordsPanel.designs.clear(); wordsPanel.styleEdits = {};
    // A section already in the video is composed again, so the video shows what was just saved.
    if (wordsPanel.read.in_video?.includes(anim.animation_id)) { await wordsCommand("words.apply", { animation_id: anim.animation_id }); wordsPanel.stage = null; }
    toast(`Saved ${wordsLabel(anim)} · rev ${result.animation.source_revision}${wordsPanel.read.in_video?.includes(anim.animation_id) ? " · the video is updated" : ""}`);
    await wordsOpen();
  } catch (error) { toast(String(error)); }
}
