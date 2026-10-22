// A separate image composer. It never reads or changes the chat/provider configuration.
const imageDrafts = new Map();

function selectedPartTarget() {
  if (!motion?.character || !motion?.selected || !motion?.meta?.layers) return null;
  const layer = motion.meta.layers.find((part) => part.id === motion.selected);
  if (!layer?.file || !state.project) return null;
  const root = state.project.slice(0, state.project.lastIndexOf('/'));
  const prefix = `${root}/`;
  const original = motion.assets?.find((asset) => asset.id === motion.character)?.path;
  if (!original?.startsWith(prefix) || !layer.file.startsWith(prefix) || original === layer.file) return null;
  return { project: state.project, assetId: motion.character, partId: layer.id, name: layer.label || layer.id,
    references: [original.slice(prefix.length), layer.file.slice(prefix.length)], file: layer.file, original };
}

// The parts panel uses this to display only the candidate generated for the captured part.
function imageCandidateForPart(assetId, partId) {
  const draft = imageDrafts.get(state.project);
  const target = draft?.candidateTarget;
  if (!draft?.result?.image || target?.assetId !== assetId || target?.partId !== partId) return null;
  return { ...target, image: draft.result.image, result: draft.result };
}

function imagePromptBlock() {
  const project = state.project;
  let draft = imageDrafts.get(project);
  if (!draft) { draft = { prompt: '', model: 'gpt-6-luna', busy: false, result: null, error: '', startedAt: null, target: null, candidateTarget: null, timer: null }; imageDrafts.set(project, draft); }
  const prompt = el('textarea', { id: 'image-prompt', rows: '3', placeholder: 'Describe the artwork or part repair…', 'aria-label': 'Image prompt' });
  prompt.value = prompt.defaultValue = draft.prompt;
  prompt.addEventListener('input', () => { draft.prompt = prompt.value; updateCommand(); });
  const model = el('input', { id: 'image-model', value: draft.model, list: 'image-model-options', 'aria-label': 'Codex image agent model', title: 'Selected Codex model; pixel dimensions are determined by its native image tool' });
  const models = el('datalist', { id: 'image-model-options' }, el('option', { value: 'gpt-6-luna' }), el('option', { value: 'gpt-6-sol' }));
  model.addEventListener('input', () => { draft.model = model.value; updateCommand(); });
  const command = el('code', { id: 'image-command' });
  const quote = value => `'${String(value).replaceAll("'", "'\\''")}'`;
  function updateCommand() {
    const ref = draft.target?.references ? ` ${quote(`references=${JSON.stringify(draft.target.references)}`)}` : '';
    command.textContent = `mc exec image.generate ${quote(`project=${project}`)} ${quote(`prompt=${draft.prompt}`)} ${quote(`model=${draft.model}`)} quality=auto${ref}`;
  }
  updateCommand();
  const elapsed = () => Math.max(0, Math.floor((performance.now() - draft.startedAt) / 1000));
  const targetLabel = draft.target ? `${draft.target.name} · ${draft.target.partId}` : '';
  const status = el('p', { id: 'image-status', role: 'status', 'aria-live': 'polite', class: draft.error ? 'image-error' : 'why' });
  function updateStatus() {
    if (draft.busy) status.textContent = `image.generate request running · ${elapsed()}s elapsed${targetLabel ? ` · target ${targetLabel}` : ''}`;
    else if (draft.error) status.textContent = draft.error;
    else if (draft.result) status.textContent = `Candidate · ${draft.result.width} × ${draft.result.height} · ${draft.result.native_tool_receipt_verified ? 'native image receipt verified' : 'receipt unavailable'} · ${draft.completedElapsed ?? 0}s elapsed · needs visual review${draft.candidateTarget ? ` · captured ${draft.candidateTarget.name} (${draft.candidateTarget.partId})` : ''}`;
    else status.textContent = 'Native Codex image tool · current login · quality and dimensions are tool-controlled.';
  }
  updateStatus();
  const generate = el('button', { id: 'image-generate', class: 'start', 'data-commands': 'image.generate', 'aria-label': 'Generate image candidate', onclick: async () => {
    if (!draft.prompt.trim()) { prompt.focus(); return; }
    if (draft.busy) return;
    // Freeze the target and reference at invocation time; later selection changes cannot retarget this job.
    if (motion?.selected && !selectedPartTarget()) {
      draft.error = 'This part has no project-local full source reference. Generation was not started.';
      updateStatus();
      return;
    }
    const captured = selectedPartTarget();
    draft.target = captured;
    draft.busy = true; draft.error = ''; draft.result = null; draft.startedAt = performance.now();
    generate.disabled = true;
    updateStatus();
    draft.timer = setInterval(updateStatus, 250);
    const submitted = { prompt: draft.prompt, model: draft.model, quality: 'auto' };
    if (captured) submitted.references = captured.references;
    try {
      const result = await invoke('engine_command', { id: 'image.generate', params: submitted });
      draft.completedElapsed = elapsed();
      draft.result = result;
      draft.candidateTarget = captured;
      if (state.project === project) {
        state.data = await invoke('project_state', { path: project });
        if (state.view === 'animation' && motion.meta) animationControls();
        renderBlocks();
        await acFeedback.report(null, null, { stage: 'image_block_rendered', command: 'image.generate', asset_id: result.asset_id, receipt: result.receipt, captured_part: captured?.partId || null });
      }
    } catch (error) { draft.completedElapsed = elapsed(); draft.error = String(error); }
    finally {
      clearInterval(draft.timer); draft.timer = null; draft.busy = false;
      if (state.project === project) renderBlocks();
    }
  } }, draft.busy ? 'Generating…' : 'Generate candidate');
  generate.disabled = draft.busy;
  const copy = el('button', { class: 'ghost', id: 'image-copy-command', title: 'Copy the exact image command without executing it', onclick: async () => {
    try { await navigator.clipboard.writeText(command.textContent); toast('Image command copied · nothing executed'); }
    catch { toast('Clipboard unavailable · select the command text'); }
  } }, 'Copy command');
  const body = [el('label', { for: 'image-prompt' }, 'Prompt'), prompt,
    el('label', { for: 'image-model' }, 'Codex model'), model, models, status, generate,
    el('details', {}, el('summary', {}, 'CLI command'), command, copy)];
  if (draft.result?.image) body.push(el('img', { class: 'image-result', src: convertFileSrc(draft.result.image), alt: 'Generated candidate artwork · needs visual review' }),
    el('button', { class: 'ghost', id: 'image-open-result', onclick: () => openPath(draft.result.image) }, 'Open generated image'));
  const section = block('image', ICONS.animate, 'Generate image', draft.busy ? `${Math.floor(elapsed())}s` : draft.result ? 'Candidate' : 'Codex · Luna', false, body, draft.error ? 'needs' : 'empty');
  section.classList.add('image-prompt-block');
  return section;
}
