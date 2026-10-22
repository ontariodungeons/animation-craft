#!/usr/bin/env python3
"""Existing storyboard-kit template population via DeckCraft native commands; no alternate document editor."""
import argparse, collections, contextlib, hashlib, json, math, os, re, select, shutil, subprocess, time
from pathlib import Path

VERSION = '3.2'
SIZE = [1440,1080]
SCENE_ART_REGION = [70,430,556,200]
INK='#17363D'; TEAL='#176E69'; PAPER='#F4F1E9'; MUTED='#52676C'
STATES={'missing':('#F8E5DE','#983F29'),'draft':('#E7EDF2','#435D71'),'needs review':('#FFF0CE','#8B5C0D'),'approved':('#DDEFE4','#296246')}
CREDIT="Inspired by and built on the Backlot concept and Jesse Naiman's 'I Went Down into the Barnyard' storyboard and production workflow."
STOP=set('the a an and or but on in at to of with from for by into then their our its'.split())
def dump(p,v): Path(p).write_text(json.dumps(v,indent=2,ensure_ascii=False)+'\n')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def clock(t):
    n=round(float(t)*1000);return f'{n//60000:02}:{n//1000%60:02}.{n%1000:03}'
def norm(t):return re.sub(r'[^\w]+','',t.lower())
def fit_rect(width,height,region):
    if not all(math.isfinite(v) and v>0 for v in [width,height,region[2],region[3]]):raise ValueError('Picture and region dimensions must be positive')
    x,y,w,h=region;scale=min(w/width,h/height);fw,fh=width*scale,height*scale
    return [x+(w-fw)/2,y+(h-fh)/2,fw,fh]
def validate_words(words,duration):
    ids=[];previous=-1
    for w in words:
        a,b=w['start'],w['end']
        if not all(math.isfinite(x) for x in [a,b]) or not 0<=a<=b<=duration+.001 or a<previous or not str(w['text']).strip():raise ValueError('Invalid or unordered source word timing')
        ids.append(w['id']);previous=a
    if not words or len(set(ids))!=len(ids):raise ValueError('Word occurrence IDs must be nonempty and unique')
def validate_groups(groups,words,duration):
    ids=[i for g in groups for i in g['word_ids']]
    if ids!=[w['id'] for w in words]:raise ValueError('Grouping must preserve every word occurrence exactly once and in source order')
    if len({g['id'] for g in groups})!=len(groups):raise ValueError('Group targets must be unique')
    lookup={w['id']:w for w in words};previous=0
    for g in groups:
        if abs(g['span_start']-previous)>.001 or g['span_end']<=g['span_start']:raise ValueError('Scene spans must be positive and contiguous')
        if any(lookup[i]['start']<g['span_start']-.001 or lookup[i]['end']>g['span_end']+.001 for i in g['word_ids']):raise ValueError('Scene span excludes its source word timing')
        previous=g['span_end']
    if abs(previous-duration)>.001:raise ValueError('Scene spans do not cover the full recording')
def propose_groups(words,duration):
    validate_words(words,duration);chunks=[];start=0
    for i,w in enumerate(words):
        count=i-start+1;gap=words[i+1]['start']-w['end'] if i+1<len(words) else 0
        terminal=norm(w['text']) not in STOP
        boundary=(str(w['text']).endswith(('.', '?','!')) and count>=3) or (gap>=.16 and count>=5 and terminal) or (count>=16 and terminal)
        if boundary or i==len(words)-1:chunks.append(words[start:i+1]);start=i+1
    if len(chunks)>1 and len(chunks[-1])<3:chunks[-2]+=chunks.pop()
    groups=[];cursor=0
    for i,ch in enumerate(chunks):
        if ch[0]['start']-cursor>=1:
            groups.append(dict(id='gap-'+ch[0]['id'],word_ids=[],text='',span_start=cursor,span_end=ch[0]['start'],kind='uncaptioned audio',status='needs review',reason='No source words in this interval; listen before calling it instrumental.'));cursor=ch[0]['start']
        nxt=chunks[i+1][0]['start'] if i+1<len(chunks) else duration
        end=ch[-1]['end'] if nxt-ch[-1]['end']>=1 else nxt
        groups.append(dict(id='phrase-'+ch[0]['id'],word_ids=[w['id'] for w in ch],text=' '.join(w['text'] for w in ch),span_start=cursor,span_end=end,lyric_start=ch[0]['start'],lyric_end=ch[-1]['end'],kind='provisional phrase',status='needs review',reason='Proposed from word pauses/punctuation with a line fallback; no verse/chorus identity asserted.'));cursor=end
    if duration-cursor>.001:groups.append(dict(id='gap-tail',word_ids=[],text='',span_start=cursor,span_end=duration,kind='uncaptioned audio',status='needs review',reason='Recording continues after the last transcribed word.'))
    counts=collections.Counter(' '.join(norm(t) for t in g['text'].split()) for g in groups if g['word_ids'])
    for g in groups:
        key=' '.join(norm(t) for t in g['text'].split());g['repeat_occurrences']=counts.get(key,0)
    validate_groups(groups,words,duration);return groups
