// Theatre "Curves": every animated channel of the current clip, drawn from the engine's own
// sample (per-frame layer matrices from motion.sample) and channel summary (motion.tracks).
const timelineSeriesCache = new WeakMap();
let timelineWidth = 0;

// Per layer, per frame: rotation (deg), scale, pivot offset and opacity, read back from the matrices.
function timelineSeries(sample, meta) {
  if (timelineSeriesCache.has(sample)) return timelineSeriesCache.get(sample);
  const series = meta.layers.map((layer, i) => {
    const [px, py] = layer.pivot || [0, 0];
    const out = { rotation: [], scale: [], x: [], y: [], opacity: [] };
    for (const frame of sample.frames) {
      const l = frame.layers[i];
      const [a, b, c, d, e, f] = l?.matrix || [1, 0, 0, 1, 0, 0];
      out.rotation.push(Math.atan2(b, a) * 180 / Math.PI);
      out.scale.push(Math.hypot(a, b));
      out.x.push(a * px + c * py + e - px);
      out.y.push(b * px + d * py + f - py);
      out.opacity.push(l?.opacity ?? 1);
    }
    return out;
  });
  timelineSeriesCache.set(sample, series);
  return series;
}

const timelineSpan = (values) => Math.max(...values) - Math.min(...values);

// Properties the sample actually moves (standard actions also carry constant opacity keys).
function timelineMoving(series, properties) {
  const moves = { rotation: timelineSpan(series.rotation) > 0.05, position: Math.max(timelineSpan(series.x), timelineSpan(series.y)) > 0.05,
    scale: timelineSpan(series.scale) > 1e-3, opacity: timelineSpan(series.opacity) > 1e-3 };
  return properties.filter((property) => moves[property] ?? true);
}

// The curve a row shows: its first bound property in rotation, position, scale, opacity order.
function timelineCurve(series, properties) {
  for (const kind of ['rotation', 'position', 'scale', 'opacity']) {
    if (!properties.includes(kind)) continue;
    if (kind === 'position') return { kind, values: timelineSpan(series.y) >= timelineSpan(series.x) ? series.y : series.x };
    return { kind, values: series[kind] };
  }
  return { kind: properties[0] || 'rotation', values: series.rotation };
}

function timelineDraw(canvas, curve, keys, duration) {
  const dpr = devicePixelRatio || 1, w = canvas.clientWidth, h = canvas.clientHeight;
  if (!w || !h) return;
  canvas.width = Math.round(w * dpr); canvas.height = Math.round(h * dpr);
  const ctx = canvas.getContext('2d'); ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  const tone = getComputedStyle(document.documentElement).getPropertyValue(uiConfig.timeline_tones[curve.kind] || '--accent').trim();
  ctx.fillStyle = tone; ctx.globalAlpha = 0.1;
  ctx.beginPath(); ctx.roundRect(0, 2, w, h - 4, 3); ctx.fill(); ctx.globalAlpha = 1;
  const lo = Math.min(...curve.values), hi = Math.max(...curve.values), flat = hi - lo < 1e-6;
  const y = (v) => flat ? h / 2 : 4 + (h - 8) * (1 - (v - lo) / (hi - lo));
  ctx.strokeStyle = tone; ctx.lineWidth = 1.4; ctx.lineJoin = 'round'; ctx.beginPath();
  curve.values.forEach((v, i) => { const x = w * i / Math.max(1, curve.values.length - 1); i ? ctx.lineTo(x, y(v)) : ctx.moveTo(x, y(v)); });
  ctx.stroke();
  ctx.fillStyle = getComputedStyle(document.documentElement).getPropertyValue('--depth-gold').trim();
  for (const t of keys) {
    const x = w * Math.min(1, t / duration), frame = Math.round(Math.min(1, t / duration) * (curve.values.length - 1));
    ctx.save(); ctx.translate(x, y(curve.values[frame])); ctx.rotate(Math.PI / 4); ctx.fillRect(-3, -3, 6, 6); ctx.restore();
  }
}

