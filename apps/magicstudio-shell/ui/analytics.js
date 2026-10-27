// Analytics view: code complexity (lizard), Omarchy & project token spend, and app/tool health.
const analyticsState = { data: null, section: 'overview', loading: false, error: null };
const ANALYTICS_SECTIONS = [
  ['overview', 'Overview'],
  ['tokens', 'Token Spend'],
  ['complexity', 'Code Complexity'],
  ['health', 'App & Tool Health'],
];

function fmtCount(n) {
  const num = Number(n || 0);
  if (num >= 1_000_000) return `${(num / 1_000_000).toFixed(2)}M`;
  if (num >= 1_000) return `${(num / 1_000).toFixed(1)}k`;
  return String(num);
}

async function renderAnalytics(scan = true) {
  const root = $('view-analytics');
  if (!root) return;
  analyticsMount();
  analyticsState.loading = true;
  analyticsState.error = null;
  const msg = $('analytics-message');
  if (msg) msg.textContent = scan ? 'Scanning code complexity & reading token telemetry…' : 'Reading telemetry…';
  try {
    analyticsState.data = await invoke('engine_command', { id: 'app.analytics', params: { scan } });
    analyticsState.loading = false;
    analyticsRenderSurface();
  } catch (e) {
    analyticsState.loading = false;
    analyticsState.error = String(e);
    if (msg) msg.textContent = analyticsState.error;
  }
}

function analyticsMount() {
  const root = $('view-analytics');
  if (!root || $('analytics-layout')) return;
  const nav = el('nav', { class: 'learning-categories', 'aria-label': 'Analytics sections' },
    ...ANALYTICS_SECTIONS.map(([id, label]) => el('button', {
      id: `analytics-nav-${id}`,
      class: 'ghost',
      title: `Show ${label}`,
      onclick: () => { analyticsState.section = id; analyticsRenderSurface(); },
    }, label))
  );
  const rail = el('aside', { class: 'learning-rail', id: 'analytics-rail', 'aria-label': 'Analytics navigation' },
    el('h2', {}, 'Analytics'),
    nav,
    el('div', { id: 'analytics-rail-summary', class: 'learning-task-list' }),
  );
  const refreshBtn = el('button', {
    id: 'analytics-refresh',
    class: 'start',
    title: 'Refresh token usage, host telemetry and run Lizard complexity scan',
    'data-commands': 'app.analytics',
    onclick: () => renderAnalytics(true),
  }, 'Refresh metrics');
  const main = el('main', { class: 'learning-main', id: 'analytics-main' },
    el('header', { class: 'learning-bar', id: 'analytics-bar' },
      el('span', { class: 'learning-breadcrumb', id: 'analytics-breadcrumb' }, 'Analytics · Overview'),
      el('div', { class: 'learning-actions' }, refreshBtn),
    ),
    el('p', { class: 'learning-message', id: 'analytics-message' }, ''),
    el('div', { class: 'learning-surface', id: 'analytics-surface' }),
  );
  root.replaceChildren(el('div', { class: 'learning-layout', id: 'analytics-layout' }, rail, main));
}