def read_inputs(config,grouping=None):
    c=json.loads(Path(config).read_text());words=json.loads(Path(c['transcript']).read_text());words=words if isinstance(words,list) else words['words']
    words=[dict(w,id=w.get('id',f'w{i}')) for i,w in enumerate(words)]
    duration=float(c['duration_sec']) if c.get('duration_sec') is not None else float(json.loads(Path(c['audiomap']).read_text())['audio']['duration_sec']);validate_words(words,duration)
    cues=[]
    for block in re.split(r'\n\s*\n',Path(c['captions']).read_text().strip()):
        lines=block.splitlines();hit=next((i for i,l in enumerate(lines) if '-->' in l),None)
        if hit is None:raise ValueError('Malformed caption block')
        times=re.findall(r'(\d+):(\d+):(\d+)[,.](\d+)',lines[hit])
        if len(times)!=2:raise ValueError('Malformed cue timestamp')
        a,b=[int(h)*3600+int(m)*60+int(s)+int(ms)/1000 for h,m,s,ms in times]
        if a<0 or b<a or b>duration+.001 or (cues and a<cues[-1]['end']):raise ValueError('Caption timings overlap or exceed recording')
        cues.append(dict(id=f'cue-{len(cues)+1:03}',start=a,end=b,text=' '.join(lines[hit+1:]),status='needs review'))
    if [norm(t) for x in cues for t in x['text'].split()]!=[norm(t) for w in words for t in w['text'].split()]:raise ValueError('Caption text does not preserve the word transcript occurrence sequence')
    groups=json.loads(Path(grouping).read_text())['groups'] if grouping else propose_groups(words,duration)
    lookup={w['id']:w for w in words}
    for g in groups:
        g['text']=' '.join(lookup[i]['text'] for i in g['word_ids'])
        if g['word_ids']:g['lyric_start']=lookup[g['word_ids'][0]]['start'];g['lyric_end']=lookup[g['word_ids'][-1]]['end']
    validate_groups(groups,words,duration)
    image_map=c.get('scene_images',{})
    if not isinstance(image_map,dict) or set(image_map)-{g['id'] for g in groups}:raise ValueError('scene_images keys must identify actual grouping IDs')
    if any(not isinstance(r,dict) or not r.get('file') for r in image_map.values()):raise ValueError('Each scene_images record requires an explicit file path')
    return c,words,cues,groups,duration