function timelineRender() {
  const box = $('theatre-curves');
  if (!box || typeof box.replaceChildren !== 'function') return;
  if (!box.dataset.observed) { box.dataset.observed = 'true'; timelineResize.observe(box); }
  const meta = motion.meta, sample = motion.sample;
  const track = meta?.tracks?.find((t) => t.id === motion.track);
  if ($('theatre-info')) $('theatre-info').textContent = sample ? `Range 0.00–${sample.duration.toFixed(2)}s · ${sample.fps} fps · ${sample.loop ? 'loop' : 'once'}` : '';
  if (!meta || !sample) { box.replaceChildren(el('p', { class: 'panel-empty inline muted' }, 'Choose an action or clip to see its channels.')); return; }
  const series = timelineSeries(sample, meta);
  const rows = (motion.rest ? [] : track?.channels || [])
    .map((channel) => { const index = meta.layers.findIndex((l) => l.id === channel.layer); return { channel, index, properties: index < 0 ? [] : timelineMoving(series[index], channel.properties) }; })
    .filter((row) => row.properties.length);
  if (!rows.length) { box.replaceChildren(el('p', { class: 'panel-empty inline muted' }, motion.rest ? 'Rest pose · nothing moves. Choose an action or clip.' : 'This clip animates no channels.')); return; }
  const duration = sample.duration || 1;
  const tick = (at, text, end = false) => { const node = el('span', end ? { class: 'end' } : {}, text); node.style.setProperty('--tick-position', `${at}%`); return node; };
  const ruler = el('div', { class: 'theatre-ruler' }, ...Array.from({ length: Math.floor(duration) + 1 }, (_, s) => tick(100 * s / duration, `${s}s`)), duration % 1 ? tick(100, `${duration.toFixed(1)}s`, true) : null);
  const seek = (event) => {
    const r = event.currentTarget.getBoundingClientRect();
    animationPause(); motion.time = Math.max(0, Math.min(1, (event.clientX - r.left) / r.width)) * duration; animationDraw();
  };
  const drawn = [];
  const lines = rows.map(({ channel, index, properties }) => {
    const layer = meta.layers[index], name = layer.label || layer.id, picked = motion.selected === layer.id;
    const curve = timelineCurve(series[index], properties);
    const canvas = el('canvas', { class: 'theatre-curve', 'aria-hidden': 'true', onclick: seek });
    drawn.push([canvas, curve, channel.keys?.[curve.kind] || []]);
    const row = el('div', { class: `theatre-row${picked ? ' selected' : ''}` },
      el('button', { class: 'theatre-channel', title: `Select ${name}`, 'aria-pressed': String(picked),
        onclick: () => { motion.selected = layer.id; animationControls(); animationDraw(); updateMagicSelection(); } }, `${name} · ${properties.join(' + ')}`),
      el('span', { class: 'theatre-motion' }, channel.summary), canvas);
    row.style.setProperty('--tone', `var(${uiConfig.timeline_tones[curve.kind] || '--accent'})`); return row;
  });
  const picked = rows.find(({ index }) => meta.layers[index].id === motion.selected);
  const note = picked ? `${meta.layers[picked.index].label || picked.channel.layer} / ${picked.properties.join(' + ')} · ${picked.channel.summary}` : `${rows.length} animated parts · select one to follow it`;
  box.replaceChildren(
    el('div', { class: 'theatre-grid' },
      el('div', { id: 'theatre-ruler' }, el('span', {}, 'Layer / channel'), el('span', {}, 'Motion'), ruler),
      ...lines, el('div', { class: 'theatre-playhead', id: 'theatre-playhead' })),
    el('div', { class: 'theatre-foot muted' }, el('span', {}, note), el('span', {}, 'Curves show the clip’s source motion · Keys edits Theatre rotation')));
  for (const [canvas, curve, keys] of drawn) timelineDraw(canvas, curve, keys, duration);
  timelineWidth = box.clientWidth;
  timelineSeek(motion.time);
}

function timelineSeek(time) {
  const head = $('theatre-playhead');
  if (!head || !motion.sample) return;
  head.style.setProperty('--at', String(Math.max(0, Math.min(1, time / (motion.sample.duration || 1)))));
}

// Curves are drawn at the panel's width; redraw when it changes (not on every height change).
const timelineResize = new ResizeObserver(() => { const box = $('theatre-curves'); if (box?.clientWidth && box.clientWidth !== timelineWidth) timelineRender(); });