function analyticsRenderSurface() {
  const surface = $('analytics-surface');
  if (!surface) return;
  const d = analyticsState.data || {};
  const usage = d.usage || {};
  const totals = usage.totals || {};
  const proj = d.project || {};
  const comp = d.complexity || {};
  const host = d.host || {};
  const tools = d.tools || [];

  ANALYTICS_SECTIONS.forEach(([id, label]) => {
    const btn = $(`analytics-nav-${id}`);
    if (btn) btn.classList.toggle('active', analyticsState.section === id);
    if (btn && analyticsState.section === id) {
      const bc = $('analytics-breadcrumb');
      if (bc) bc.textContent = `Analytics · ${label}`;
    }
  });

  const railSummary = $('analytics-rail-summary');
  if (railSummary) {
    const availTools = tools.filter(t => t.available).length;
    railSummary.replaceChildren(
      el('div', { class: 'learning-task' },
        el('strong', {}, `${fmtCount(totals.today_tokens)} tokens today`),
        el('small', {}, `${totals.today_prompts || 0} prompts · ${fmtCount(proj.estimated_tokens)} in project`),
      ),
      el('div', { class: 'learning-task' },
        el('strong', {}, `${comp.functions_over_ccn_15 ?? 0} CCN > 15 hotspots`),
        el('small', {}, `${comp.files_analyzed ?? 0} files · ${fmtCount(comp.total_nloc)} NLOC · max CCN ${comp.max_ccn ?? 0}`),
      ),
      el('div', { class: 'learning-task' },
        el('strong', {}, `${comp.long_files ?? 0} long files`),
        el('small', {}, `≥ ${comp.long_file_threshold_lines ?? 600} lines · ${comp.ui_files_analyzed ?? 0} UI files · ${fmtCount(comp.total_source_lines)} lines`),
      ),
      el('div', { class: 'learning-task' },
        el('strong', {}, `${availTools}/${tools.length} Craft tools`),
        el('small', {}, `RSS ${(Number(host.self_rss_kb || 0) / 1024).toFixed(0)} MB · load ${host.load || '—'}`),
      ),
    );
  }

  const msg = $('analytics-message');
  if (msg) {
    msg.textContent = analyticsState.error || `Live telemetry from /proc, Omarchy agent state, and ${comp.tool || 'lizard'} static analysis.`;
  }

  surface.replaceChildren();
  const sec = analyticsState.section;

  if (sec === 'overview' || sec === 'tokens') {
    surface.append(
      el('h2', {}, 'Token Spend & Agent Activity'),
      el('div', { class: 'learning-grid' },
        el('div', { class: 'learning-summary' },
          el('h3', {}, 'Project Session (.agent)'),
          el('code', {}, `${fmtCount(proj.estimated_tokens)} est. tokens`),
          el('p', {}, `${proj.total_turns || 0} chat turns (${proj.user_turns || 0} user / ${proj.assistant_turns || 0} assistant) · ${proj.assets_count || 0} connected assets`),
        ),
        el('div', { class: 'learning-summary' },
          el('h3', {}, 'Today Across Agents'),
          el('code', {}, `${fmtCount(totals.today_tokens)} tokens`),
          el('p', {}, `${totals.today_prompts || 0} prompts today across ${usage.agents?.length || 0} active agents`),
        ),
        el('div', { class: 'learning-summary' },
          el('h3', {}, 'Cumulative Recorded Usage'),
          el('code', {}, `${fmtCount(totals.total_tokens)} total`),
          el('p', {}, `Input: ${fmtCount(totals.input_tokens)} · Output: ${fmtCount(totals.output_tokens)} · Cache Read: ${fmtCount(totals.cache_read_tokens)}`),
        ),
      ),
    );
    if (usage.agents?.length) {
      surface.append(
        el('div', { class: 'learning-grid learning-task-grid' },
          ...usage.agents.map(ag => {
            const topModels = (ag.models || []).slice(0, 3).map(m => `${m.model}: ${fmtCount(m.total_tokens)}`).join(' · ');
            return el('div', { class: 'learning-summary', id: `analytics-agent-${ag.id}` },
              el('h3', {}, `${ag.name || ag.id}${ag.tier ? ` (${ag.tier})` : ''}`),
              el('code', {}, `Today: ${fmtCount(ag.today_tokens)} (${ag.today_prompts} prompts) · Total: ${fmtCount(ag.total_tokens)}`),
              el('p', {}, `In ${fmtCount(ag.input_tokens)} · Out ${fmtCount(ag.output_tokens)} · Cache ${fmtCount(ag.cache_read_tokens)}`),
              topModels ? el('small', { class: 'muted' }, topModels) : null,
            );
          }),
        ),
      );
    }
  }

  if (sec === 'overview' || sec === 'complexity') {
    surface.append(
      el('h2', {}, 'Code Complexity (Lizard)'),
      el('div', { class: 'learning-grid' },
        el('div', { class: 'learning-summary' },
          el('h3', {}, 'Source Footprint'),
          el('code', {}, `${fmtCount(comp.total_nloc)} NLOC`),
          el('p', {}, `${comp.files_analyzed || 0} source files · ${comp.functions_analyzed || 0} functions (Rust + Python)`),
        ),
        el('div', { class: 'learning-summary' },
          el('h3', {}, 'Cyclomatic Hotspots'),
          el('code', {}, `${comp.functions_over_ccn_15 || 0} functions > CCN 15`),
          el('p', {}, `Highest CCN: ${comp.max_ccn || 0} · Threshold: 15`),
        ),
        el('div', { class: 'learning-summary' },
          el('h3', {}, 'Lexer Token Volume'),
          el('code', {}, `${fmtCount(comp.total_code_tokens)} code tokens`),
          el('p', {}, `Measured by ${comp.tool || 'lizard'} without compiling`),
        ),
        el('div', { class: 'learning-summary' },
          el('h3', {}, 'Long Files'),
          el('code', {}, `${comp.long_files || 0} files ≥ ${comp.long_file_threshold_lines || 600} lines`),
          el('p', {}, `${comp.ui_files_analyzed || 0} shell UI modules scanned · ${fmtCount(comp.total_source_lines)} source lines`),
        ),
      ),
    );
    if (comp.longest_files?.length) {
      surface.append(
        el('h3', {}, 'Longest files (reading cost)'),
        el('div', { class: 'learning-grid learning-task-grid' },
          ...comp.longest_files.map((file, idx) => el('div', {
            class: 'learning-summary',
            id: `analytics-long-file-${idx}`,
          },
            el('h3', {}, `${file.file} · ${file.lines} lines`),
            el('code', {}, `CCN ${file.ccn} · max ${file.max_ccn} · ${file.functions} functions`),
          )),
        ),
      );
    }
    if (comp.largest_ccn_files?.length) {
      surface.append(
        el('h3', {}, 'Highest complexity files'),
        el('div', { class: 'learning-grid learning-task-grid' },
          ...comp.largest_ccn_files.map((file, idx) => el('div', {
            class: 'learning-summary',
            id: `analytics-ccn-file-${idx}`,
          },
            el('h3', {}, `${file.file} · CCN ${file.ccn}`),
            el('code', {}, `${file.lines} lines · max CCN ${file.max_ccn} · ${file.functions} functions`),
          )),
        ),
      );
    }
    if (comp.hotspots?.length) {
      surface.append(
        el('div', { class: 'learning-grid learning-task-grid' },
          ...comp.hotspots.slice(0, sec === 'complexity' ? 24 : 9).map((fn, idx) => el('div', {
            class: 'learning-summary',
            id: `analytics-hotspot-${idx}`,
          },
            el('h3', {}, `${fn.name} (CCN ${fn.ccn})`),
            el('code', {}, `${fn.file}:${fn.start_line}-${fn.end_line}`),
            el('p', {}, `${fn.nloc} NLOC · ${fn.tokens} tokens · ${fn.params} params`),
          )),
        ),
      );
    }
  }

  if (sec === 'overview' || sec === 'health') {
    const usedGb = (Number(host.mem_used_kb || 0) / 1048576).toFixed(1);
    const totalGb = (Number(host.mem_total_kb || 0) / 1048576).toFixed(1);
    const selfMb = (Number(host.self_rss_kb || 0) / 1024).toFixed(1);
    surface.append(
      el('h2', {}, 'App Status & Craft Tool Health'),
      el('div', { class: 'learning-grid' },
        el('div', { class: 'learning-summary' },
          el('h3', {}, 'Engine Process'),
          el('code', {}, `PID ${host.pid || '?'} · ${selfMb} MB RSS`),
          el('p', {}, `Host load: ${host.load || '—'} · System RAM: ${usedGb} GB / ${totalGb} GB`),
        ),
        ...tools.map(t => el('div', { class: 'learning-summary', id: `analytics-tool-${t.tool}` },
          el('h3', {}, t.tool),
          el('code', {}, t.available ? 'Available' : 'Missing'),
          el('p', {}, t.resolved || ''),
        )),
      ),
    );
  }
}
