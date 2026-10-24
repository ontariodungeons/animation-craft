import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import assert from 'node:assert/strict';
const temp=fs.mkdtempSync(path.join(os.tmpdir(),'magic-learning-'));
const bin=process.env.MC_BIN || 'mc';
const env={...process.env,XDG_CONFIG_HOME:path.join(temp,'config')};
fs.mkdirSync(path.join(env.XDG_CONFIG_HOME,'magicstudio'),{recursive:true});
const legacy={legacy:{id:'legacy',name:'Old reference',description:'Keep me',source:'https://example.com',future:{nested:[1,2]}}};
const legacyPath=path.join(env.XDG_CONFIG_HOME,'magicstudio','skills.json');fs.writeFileSync(legacyPath,JSON.stringify(legacy));
const project=path.join(temp,'Song.magic');
function run(commands,ok=true) {
 const p=spawnSync(bin,['run','-'],{cwd:temp,env,input:[{id:'project.open',params:{path:project}},...commands].map(JSON.stringify).join('\n')+'\n',encoding:'utf8'});
 if(ok) assert.equal(p.status,0,p.stderr+'\n'+p.stdout);
 return p;
}
function value(p){const lines=p.stdout.trim().split('\n').filter(Boolean);const v=JSON.parse(lines.at(-1));return v.result ?? v;}
try {
 const init=spawnSync(bin,['exec','project.new',`path=${project}`],{cwd:temp,env,encoding:'utf8'});assert.equal(init.status,0,init.stderr);
 let info=value(run([{id:'skills.info',params:{id:'legacy'}}])).skill;
 assert.deepEqual(info.future,{nested:[1,2]});assert.ok(info.revision);
 const md='---\nfuture: {preserve: yes}\n---\n# Revised title\n\n1. Exact step\n\n```sh\nmc "$PROJECT" storyboard.init\n```\n';
 let changed=value(run([{id:'skills.edit',params:{id:'legacy',markdown:md,expected_revision:info.revision}}])).skill;
 assert.equal(changed.markdown,md);assert.equal(changed.name,'Revised title');assert.deepEqual(changed.future,legacy.legacy.future);
 const stale=run([{id:'skills.edit',params:{id:'legacy',markdown:'# Lost edit',expected_revision:info.revision}}],false);assert.notEqual(stale.status,0);assert.match(stale.stderr+' '+stale.stdout,/conflict/i);
 info=value(run([{id:'skills.info',params:{id:'legacy'}}])).skill;assert.equal(info.markdown,md);
 const removed=value(run([{id:'skills.remove',params:{id:'legacy',expected_revision:info.revision}}])).skill;
 assert.equal(removed.removed,true);assert.equal(value(run([{id:'skills.list',params:{}}])).skills.some(x=>x.id==='legacy'),false);
 const restored=value(run([{id:'skills.undo',params:{id:'legacy',expected_revision:removed.revision}}])).skill;assert.equal(restored.markdown,md);assert.equal(restored.removed,false);
 assert.equal(fs.readFileSync(legacyPath,'utf8'),JSON.stringify(legacy));
 const source=path.join(temp,'SKILL.md');fs.writeFileSync(source,`---\nname: source-only\n---\n# Useful reference\n\nRun: touch ${path.join(temp,'must-not-execute')}\n`);
 const imported=value(run([{id:'skills.import',params:{id:'source-only',source}}])).skill;assert.equal(imported.status,'researching');assert.equal(imported.verification,'draft');assert.equal(imported.block.action.kind,'source');assert.equal(imported.block.action.ref,source);assert.ok(!imported.markdown.includes('touch /tmp/'));assert.ok(fs.readFileSync(source,'utf8').includes('touch /tmp/'));assert.equal(imported.source,source);assert.equal(fs.existsSync(path.join(temp,'must-not-execute')),false);
 assert.notEqual(run([{id:'skills.import',params:{id:'bad',source:'http://127.0.0.1/run'}}],false).status,0);
 assert.notEqual(run([{id:'skills.add',params:{id:'../escape',name:'Bad'}}],false).status,0);
 assert.ok(fs.existsSync(path.join(temp,'learning','legacy.md')));
 execStructured();
 function execStructured(){
  value(run([{id:'skills.limits',params:{max_words:100,max_steps:8}}]));
  let task=value(run([{id:'skills.info',params:{id:'import-timing'}}])).skill;
  const md=task.markdown.replace('Import existing timed lyrics','Import supplied timings');
  task=value(run([{id:'skills.edit',params:{id:task.id,expected_revision:task.revision,markdown:md}}])).skill;
  const tooLong=md+'\n## Notes\n'+Array(101).fill('word').join(' ');
  assert.notEqual(run([{id:'skills.edit',params:{id:task.id,expected_revision:task.revision,markdown:tooLong}}],false).status,0);
  const removed=value(run([{id:'skills.remove',params:{id:task.id,expected_revision:task.revision}}])).skill;assert.equal(removed.removed,true);
  const restored=value(run([{id:'skills.undo',params:{id:task.id,expected_revision:removed.revision}}])).skill;assert.equal(restored.removed,false);
  assert.equal(restored.block.action.kind,'command');assert.match(restored.block.action.ref,/workshop.import_transcript/);
  const diagram=value(run([{id:'skills.diagram',params:{kind:'production'}}]));assert.equal(diagram.spec.nodes.find(x=>x.id===task.id).label,'Import supplied timings');
  const ordered=diagram.spec.mainPath.map(id=>diagram.spec.nodes.find(n=>n.id===id));
  assert.ok(ordered.every((n,i)=>!i||n.col>=ordered[i-1].col),'Archify main path must never move backward');
 }

 console.log('PASS Learning CLI: legacy preservation, Markdown roundtrip, stale writes, remove/restore, draft import, rejected unsafe source/id');
} finally {fs.rmSync(temp,{recursive:true,force:true});}