class Deck:
    def __init__(self,template_root,output_root):
        self.p=subprocess.Popen(['deckcraft-cli','mcp','--automation-read-root',str(template_root),'--automation-write-root',str(output_root)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);self.serial=0;self.queue=[]
        self.rpc('initialize',dict(protocolVersion='2024-11-05',capabilities={},clientInfo=dict(name='project-local-storyboard',version=VERSION)))
    def rpc(self,method,params):
        self.serial+=1;req=dict(jsonrpc='2.0',id=self.serial,method=method,params=params);self.p.stdin.write(json.dumps(req)+'\n');self.p.stdin.flush()
        if not select.select([self.p.stdout],[],[],60)[0]:raise RuntimeError('DeckCraft MCP timed out; no fallback renderer used')
        line=self.p.stdout.readline()
        if not line:raise RuntimeError('DeckCraft stopped: '+self.p.stderr.read())
        r=json.loads(line)
        if 'error' in r or r.get('result',{}).get('isError'):raise RuntimeError(r)
        blocks=r.get('result',{}).get('content',[])
        if blocks and blocks[0]['type']=='text':
            try:return json.loads(blocks[0]['text'])
            except ValueError:return blocks[0]['text']
        return r.get('result',{})
    def cmd(self,c,**p):return self.rpc('tools/call',dict(name='run_command',arguments=dict(command=c,params=p)))
    def q(self,c,**p):self.queue.append(dict(command=c,params=p))
    def flush(self):
        if self.queue:self.rpc('tools/call',dict(name='batch',arguments=dict(commands=self.queue)));self.queue=[]
    def close(self):self.p.terminate();self.p.wait()
    def box(self,rect,color,role=None):
        self.q('shape.insert',preset='rect',rect=rect);self.q('shape.fill',color=color);self.q('shape.line',none=True)
        if role:self.q('shape.altText',text=role)
    def text(self,t,rect,size=22,color=INK,bold=False,role=None):
        self.q('insert.textBox',rect=rect,text=t);self.q('format.font',family='DejaVu Sans');self.q('format.size',size=size);self.q('format.color',color=color);self.q('format.bold',on=bold);self.q('format.margins',left=0,right=0,top=0,bottom=0)
        if role:self.q('shape.altText',text=role);self.q('shape.rename',name=role)
    def card(self,title,instruction,rect,role,state='missing',size=22):
        x,y,w,h=rect;bg,fg=STATES[state];self.box(rect,'#FFFFFF','layout.card.'+role);self.box([x,y,6,h],fg)
        self.text(title.upper(),[x+22,y+18,w-44,28],18,TEAL,True)
        self.text(state.upper(),[x+22,y+h-28,w-44,22],15,fg,True,'state.'+role)
        self.text(instruction,[x+22,y+54,w-44,h-88],size,MUTED,role='user.'+role)
    def page(self,kind,title,sub):
        self.q('slide.new',layout='blank');self.q('slide.rename',name='template.'+kind);self.q('design.background',color=PAPER);self.box([0,0,1440,10],TEAL)
        self.text('ANIMATION STUDIO / '+kind.upper(),[48,28,1100,26],18,TEAL,True)
        self.text(title,[48,75,1344,65],42,INK,True,'auto.title');self.text(sub,[48,147,1344,40],20,MUTED,role='auto.subtitle')
        self.text('Contents',[1250,28,142,28],18,TEAL,True,'nav.contents')
        self.box([48,1030,1344,1],'#C9D1C9');self.text(CREDIT,[48,1044,1344,24],12,MUTED)
        self.q('slide.notes',text=json.dumps(dict(sb_template=VERSION,kind=kind)))

def inspect_pages(d):
    info=d.cmd('document.inspect');return [d.cmd('slide.inspect',index=i) for i in range(len(info['slides']))]
def note(slide):
    try:return json.loads(slide['notes'])
    except (ValueError,KeyError):return {}
def snapshot(d):return [dict(id=s['id'],notes=s['notes'],shapes=s['shapes']) for s in inspect_pages(d)]
def source_fingerprint(c,groups):
    return hashlib.sha256(json.dumps(dict(config=c,inputs={k:sha(c[k]) for k in ['transcript','captions','audiomap'] if c.get(k)},groups=groups),sort_keys=True).encode()).hexdigest()
def build_plan(c,cues,groups,representative=False):
    if representative:return [dict(kind='scene',key='scene:'+groups[0]['id'],title='Scene 01 / '+groups[0]['kind'],group=groups[0])]
    pages=[dict(kind='overview',key='overview',title='Song overview & writeup')]
    pages += [dict(kind='lyrics',key=f'lyrics:{i//4+1}',title=f'Lyrics verification {i//4+1}',cues=cues[i:i+4]) for i in range(0,len(cues),4)]
    pages += [dict(kind='assets',key='asset:'+x['id'],title=x.get('name','Character / asset inventory'),asset=x) for x in c.get('assets',[dict(id='inventory',name='Character / asset inventory')])]
    pages += [dict(kind='locations',key='location:'+x['id'],title=x.get('name','Location / clean plate inventory'),location=x) for x in c.get('locations',[dict(id='inventory',name='Location / clean plate inventory')])]
    pages += [dict(kind='scene',key='scene:'+x['id'],title=f'Scene {i+1:02} / '+x['kind'],group=x) for i,x in enumerate(groups)]
    pages += [dict(kind='qc',key='qc',title='Delivery, QC & ownership')]
    ntoc=math.ceil((len(pages)+1)/12)
    while ntoc!=math.ceil((len(pages)+ntoc)/12):ntoc=math.ceil((len(pages)+ntoc)/12)
    return [dict(kind='toc',key=f'toc:{i+1}',title=f'Contents {i+1}/{ntoc}') for i in range(ntoc)]+pages
def fill_text(d,slide,role,value,size=None):
    sh=next((x for x in slide['shapes'] if x.get('altText')==role),None)
    if sh:
        d.q('text.set',id=sh['id'],text=value)
        if size:d.q('edit.select',ids=[sh['id']]);d.q('format.size',size=size)
def place_scene_picture(d,slide,record,group_id):
    path=Path(record['file']).resolve()
    if not path.is_file():raise ValueError('Configured scene reference does not exist: '+str(path))
    d.flush()
    # Native insertion without a rect supplies the decoded image's natural aspect.
    picture=d.cmd('insert.picture',path=str(path),name=record.get('name','Reference only'))
    pid=picture['id'];native=d.cmd('shape.inspect',id=pid)['box']
    fitted=fit_rect(native['w'],native['h'],SCENE_ART_REGION)
    d.cmd('shape.sizePosition',ids=[pid],x=fitted[0],y=fitted[1],w=fitted[2],h=fitted[3],lockAspect=False)
    d.cmd('shape.sizePosition',ids=[pid],lockAspect=True)
    d.cmd('edit.select',ids=[pid]);d.cmd('shape.altText',text='user.picture.scene.'+group_id)
    d.cmd('shape.rename',name='Reference only · '+record.get('name',path.name))
    d.cmd('insert.hyperlink',url=path.as_uri(),tooltip='Reference source; not an approved production asset')
    fill_text(d,slide,'user.scene.reference','')
    fill_text(d,slide,'state.scene.reference','REFERENCE ONLY · NEEDS REVIEW')
    state=next(x for x in slide['shapes'] if x.get('altText')=='state.scene.reference')
    d.q('edit.select',ids=[state['id']]);d.q('format.color',color=STATES['needs review'][1]);d.flush()
    return dict(file=str(path),status='reference only / needs review',region=SCENE_ART_REGION,picture_id=pid,rect=fitted)
def fill_page(d,slide,page,c,words,cues,groups,duration,all_pages,representative):
    kind=page['kind'];fill=lambda r,t,sz=None:fill_text(d,slide,r,t,sz)
    fill('auto.title',c['title']+' / '+page['title'],36)
    fill('auto.subtitle',f'{c.get("artist","Artist unassigned")} / {clock(duration)} / NEEDS REVIEW / '+page['key'],20)
    nav=next(x for x in slide['shapes'] if x.get('altText')=='nav.contents');d.q('shape.action',ids=[nav['id']],action='none' if representative else 'slide',**({} if representative else {'slide':0}))
    if kind=='toc':
        at=int(page['key'].split(':')[1])-1
        for slot in range(12):
            index=at*12+slot;target=all_pages[index] if index<len(all_pages) else None
            if target is None:
                ids=[x['id'] for x in slide['shapes'] if x.get('altText') in ['layout.toc.row.'+str(slot),'auto.toc.label.'+str(slot),'auto.toc.meta.'+str(slot)]]
                d.q('edit.delete',ids=ids);continue
            fill('auto.toc.label.'+str(slot),target['title'] if target else '',22)
            fill('auto.toc.meta.'+str(slot),f'Page {index+1:02} · NEEDS REVIEW' if target else '',18)
            sh=next(x for x in slide['shapes'] if x.get('altText')=='auto.toc.label.'+str(slot))
            d.q('shape.action',ids=[sh['id']],action='slide' if target else 'none',**({'slide':index} if target else {}))
    if kind=='overview':
        fill('user.source',f'{c.get("album","Album unassigned")} / track {c.get("track","?")}\nMeasured duration: {clock(duration)}\n{len(words)} words / {len(cues)} original cues\n{len(groups)} proposed scene spans\nTranscript: ASR, not verified by listening.',21)
        links=[('Audio',c.get('audio')),('Words',c['transcript']),('Cues',c['captions']),('Project',c.get('project'))]
        for i,(label,path) in enumerate(links):
            if path and Path(path).exists():d.text(label+' ↗',[930+i*110,413,108,26],16,TEAL,True,'source.'+str(i));d.q('insert.hyperlink',url=Path(path).resolve().as_uri(),tooltip=label)
    if kind=='lyrics':
        offset=int(page['key'].split(':')[1])*4-4
        for slot in range(4):
            if slot>=len(page['cues']):
                ids=[x['id'] for x in slide['shapes'] if x.get('altText') in [r+str(slot) for r in ['layout.cue.row.','auto.cue.time.','auto.cue.raw.','auto.cue.state.','user.correction.','auto.correction.label.']]]
                d.q('edit.delete',ids=ids)
                continue
            cue=page['cues'][slot];ids=[w['id'] for w in words if w['start']<cue['end'] and w['end']>cue['start']]
            mapped=[g['id'] for g in groups if any(w in g['word_ids'] for w in ids)]
            fill('auto.cue.time.'+str(slot),f'{cue["id"]} · {clock(cue["start"])}–{clock(cue["end"])} · '+(', '.join(ids[:1]+ids[-1:]) if ids else 'timing uncertain'),18)
            fill('auto.cue.raw.'+str(slot),cue['text'],24)
            fill('auto.cue.state.'+str(slot),'NEEDS REVIEW · '+', '.join(mapped),16)
    if kind=='scene':
        g=page['group'];ids=g['word_ids'];raw=g['text'] or 'No transcribed words in this interval — listen before labelling it instrumental.'
        fill('auto.scene.lyrics',raw,27)
        existing=c.get('scene_content',{}).get(g['id'],{})
        sections=existing.get('sections') or {}
        for source_role,target_role in [('reference_prompt','user.scene.image_prompt'),('animation_prompt','user.scene.animation_prompt'),('layer_stack','user.scene.continuity')]:
            if sections.get(source_role):fill(target_role,sections[source_role],21)
        t=f'Scene span {clock(g["span_start"])}–{clock(g["span_end"])}'
        if ids:t+=f' / Words {clock(g["lyric_start"])}–{clock(g["lyric_end"])} / {ids[0]}…{ids[-1]}'
        fill('auto.subtitle',t+' / PROVISIONAL · NEEDS REVIEW',19)
        for label,targetkey,x in [('Asset pages','asset:',1078),('Clean plates','location:',1220)]:
            target=next((i for i,p in enumerate(all_pages) if p['key'].startswith(targetkey)),None)
            if target is not None:d.text(label,[x,616,140,24],16,TEAL,True,'nav.'+targetkey);d.q('shape.action',action='slide',slide=target,tooltip=label)
    if kind=='qc':fill('user.qc.settings',f'Measured recording: {clock(duration)}\nCanvas / fps / codec: NOT SET\nExport destination: NOT SET\nEnter proposed settings, then record approval.',22)
    # Configured pictures become real independently replaceable DeckCraft picture objects.
    record=page.get('asset') or page.get('location') or {}
    if record.get('file'):
        path=Path(record['file']).resolve()
        if not path.is_file():raise ValueError('Configured asset does not exist: '+str(path))
        rect=[70,320,550,220] if kind=='assets' else [70,320,750,220]
        d.q('insert.picture',path=str(path),rect=rect,name=record.get('name',path.stem));d.q('shape.altText',text='user.picture.'+record['id'])
    reference=None
    if kind=='scene' and page['group']['id'] in c.get('scene_images',{}):reference=place_scene_picture(d,slide,c['scene_images'][page['group']['id']],page['group']['id'])
    meta=dict(sb_generator=VERSION,page_key=page['key'],kind=kind,status='needs review',group=page.get('group'),source_transcript=c['transcript'],source_captions=c['captions'])
    if kind=='scene':meta['source_packet']=c.get('scene_content',{}).get(page['group']['id'],{})
    if reference:meta['reference_image']=reference
    d.q('slide.notes',text=json.dumps(meta,indent=2));d.q('slide.rename',name=page['key']);d.flush()

def validate_deck(d,pages,words,cues,groups,duration,representative=False):
    slides=inspect_pages(d);assert len(slides)==len(pages),'Page count mismatch'
    keys=[note(s).get('page_key') for s in slides];assert keys==[p['key'] for p in pages],'Page mapping mismatch'
    overflow=[];links=0;file_links=0;actual_cues=[];creative_instructions=0
    for i,(sl,page) in enumerate(zip(slides,pages)):
        d.cmd('slide.go',index=i)
        cards={x['altText'][12:]:x['box'] for x in sl['shapes'] if x.get('altText','').startswith('layout.card.')}
        for sh in sl['shapes']:
            x,y,w,h=sh['box'];role=sh.get('altText','')
            if x<-.1 or y<-.1 or x+w>SIZE[0]+1 or y+h>SIZE[1]+1:overflow.append(dict(page=i+1,role=role,box=sh['box']))
            if role.startswith('user.'):creative_instructions+=1
            if role.startswith('user.') and role[5:] in cards:
                card=cards[role[5:]]
                if y+h>card[1]+card[3]-30:overflow.append(dict(page=i+1,role=role,reason='Text collides with its card status; shorten or enlarge the editable card',box=sh['box']))
            if role=='auto.scene.lyrics' and y+h>342:overflow.append(dict(page=i+1,role=role,reason='Lyric block exceeds its allocated area',box=sh['box']))
            if role.startswith('auto.cue.raw.'):
                limit=220+int(role.split('.')[-1])*194+128
                if y+h>limit:overflow.append(dict(page=i+1,role=role,reason='Caption text exceeds its verification row; reflow this template before saving',box=sh['box']))
            target=None
            if role=='nav.contents' and not representative:target=0
            if role.startswith('auto.toc.label.'):
                at=(int(page['key'].split(':')[1])-1)*12+int(role.split('.')[-1]);target=at if at<len(pages) else None
            if role in ['nav.asset:','nav.location:']:
                prefix=role[4:];target=next(j for j,p in enumerate(pages) if p['key'].startswith(prefix))
            if target is not None:
                full=d.cmd('shape.inspect',id=sh['id']);assert full['shape']['click']['action']==dict(kind='slide',slide=slides[target]['id']),'Native TOC target mismatch';links+=1
            if role.startswith('source.'):
                from urllib.parse import unquote,urlparse
                action=d.cmd('shape.inspect',id=sh['id'])['shape']['click']['action'];uri=urlparse(action['url'])
                assert action['kind']=='url' and uri.scheme=='file' and Path(unquote(uri.path)).is_file(),'Source file link does not resolve';file_links+=1
        if page['kind']=='lyrics':
            for j,cue in enumerate(page['cues']):actual_cues.append(next(x['text'] for x in sl['shapes'] if x.get('altText')=='auto.cue.raw.'+str(j)))
    if not representative:assert actual_cues==[x['text'] for x in cues],'Original caption occurrence lost';assert {'toc','overview','lyrics','assets','locations','scene','qc'}.issubset({x['kind'] for x in pages})
    validate_groups(groups,words,duration)
    return dict(ok=not overflow,slides=len(slides),native_links=links,resolving_source_links=file_links,caption_occurrences=len(actual_cues),source_words=len(words),scene_spans=len(groups),duration=duration,source_sequence_preserved=True,provisional_grouping=True,overflow=overflow,editable_fields=creative_instructions,approval='needs review',pages=[dict(key=p['key'],id=s['id'],kind=p['kind']) for p,s in zip(pages,slides)])

def generate(template,config,output,grouping=None,representative=False):
    template=Path(template).resolve();output=Path(output).resolve();output.parent.mkdir(parents=True,exist_ok=True)
    state=output.with_suffix('.state.json');lock=output.with_suffix('.lock');fd=os.open(lock,os.O_CREAT|os.O_EXCL|os.O_WRONLY);os.close(fd)
    d=None
    try:
        c,words,cues,groups,duration=read_inputs(config,grouping);fingerprint=source_fingerprint(c,groups);pages=build_plan(c,cues,groups,representative);previous_hash=sha(output) if output.exists() else None
        if output.exists() and not state.exists():raise ValueError('Existing deck has no generator state; refusing to overwrite authored work')
        if state.exists() and json.loads(state.read_text())['fingerprint']!=fingerprint:raise ValueError('Source/grouping changed. Generate to a new output revision; existing authored work is preserved.')
        d=Deck(template.parent,output.parent)
        if output.exists():
            d.cmd('file.open',path=str(output));before=snapshot(d)
        else:
            d.cmd('file.open',path=str(template));originals=inspect_pages(d);sources={note(s)['kind']:s['id'] for s in originals}
            for page in pages:
                d.cmd('slide.go',id=sources[page['kind']]);d.cmd('slide.duplicate');info=d.cmd('document.inspect')
                # Duplicate is current; move it to the end so original template indices stay fixed.
                active=d.cmd('slide.inspect');d.cmd('slide.move',**{'from':active['index'],'to':len(info['slides'])-1})
            for i in reversed(range(len(originals))):d.cmd('slide.delete',index=i)
            for i,page in enumerate(pages):d.cmd('slide.go',index=i);fill_page(d,d.cmd('slide.inspect',index=i),page,c,words,cues,groups,duration,pages,representative)
            before=None
        validation=validate_deck(d,pages,words,cues,groups,duration,representative)
        if not validation['ok']:raise ValueError('Layout overflow: '+json.dumps(validation['overflow']))
        if before is not None:assert snapshot(d)==before,'Regeneration altered authored objects'
        if output.exists() and sha(output)!=previous_hash:raise ValueError('Concurrent disk edit detected; refusing to replace current deck')
        if output.exists():
            history=output.parent/'history';history.mkdir(exist_ok=True);backup=history/(output.stem+'-'+previous_hash[:12]+output.suffix)
            if not backup.exists():shutil.copy2(output,backup)
        pending=output.with_name(output.stem+'.pending.deckcraft');d.cmd('file.saveAs',path=str(pending));os.replace(pending,output)
        dump(state,dict(version=VERSION,fingerprint=fingerprint,template=str(template),template_sha256=sha(template),source_config=str(Path(config).resolve()),representative=representative))
        dump(output.with_suffix('.grouping.json'),dict(status='provisional / editable',groups=groups))
        dump(output.with_suffix('.validation.json'),validation)
        d.cmd('file.export',path=str(output.with_suffix('.pdf')),format='pdf')
        scene=next(i for i,p in enumerate(pages) if p['kind']=='scene');d.cmd('file.export',path=str(output.with_name(output.stem+'-scene.png')),format='png',slide=scene,scale=1)
        if not representative:
            for kind in ['toc','overview','lyrics','assets','locations','qc']:
                i=next(i for i,p in enumerate(pages) if p['kind']==kind);d.cmd('file.export',path=str(output.with_name(output.stem+'-'+kind+'.png')),format='png',slide=i,scale=1)
        validation['authored_edits_preserved']=before is not None;dump(output.with_suffix('.validation.json'),validation)
        return dict(ok=True,deck=str(output),pdf=str(output.with_suffix('.pdf')),proof=str(output.with_name(output.stem+'-scene.png')),reopened_existing=before is not None,validation=validation)
    finally:
        if d:d.close()
        lock.unlink(missing_ok=True)

