// A separate image composer. It never reads or changes the chat/provider configuration.
const imageDrafts = new Map();
function imagePromptBlock() {
  const project = state.project;
  let draft = imageDrafts.get(project);
  if (!draft) { draft = { prompt: '', model: 'gpt-6-luna', busy: false, result: null, error: '' }; imageDrafts.set(project, draft); }
  const prompt = el('textarea', { id: 'image-prompt', rows: '4', placeholder: 'Describe the missing artwork…', 'aria-label': 'Image prompt' });
  prompt.value = prompt.defaultValue = draft.prompt;
  prompt.addEventListener('input', () => { draft.prompt = prompt.value; updateCommand(); });
  const model = el('input', { id: 'image-model', value: draft.model, list: 'image-model-options', 'aria-label': 'Codex image agent model', title: 'Selected Codex model; pixel dimensions are determined by its native image tool' });
  const models = el('datalist', { id: 'image-model-options' }, el('option', { value: 'gpt-6-luna' }), el('option', { value: 'gpt-6-sol' }));
  model.addEventListener('input', () => { draft.model = model.value; updateCommand(); });
  const command = el('code', { id: 'image-command' });
  const quote = value => `'${String(value).replaceAll("'", "'\\''")}'`;
  function updateCommand() {
    command.textContent = `animationcraft-cli exec image.generate ${quote(`project=${project}`)} ${quote(`prompt=${draft.prompt}`)} ${quote(`model=${draft.model}`)} quality=auto`;
  }
  updateCommand();
  const status = el('p', { id: 'image-status', role: 'status', class: draft.error ? 'image-error' : 'why' },
    draft.busy ? 'Generating with Codex… Engine changes are queued until it finishes.' : draft.error || (draft.result ? `Draft · ${draft.result.width} × ${draft.result.height} · needs visual review` : 'Native Codex image tool · current login · no API key requested. Quality and dimensions are tool-controlled.'));
  const generate = el('button', { id: 'image-generate', class: 'start', 'data-commands': 'image.generate', 'aria-label': 'Generate and attach image', onclick: async () => {
    if (!draft.prompt.trim()) { prompt.focus(); return; }
    if (draft.busy) return;
    const submitted = { prompt: draft.prompt, model: draft.model, quality: 'auto' };
    draft.busy = true; draft.error = ''; generate.disabled = true;
    status.textContent = 'Generating with Codex… Engine changes are queued until it finishes.';
    try {
      draft.result = await invoke('engine_command', { id: 'image.generate', params: submitted });
      // Preserve anything typed during the job; no chat interaction or implicit Studio switch.
      if (state.project === project) {
        state.data = await invoke('project_state', { path: project });
        renderBlocks();
        await acFeedback.report(null, null, { stage: 'image_block_rendered', command: 'image.generate', asset_id: draft.result.asset_id });
      }
    } catch (error) { draft.error = String(error); }
    finally {
      draft.busy = false;
      if (state.project === project) renderBlocks();
    }
  } }, draft.busy ? 'Generating…' : 'Generate image');
  generate.disabled = draft.busy;
  const copy = el('button', { id: 'image-copy-command', class: 'ghost', title: 'Copy the exact image command without executing it', onclick: async () => {
    try { await navigator.clipboard.writeText(command.textContent); toast('Image command copied · nothing executed'); }
    catch { toast('Clipboard unavailable · select the command text'); }
  } }, 'Copy command');
  const body = [el('label', { for: 'image-prompt' }, 'Image prompt'), prompt,
    el('label', { for: 'image-model' }, 'Codex model'), model, models, status, generate,
    el('details', {}, el('summary', {}, 'CLI command'), command, copy)];
  if (draft.result?.image) body.push(el('img', { class: 'image-result', src: convertFileSrc(draft.result.image), alt: 'Generated draft artwork · needs visual review' }),
    el('button', { class: 'ghost', id: 'image-open-result', onclick: () => openPath(draft.result.image) }, 'Open generated image'));
  const section = block('image', ICONS.animate, 'Generate image', draft.busy ? 'Generating…' : draft.result ? 'Draft' : 'Codex · Luna', false, body, draft.error ? 'needs' : 'empty');
  section.classList.add('image-prompt-block');
  return section;
}
