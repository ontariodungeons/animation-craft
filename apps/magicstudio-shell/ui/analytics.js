// Analytics reads saved measurements on entry. Scanning is an explicit action, never a timer.
const analyticsState = { data: null, section: 'overview', loading: false, error: null, diagram: 'magic-cli', commandFilter: '' };
const ANALYTICS_SECTIONS = [['overview','Overview'],['complexity','Code Complexity'],['tokens','Recorded Usage'],['health','App & Tool Health'],['commands','Command Reference']];
// Existing Archify exports, packaged byte-for-byte. They are references, not live system models.
const ANALYTICS_DIAGRAMS = [
  {id:'magic-cli',file:'magic-cli-architecture.html',title:'Magic CLI & Studio architecture',source:'CLI-Anything magic harness · unified live Studio & headless engine routing',revision:null},
  {id:'creator',file:'creator-view.html',title:'Creator’s loop',source:'Existing Magic Studio creator workflow · saved 27 Oct 2026',revision:null},
  {id:'openmontage',file:'openmontage-evidence.html',title:'OpenMontage pipeline',source:'calesthio/OpenMontage · evidence-linked reference',revision:'3cd5700b977bcd63644a3118cc1783df1cb002c8'},
];
function fmtCount(value) {
  if (value === null || value === undefined || !Number.isFinite(Number(value))) return '—';
  return Number(value).toLocaleString(undefined,{maximumFractionDigits:1});
}
function analyticsTime(seconds) {return seconds == null ? 'time unavailable' : new Date(seconds*1000).toLocaleString();}
function analyticsCard(title,value,description) {return el('div',{class:'learning-summary'},el('h3',{},title),el('strong',{},value),el('p',{},description));}
function analyticsGrid(...cards) {return el('div',{class:'learning-grid analytics-grid'},...cards);}
function analyticsCode(comp,key) {return comp.available === true ? fmtCount(comp[key]) : '—';}
function analyticsMemory(kb) {return kb == null ? '—' : `${fmtCount(Number(kb)/1024)} MB`;}
async function renderAnalytics(scan = false) {
  if (!$('view-analytics') || analyticsState.loading) return;
  analyticsMount();analyticsState.loading=true;analyticsState.error=null;
  $('analytics-refresh').disabled=true;$('analytics-scan').disabled=true;
  $('analytics-message').textContent=scan?'Running Lizard on local source…':'Reading saved scan and recorded telemetry…';
  try {analyticsState.data=await invoke('engine_command',{id:'app.analytics',params:{scan}});}
  catch(error){analyticsState.error=String(error);}
  finally {
    analyticsState.loading=false;$('analytics-refresh').disabled=false;$('analytics-scan').disabled=false;
    analyticsRenderSurface();
  }
}
function analyticsMount() {
  if ($('analytics-layout')) return;
  const nav=el('nav',{class:'learning-categories','aria-label':'Analytics sections'},...ANALYTICS_SECTIONS.map(([id,label])=>el('button',{
    id:`analytics-nav-${id}`,class:'ghost',onclick:()=>{analyticsState.section=id;analyticsRenderSurface();},
  },label)));
  const rail=el('aside',{class:'learning-rail','aria-label':'Analytics navigation'},el('h2',{},'Analytics'),nav,el('div',{id:'analytics-rail-summary',class:'learning-task-list'}));
  const actions=el('div',{class:'learning-actions'},
    el('button',{id:'analytics-refresh',class:'ghost',title:'Read saved scan and current telemetry without running Lizard','data-commands':'app.analytics',onclick:()=>renderAnalytics(false)},'Refresh data'),
    el('button',{id:'analytics-scan',class:'start',title:'Explicitly measure local Rust, Python and shell JavaScript with Lizard','data-commands':'app.analytics',onclick:()=>renderAnalytics(true)},'Refresh scan'));
  const main=el('main',{class:'learning-main',id:'analytics-main'},
    el('header',{class:'learning-bar'},el('h2',{id:'analytics-breadcrumb',class:'learning-breadcrumb'},'Analytics'),actions),
    el('p',{id:'analytics-message',class:'learning-message',role:'status'}),el('div',{id:'analytics-surface',class:'learning-surface'}));
  $('view-analytics').replaceChildren(el('div',{class:'learning-layout',id:'analytics-layout'},rail,main));
}
function analyticsDiagram(surface) {
  const selected=ANALYTICS_DIAGRAMS.find(item=>item.id===analyticsState.diagram)||ANALYTICS_DIAGRAMS[0];
  const picker=el('select',{id:'analytics-diagram-select','aria-label':'Architecture reference',onchange:event=>{analyticsState.diagram=event.target.value;analyticsRenderSurface();}},
    ...ANALYTICS_DIAGRAMS.map(item=>el('option',{value:item.id},item.title)));
  picker.value=selected.id;
  surface.append(el('section',{class:'analytics-architecture','aria-label':'Architecture reference'},
    el('div',{class:'learning-bar'},el('h2',{},'How the work fits together'),picker),
    el('p',{class:'muted'},selected.source+(selected.revision?` · revision ${selected.revision}`:'')),
    el('iframe',{id:'analytics-diagram-frame',class:'analytics-diagram',title:selected.title,
      src:`analytics-diagrams/${selected.file}?theme=dark&embed=1`,sandbox:'allow-scripts',
      onerror:()=>{$('analytics-diagram-status').textContent='Diagram could not load. The source reference is shown above.';}}),
    el('p',{id:'analytics-diagram-status',class:'muted'},'Saved Archify reference. Measurements below describe this Magic Studio source; they are not measurements of the diagram’s boxes.')));
}
function analyticsRenderSurface() {
  const surface=$('analytics-surface');if(!surface)return;
  const d=analyticsState.data||{},comp=d.complexity||{},usage=d.usage||{},totals=usage.available?usage.totals||{}:{},host=d.host||{},project=d.project||{};
  const section=analyticsState.section;
  for(const [id,label] of ANALYTICS_SECTIONS){const button=$(`analytics-nav-${id}`);button.classList.toggle('active',id===section);button.setAttribute('aria-pressed',String(id===section));if(id===section)$('analytics-breadcrumb').textContent=`Analytics · ${label}`;}
  $('analytics-message').textContent=analyticsState.error?`Could not refresh: ${analyticsState.error}. Previous results, if any, remain below.`:
    `Read ${analyticsTime(d.sampled_at_unix_secs)} · no background scans or polling`;
  $('analytics-rail-summary').replaceChildren(analyticsRailReport(comp));
  surface.replaceChildren();
  if(section==='overview')analyticsDiagram(surface);
  if(section==='overview'||section==='complexity')analyticsComplexity(surface,comp,section);
  if(section==='overview'||section==='tokens') {
    surface.append(el('h2',{},'Recorded usage · not billing'),analyticsGrid(
      analyticsCard('Today’s recorded tokens',fmtCount(totals.today_tokens),usage.available?`${fmtCount(totals.today_prompts)} prompts · Omarchy local records`:'Omarchy usage records unavailable.'),
      analyticsCard('Cumulative recorded tokens',fmtCount(totals.total_tokens),`Input ${fmtCount(totals.input_tokens)} · output ${fmtCount(totals.output_tokens)} · cache ${fmtCount(totals.cache_read_tokens)}`),
      analyticsCard('Project chat estimate',project.project?fmtCount(project.estimated_tokens):'—',project.project?`${fmtCount(project.total_turns)} turns · ${fmtCount(project.assets_count)} assets · characters ÷ 4; not provider usage`:'No project selected.')));
    if(section==='tokens')for(const agent of usage.agents||[])surface.append(analyticsCard(agent.name||agent.id,`${fmtCount(agent.today_tokens)} tokens today`,
      `${fmtCount(agent.total_prompts)} total prompts · ${fmtCount(agent.total_sessions)} sessions · ${(agent.models||[]).map(model=>`${model.model}: ${fmtCount(model.total_tokens)}`).join(' · ')}`));
  }
  if(section==='overview'||section==='health') {
    surface.append(el('h2',{},'Host and tool observations'),analyticsGrid(
      analyticsCard('This process',analyticsMemory(host.self_rss_kb),`PID ${host.pid??'—'} · source /proc`),
      analyticsCard('System RAM',analyticsMemory(host.mem_used_kb),`Total ${analyticsMemory(host.mem_total_kb)} · load ${host.load||'unavailable'}`)));
    if(section==='health')surface.append(analyticsGrid(...(d.tools||[]).map(tool=>analyticsCard(tool.tool,tool.available?'Executable found':'Not found',tool.resolved+' · path check only'))),
      el('p',{class:'muted'},'Executable presence is not a successful run. Codacy findings and CodeGraph measurements are not connected to this view.'));
  }
  if(section==='commands')analyticsCommands(surface,d.commands);
}
function analyticsScanLabel(comp) {
  if (!comp.available) return 'Not measured';
  return {measured:'Explicit scan',saved:'Saved scan',stale:'Saved scan · revision changed',worktree_unverified:'Saved scan · worktree changes not verified'}[comp.state]||'Saved measurement';
}
function analyticsRailReport(comp) {
  const metrics=[['Files','files_analyzed'],['Functions','functions_analyzed'],['CCN > 15','functions_over_ccn_15'],['Max CCN','max_ccn'],['Long files','long_files'],['Source lines','total_source_lines']];
  const failed=comp.scan_failures?.length||0;
  const total=comp.available?Number(comp.functions_analyzed):0;
  const hotspots=comp.available?Number(comp.functions_over_ccn_15):0;
  const ratio=total>0&&Number.isFinite(hotspots)?`${fmtCount(hotspots)} / ${fmtCount(total)} functions exceed CCN 15`:null;
  return el('section',{class:'analytics-rail-report','aria-label':'Last code scan report'},
    el('h3',{},'Last code scan'),
    el('p',{class:'analytics-scan-state',role:'status','data-state':failed?'partial':comp.state||'not_measured'},analyticsScanLabel(comp)+(failed?` · partial (${failed} failed target${failed===1?'':'s'})`:'')),
    el('small',{},comp.available?analyticsTime(comp.measured_at_unix_secs):(comp.reason||'Use Refresh scan to measure code.')),
    ratio?el('meter',{min:0,max:total,value:hotspots,'aria-label':ratio,title:ratio}):null,
    ratio?el('small',{},ratio):null,
    el('dl',{class:'analytics-numbers'},...metrics.flatMap(([label,key])=>[el('dt',{},label),el('dd',{},analyticsCode(comp,key))])),
    comp.available?el('small',{title:comp.source_revision||'Source revision unavailable'},`Lizard ${comp.tool_version||'version unknown'} · ${(comp.source_revision||'revision unavailable').slice(0,12)}`):null,
    comp.last_error||comp.cache_error?el('small',{role:'status'},comp.last_error||comp.cache_error):null,
    el('small',{},'Current CLI: magic (CLI-Anything harness)'));
}
function analyticsComplexity(surface,comp,section) {
  surface.append(el('h2',{},'Code measurements · Lizard'));
  if(!comp.available){surface.append(el('p',{class:'panel-empty'},comp.reason||'No saved scan. Use Refresh scan to measure local source.'));return;}
  const stateLabel=analyticsScanLabel(comp);
  surface.append(el('p',{class:'muted'},`${stateLabel} · ${analyticsTime(comp.measured_at_unix_secs)} · ${comp.tool||'lizard'}${comp.tool_version?' '+comp.tool_version:''}`),
    el('details',{class:'analytics-provenance'},el('summary',{},'Measurement source and coverage'),
      el('code',{},`${comp.source_root||'Source root unavailable'}\nrevision ${comp.source_revision||'unavailable'}\nWorktree at scan: ${comp.source_dirty===true?'changed':comp.source_dirty===false?'clean':'unknown'}\n${comp.tool_path||'Tool path unavailable'}\nTargets: ${(comp.targets||[]).join(', ')}`)),
    analyticsGrid(analyticsCard('Functions over CCN 15',analyticsCode(comp,'functions_over_ccn_15'),`Max CCN ${analyticsCode(comp,'max_ccn')} · ${analyticsCode(comp,'functions_analyzed')} functions`),
      analyticsCard('Source size',analyticsCode(comp,'total_source_lines'),`${analyticsCode(comp,'files_analyzed')} files · ${analyticsCode(comp,'total_nloc')} NLOC`),
      analyticsCard('Long files',analyticsCode(comp,'long_files'),`Threshold ${fmtCount(comp.long_file_threshold_lines)} lines · ${analyticsCode(comp,'ui_files_analyzed')} UI modules`)));
  if(comp.last_error||comp.cache_error)surface.append(el('p',{role:'status'},comp.last_error||`Measured, but could not save scan: ${comp.cache_error}`));
  if(comp.scan_failures?.length)surface.append(el('p',{role:'status'},`Partial coverage / failed targets: ${comp.scan_failures.join(', ')}`));
  if(section!=='complexity')return;
  for(const [key,title] of [['longest_files','Longest measured files'],['largest_ccn_files','Highest complexity files']]) {
    surface.append(el('h3',{},title),analyticsGrid(...(comp[key]||[]).map((file,index)=>el('div',{class:'learning-summary',id:`analytics-${key}-${index}`},
      el('h3',{},file.file),el('code',{},`${fmtCount(file.lines)} lines · CCN ${fmtCount(file.ccn)} · max ${fmtCount(file.max_ccn)} · ${fmtCount(file.functions)} functions`)))));
  }
  surface.append(el('h3',{},'Function hotspots'),analyticsGrid(...(comp.hotspots||[]).slice(0,24).map((fn,index)=>el('div',{class:'learning-summary',id:`analytics-hotspot-${index}`},
    el('h3',{},`${fn.name} · CCN ${fmtCount(fn.ccn)}`),el('code',{},`${fn.file}:${fn.start_line}–${fn.end_line}`),el('p',{},`${fmtCount(fn.nloc)} NLOC · ${fmtCount(fn.tokens)} code tokens · ${fmtCount(fn.params)} parameters`)))));
}
function analyticsCommands(surface,catalog) {
  surface.append(el('h2',{},'Current command reference'),el('p',{class:'muted'},'Read directly from the shared engine command.list metadata. Executed through the magic CLI and magic mcp over the bound live Studio session or headless engine.'));
  if(!catalog?.commands){surface.append(el('p',{class:'panel-empty'},'Command metadata unavailable. Refresh data to read it.'));return;}
  const input=el('input',{id:'analytics-command-search',type:'search',placeholder:'Find a command or feature','aria-label':'Find current command',value:analyticsState.commandFilter,
    oninput:event=>{analyticsState.commandFilter=event.target.value;analyticsCommandRows(catalog.commands);}});
  surface.append(input,el('div',{id:'analytics-command-list'}));analyticsCommandRows(catalog.commands);
}
function analyticsCommandRows(commands) {
  const root=$('analytics-command-list');if(!root)return;
  const filter=analyticsState.commandFilter.toLowerCase();const selected=commands.filter(command=>`${command.id} ${command.label} ${command.params}`.toLowerCase().includes(filter));
  const groups=new Map();for(const command of selected){const key=command.menu?.[0]||command.id.split('.')[0];if(!groups.has(key))groups.set(key,[]);groups.get(key).push(command);}
  root.replaceChildren(el('p',{class:'muted'},`${selected.length} of ${commands.length} commands · parameter descriptions are examples, not required/optional schemas`));
  for(const [name,items]of groups)root.append(el('details',{class:'analytics-command-group'},el('summary',{},`${name} · ${items.length}`),
    ...items.map(command=>el('article',{class:'learning-summary'},el('h3',{},command.label),el('code',{},`magic exec ${command.id} [key=value ...]`),
      el('p',{},`Menu: ${(command.menu||[]).join(' / ')||'no menu placement'} · Shortcut: ${command.shortcut||'none'}`),el('pre',{},command.params)))));
}
