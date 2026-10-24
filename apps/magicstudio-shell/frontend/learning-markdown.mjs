// Keep metadata opaque: editing the prose must not rewrite future frontmatter.
export function splitDocument(markdown) {
  const text = String(markdown ?? '');
  const match = text.match(/^(?:\uFEFF)?---\r?\n[\s\S]*?\r?\n---(?:\r?\n|$)/);
  return { prefix: match?.[0] || '', body: text.slice(match?.[0].length || 0) };
}
export function joinDocument(document, body) { return document.prefix + body; }
export function editableProblem(body) {
  // Crepe supports CommonMark + GFM. Other extensions remain intact, read-only.
  const prose = body.replace(/^(`{3,}|~{3,})[^\n]*\n[\s\S]*?^\1\s*$/gm, '').replace(/`[^`\n]*`/g, '');
  if (/\b(?:javascript|vbscript|data):/i.test(prose)) return 'Unsupported unsafe link scheme. Original content is preserved; editing is disabled.';
  if (/<\/?[a-z][\w-]*(?:\s[^<>]*?)?\s*\/?\s*>/i.test(prose) || /<!--|^\s*\[\^[^\]]+\]:|^:::/m.test(prose)) {
    return 'Unsupported Markdown extension or raw HTML. Original content is preserved; rich editing and save are disabled for this document.';
  }
  return null;
}

// Task sections are the portable schema; the CLI extracts the same named sections.
export function parseTask(markdown) {
  const {body}=splitDocument(markdown), sections={};
  let key='notes';
  for(const line of body.split(/\r?\n/)) {
    const h=line.match(/^##\s+(.+)$/);
    if(h){key=h[1].trim().toLowerCase();sections[key]='';}
    else sections[key]=(sections[key]||'')+line+'\n';
  }
  const clean=s=>(s||'').trim();
  const extract=s=>clean(s).replace(/^```[^\n]*\n([\s\S]*?)\n```$/,'$1');
  let action=null;
  for(const kind of ['command','skill','source']) if(clean(sections[kind])) {action={kind,ref:extract(sections[kind])};break;}
  const title=body.match(/^#\s+(.+)$/m)?.[1]?.trim() || '';
  const notes=Object.entries(sections).filter(([k])=>!['input','output','check','condition','fallback','command','skill','source'].includes(k)).map(([,v])=>v).join(' ').replace(/^#\s+.+$/m,'');
  const human=[title,sections.input,sections.output,sections.check,sections.condition,sections.fallback,notes].filter(Boolean).join(' ');
  return {title,input:clean(sections.input),action,output:clean(sections.output),check:clean(sections.check),condition:clean(sections.condition),fallback:clean(sections.fallback),notes:clean(notes),word_count:human.split(/\s+/).filter(Boolean).length,step_count:(body.match(/^\s*\d+\.\s/gm)||[]).length};
}
export function validateTask(block,limits={}) {
  const errors=[];
  const words=[block.title,block.input,block.output,block.check,block.condition,block.fallback,block.notes].filter(Boolean).join(' ').split(/\s+/).filter(Boolean).length;
  if(!block.title) errors.push('A task needs a title.');
  if(!block.input) errors.push('A task needs Input.');
  if(!block.action?.ref) errors.push('A task needs an exact Command, Skill or Source.');
  if(!block.output) errors.push('A task needs Output.');
  if(!block.check) errors.push('A task needs an observable Check.');
  if(block.action?.ref.length>2048) errors.push('Action reference exceeds 2048 characters.');
  if(limits.max_words && words>limits.max_words) errors.push(`Task exceeds ${limits.max_words} word limit; shorten explicitly. Nothing was truncated.`);
  if(limits.max_steps && block.step_count>limits.max_steps) errors.push(`Task exceeds ${limits.max_steps} step limit.`);
  return errors;
}
