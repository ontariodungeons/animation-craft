// Real browser -> current UI handlers -> real mc -> temporary project files.
import { chromium } from '../frontend/node_modules/playwright-core/index.mjs';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import http from 'node:http';
import assert from 'node:assert/strict';
const temp=fs.mkdtempSync(path.join(os.tmpdir(),'learning-ui-')),project=path.join(temp,'Song.magic');
const bin=process.env.MC_BIN || path.resolve('target/debug/mc');
const env={...process.env,XDG_CONFIG_HOME:path.join(temp,'config')};
function exec(id,params={}) {
 const p=spawnSync(bin,['run','-'],{cwd:temp,env,input:[{id:'project.open',params:{path:project}},{id,params}].map(JSON.stringify).join('\n')+'\n',encoding:'utf8'});
 const result=JSON.parse(p.stdout.trim().split('\n').at(-1));if(p.status)throw Error(result.error||p.stderr);return result.result;
}
const init=spawnSync(bin,['exec','project.new',`path=${project}`],{cwd:temp,env,encoding:'utf8'});assert.equal(init.status,0,init.stderr);
exec('skills.limits',{max_words:100,max_steps:8}); // Explicit fixture policy, not a production default.
const root=path.resolve('apps/magicstudio-shell/ui'),calls=[];
const fixture=`<!doctype html><meta charset="utf-8"><link rel="stylesheet" href="/style.css"><link rel="stylesheet" href="/vendor/learning/learning-editor.css"><style>body{display:block}#view-skills{height:100vh}</style><section id="view-skills" class="view active"></section><script>
window.state={project:${JSON.stringify(project)},view:'skills'};window.$=id=>document.getElementById(id);window.toast=()=>{};window.openPath=()=>{};
window.el=(tag,attrs={},...children)=>{let n=document.createElement(tag);for(const[k,v]of Object.entries(attrs)){if(k.startsWith('on'))n.addEventListener(k.slice(2),v);else if(v!==null&&v!==undefined)n.setAttribute(k,v);}for(const c of children.flat(Infinity)){if(c!==null&&c!==undefined)n.append(c instanceof Node?c:document.createTextNode(String(c)));}return n;};
window.invoke=async(_name,p)=>{const r=await fetch('/api',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(p)});const d=await r.json();if(d.error)throw Error(d.error);return d;};
</script><script src="/vendor/learning/learning-editor.js"></script><script src="/skills.js"></script><script>setupSkillsAdd();renderSkills();</script>`;
const server=http.createServer(async(req,res)=>{
 if(req.url==='/'){res.setHeader('Content-Type','text/html');return res.end(fixture);}
 if(req.url==='/api'){let raw='';for await(const b of req)raw+=b;try{const p=JSON.parse(raw);calls.push(p.id);res.setHeader('Content-Type','application/json');return res.end(JSON.stringify(exec(p.id,p.params)));}catch(e){res.statusCode=409;return res.end(JSON.stringify({error:String(e)}));}}
 const filename=path.resolve(root,'.'+decodeURIComponent(req.url.split('?')[0]));if(!filename.startsWith(root+path.sep)){res.statusCode=403;return res.end();}
 try{res.setHeader('Content-Type',filename.endsWith('.js')?'text/javascript':filename.endsWith('.css')?'text/css':'application/octet-stream');res.end(fs.readFileSync(filename));}catch{res.statusCode=404;res.end();}
}).listen(0,'127.0.0.1');await new Promise(r=>server.once('listening',r));
const browser=await chromium.launch({executablePath:'/usr/lib/chromium/chromium',headless:true,args:['--no-sandbox']});
try {
 const page=await browser.newPage({viewport:{width:1261,height:900}});const errors=[];page.on('pageerror',e=>errors.push(String(e)));
 await page.goto(`http://127.0.0.1:${server.address().port}`);await page.locator('#learning-home').waitFor({timeout:3000});
 await page.locator('[data-task="import-timing"]').first().click();await page.locator('.ProseMirror h1').waitFor();
 await page.locator('.ProseMirror h1').click();await page.keyboard.press('End');await page.keyboard.type(' edited');
 await page.locator('#learning-save').click();await page.waitForFunction(()=>document.querySelector('#learning-save-state')?.textContent.includes('Saved'));
 let info=exec('skills.info',{id:'import-timing'}).skill;assert.match(info.name,/edited$/);assert.match(info.markdown,/workshop.import_transcript/);
 await page.locator('.ProseMirror h1').click();await page.keyboard.press('End');await page.keyboard.type(' local draft');
 const external=info.markdown.replace('edited','external');exec('skills.edit',{id:info.id,expected_revision:info.revision,markdown:external});
 await page.waitForSelector('#learning-conflict:not([hidden])',{timeout:8000});assert.match(await page.locator('.ProseMirror h1').innerText(),/local draft/);
 assert.equal(exec('skills.info',{id:info.id}).skill.markdown,external);
 await page.locator('#learning-use-latest').click();await page.waitForFunction(()=>document.querySelector('.ProseMirror h1')?.textContent.endsWith('external'));
 assert.ok(await page.locator('#learning-recover-draft').isVisible());
 await page.locator('#learning-remove').click();await page.locator('#learning-restore').waitFor();assert.equal(exec('skills.info',{id:'import-timing'}).skill.removed,true);await page.locator('#learning-restore').click();
 assert.equal(exec('skills.info',{id:'import-timing'}).skill.removed,false);
 await page.setViewportSize({width:700,height:900});await page.screenshot({path:path.resolve('target/learning-ui-narrow.png')});
 assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false,'no viewport-wide overflow');
 assert.deepEqual(errors,[]);assert.ok(calls.every(x=>x.startsWith('skills.')),'UI never executes production instructions');
 console.log('PASS Learning UI: real inline editing/save, CLI conflict preserves draft, recoverable draft, remove/restore, 700px layout');
}finally{await browser.close();server.close();fs.rmSync(temp,{recursive:true,force:true});}
