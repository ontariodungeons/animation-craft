// Render/edit the existing native scene packets through their registered commands.
// This is a view over scene-N.md, not a DeckCraft editor or a second document store.
const sceneDrafts = new Map();
function renderNativeStoryboard(board, cards) {
  cards.classList.add('native-document');
  const project = state.project;
  const scenes = board.native_scenes || [];
  const toc = el('nav', { id: 'storyboard-contents', class: 'native-contents', 'aria-label': 'Storyboard contents' },
    el('h2', {}, `${$('project-name').textContent} · Storyboard`),
    el('p', { class: 'why' }, 'Editable native draft scenes · raw transcript preserved · DeckCraft slide integration is separate.'));
  for (const scene of scenes) toc.append(el('a', { href: `#${scene.target}`, 'data-scene': scene.scene }, `Scene ${scene.scene} · ${Number(scene.start).toFixed(3)}–${Number(scene.end).toFixed(3)}s · ${scene.status}`));
  cards.append(toc);
  const intro = el('section', { class: 'native-scene', id: 'storyboard-assets-intake' },
    el('h3', {}, 'Existing prepared assets'),
    el('p', { class: 'why' }, 'Which approved character artwork, clean background plates and official lyric/liner-note references do you already have? Keep originals linked; prepare or edit artwork outside Studio. Draft files are not approvals.'),
    el('button', { class: 'ghost', id: 'storyboard-review-assets', onclick: () => { state.open = 'assets'; state.blocksCollapsed = false; $('blocks').classList.remove('collapsed'); renderBlocks(); } }, 'Review project assets'));
  cards.append(intro);
  const field = (scene, key, line, label, saved, placeholder) => {
    const id = `scene-${scene.scene}-${key}${line ? '-'+line : ''}`;
    const cacheKey = `${project}:${id}`;
    let draft = sceneDrafts.get(cacheKey);
    if (!draft || draft.value === draft.saved) { draft = { value: saved || '', saved: saved || '', busy: false }; sceneDrafts.set(cacheKey, draft); }
    const input = el('textarea', { id, rows: key === 'lyric' ? '2' : '3', placeholder, 'aria-label': `${label}, scene ${scene.scene}` });
    input.defaultValue = draft.saved; input.value = draft.value;
    input.addEventListener('input', () => { draft.value = input.value; });
    const status = el('span', { id: id+'-status', class: 'why', role: 'status' }, draft.value !== draft.saved ? 'Unsaved edit' : '');
    const save = el('button', { id: id+'-save', class: 'ghost', 'data-commands': 'storyboard.edit', onclick: async () => {
      if (draft.busy) return;
      if (state.project !== project) { status.textContent = 'Project changed; edit preserved, save refused.'; return; }
      const submitted = input.value; draft.busy = true; save.disabled = true; status.textContent = 'Saving…';
      try {
        const result = await invoke('engine_command', { id: 'storyboard.edit', params: { scene: scene.scene, field: key, ...(line ? { line } : {}), value: submitted } });
        draft.saved = submitted; input.defaultValue = submitted;
        draft.value = input.value;
        status.textContent = draft.value === submitted ? 'Saved to scene packet · draft' : 'Saved; newer edit remains unsaved';
        await acFeedback.acknowledge(result._ui_engine_revision, project);
      } catch (error) { status.textContent = `Save blocked: ${String(error)} · edit retained`; }
      finally { draft.busy = false; save.disabled = false; }
    } }, 'Save');
    save.disabled = draft.busy;
    return el('div', { class: 'native-field' }, el('label', { for: id }, label), input, el('div', { class: 'native-field-actions' }, save, status));
  };
  for (const scene of scenes) {
    const section = el('article', { id: scene.target, class: 'native-scene', 'data-scene': scene.scene },
      el('header', {}, el('h3', {}, `Scene ${scene.scene}`), el('span', { class: 'native-status' }, scene.status || 'draft'),
        el('span', { class: 'native-time' }, `${Number(scene.start).toFixed(3)}–${Number(scene.end).toFixed(3)} s`)),
      el('p', { class: 'why' }, scene.kind === 'instrumental' ? 'Instrumental · no invented lyric text.' : 'Provisional transcript-derived grouping · review phrase boundaries.'),
      el('button', { class: 'ghost native-source', title: scene.source, onclick: () => openPath(scene.source) }, 'Scene source file'));
    for (const line of scene.lines || []) {
      section.append(el('p', { class: 'native-heard' }, `${Number(line.start).toFixed(3)} s · Heard: ${line.heard ?? line.lyric}`),
        field(scene, 'lyric', line.line, 'Reviewed lyric / correction', line.lyric, 'Review against the recording; the raw transcript is unchanged.'));
      if (line.panel) section.append(el('img', { class: 'native-reference', src: convertFileSrc(line.panel), alt: `Linked reference for scene ${scene.scene}; approval not inferred` }));
    }
    section.append(el('div', { class: 'native-art-slot' }, 'Prepared visual / staging reference needed. Link approved assets after external preparation; no image is generated or placed automatically.'));
    const prompts = el('details', {}, el('summary', {}, 'Image and animation prompts · optional draft fields'));
    for (const item of board.native_section_schema || []) {
      prompts.append(field(scene, item.field, null, item.label, scene.sections?.[item.field], `Draft ${item.label.toLowerCase()} instructions; leave blank until reviewed.`));
    }
    section.append(prompts); cards.append(section);
  }
}
