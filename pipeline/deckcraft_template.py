#!/usr/bin/env python3
"""Existing storyboard-kit template population via DeckCraft native commands; no alternate document editor."""
import argparse, collections, contextlib, hashlib, json, math, os, re, select, shutil, subprocess, sys, time
from pathlib import Path

VERSION = '4.1'
SIZE = [1080,1600]
TOC_PAGE_SIZE = 20
SCENE_ART_REGION = [60,220,960,540]
INK='#F0EBDC'; TEAL='#D7B46A'; PAPER='#111216'; MUTED='#B7B4AB'
STATES={'missing':('#F8E5DE','#983F29'),'draft':('#E7EDF2','#435D71'),'needs review':('#FFF0CE','#8B5C0D'),'approved':('#DDEFE4','#296246')}
CREDIT="Production storyboard / original source material remains linked and unchanged"
STOP=set('the a an and or but on in at to of with from for by into then their our its'.split())
def dump(p,v): Path(p).write_text(json.dumps(v,indent=2,ensure_ascii=False)+'\n')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def clock(t):
    n=round(float(t)*1000);return f'{n//60000:02}:{n//1000%60:02}.{n%1000:03}'
def norm(t):return re.sub(r'[^\w]+','',t.lower())
def panel_region(index):return [60+(index%2)*496,220+(index//2)*200,464,160]
def fit_rect(width,height,region):
    if not all(math.isfinite(v) and v>0 for v in [width,height,region[2],region[3]]):raise ValueError('Picture and region dimensions must be positive')
    x,y,w,h=region;scale=min(w/width,h/height);fw,fh=width*scale,height*scale
    return [x+(w-fw)/2,y+(h-fh)/2,fw,fh]
def validate_words(words,duration,allow_overrun=False):
    ids=[];previous=-1
    for w in words:
        a,b=w['start'],w['end']
        if not all(math.isfinite(x) for x in [a,b]) or not (0<=a<=b and (b<=duration+.001 or (allow_overrun and a<duration))) or a<previous or not str(w['text']).strip():raise ValueError('Invalid or unordered source word timing')
        ids.append(w['id']);previous=a
    if not words or len(set(ids))!=len(ids):raise ValueError('Word occurrence IDs must be nonempty and unique')
def validate_groups(groups,words,duration):
    ids=[i for g in groups for i in g['word_ids']]
    if ids!=[w['id'] for w in words]:raise ValueError('Grouping must preserve every word occurrence exactly once and in source order')
    if len({g['id'] for g in groups})!=len(groups):raise ValueError('Group targets must be unique')
    lookup={w['id']:w for w in words};previous=0
    for g in groups:
        if abs(g['span_start']-previous)>.001 or g['span_end']<=g['span_start']:raise ValueError('Scene spans must be positive and contiguous')
        if any(lookup[i]['start']<g['span_start']-.001 or (lookup[i]['end']>g['span_end']+.001 and g.get('word_assignment')!='source-start timestamp; canonical scene cuts preserved') for i in g['word_ids']):raise ValueError('Scene span excludes its source word timing')
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
    c=json.loads(Path(config).read_text())
    if c.get('empty_template'):
        return c,[],[],[],float(c.get('duration_sec') or 0)
    words=json.loads(Path(c['transcript']).read_text());words=words if isinstance(words,list) else words['words']
    words=[dict(w,id=w.get('id',f'w{i}')) for i,w in enumerate(words)]
    duration=float(c['duration_sec']) if c.get('duration_sec') is not None else float(json.loads(Path(c['audiomap']).read_text())['audio']['duration_sec']);validate_words(words,duration,c.get('allow_timing_overrun',False))
    cues=[]
    for block in re.split(r'\n\s*\n',Path(c.get('captions')).read_text().strip()) if c.get('captions') else []:
        lines=block.splitlines();hit=next((i for i,l in enumerate(lines) if '-->' in l),None)
        if hit is None:raise ValueError('Malformed caption block')
        times=re.findall(r'(\d+):(\d+):(\d+)[,.](\d+)',lines[hit])
        if len(times)!=2:raise ValueError('Malformed cue timestamp')
        a,b=[int(h)*3600+int(m)*60+int(s)+int(ms)/1000 for h,m,s,ms in times]
        if a<0 or b<a or (b>duration+.001 and not c.get('allow_timing_overrun')) or (cues and a<cues[-1]['end']):raise ValueError('Caption timings overlap or exceed recording')
        cues.append(dict(id=f'cue-{len(cues)+1:03}',start=a,end=b,text=' '.join(lines[hit+1:]),status='needs review'))
    if not c.get('captions'):cues=[dict(w,status='native word timing; no supplied phrase captions') for w in words]
    if [norm(t) for x in cues for t in x['text'].split()]!=[norm(t) for w in words for t in w['text'].split()]:raise ValueError('Caption text does not preserve the word transcript occurrence sequence')
    groups=json.loads(Path(grouping).read_text())['groups'] if grouping else propose_groups(words,duration)
    lookup={w['id']:w for w in words}
    for g in groups:
        g['text']='\n'.join(l['text'] for l in g['lyric_lines']) if g.get('lyric_lines') else ' '.join(lookup[i]['text'] for i in g['word_ids'])
        if g['word_ids']:g['lyric_start']=lookup[g['word_ids'][0]]['start'];g['lyric_end']=lookup[g['word_ids'][-1]]['end']
    validate_groups(groups,words,duration)
    image_map=c.get('scene_images',{})
    if not isinstance(image_map,dict) or set(image_map)-{g['id'] for g in groups}:raise ValueError('scene_images keys must identify actual grouping IDs')
    if any(not isinstance(r,dict) or not r.get('file') for r in image_map.values()):raise ValueError('Each scene_images record requires an explicit file path')
    return c,words,cues,groups,duration

class Deck:
    def __init__(self,template_root,output_root):
        self.p=subprocess.Popen([shutil.which('deckcraft-cli') or 'deckcraft-cli','mcp','--automation-read-root',str(template_root),'--automation-write-root',str(output_root)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);self.serial=0;self.queue=[]
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
    def card(self,title,instruction,rect,role,state='missing',size=21):
        x,y,w,h=rect;self.box(rect,'#1B1D22','layout.card.'+role)
        self.text(title.upper(),[x+18,y+16,w-36,28],18,TEAL,True)
        self.text(instruction,[x+18,y+56,w-36,h-76],size,MUTED,role='user.'+role)
    def page(self,kind,title,sub,new=True):
        if new:self.q('slide.new',layout='blank')
        self.q('slide.rename',name='template.'+kind);self.q('design.background',color=PAPER)
        for x in range(0,1080,90):self.box([x,0,45,18],'#E8E5DC')
        self.text('PRODUCTION / '+kind.upper(),[60,50,740,24],17,TEAL,True)
        self.text(title,[60,96,960,64],38,INK,True,'auto.title');self.text(sub,[60,165,960,30],18,MUTED,role='auto.subtitle')
        self.text('Contents',[880,50,140,28],17,TEAL,True,'nav.contents')
        self.box([60,1550,960,1],'#494238');self.text(CREDIT,[60,1563,960,22],12,MUTED)
        self.q('slide.notes',text=json.dumps(dict(sb_template=VERSION,kind=kind)))

    def sheet(self):
        for slot in range(4):
            x=60+(slot%2)*496;y=220+(slot//2)*610
            self.box([x,y,464,570],'#1B1D22',f'layout.sheet.{slot}.card')
            self.text('Linked asset',[x+18,y+16,428,28],20,TEAL,True,f'auto.sheet.{slot}.title')
            self.box([x+18,y+62,428,300],'#08090C',f'layout.sheet.{slot}.picture')
            self.text('',[x+18,y+390,428,155],18,MUTED,False,f'user.sheet.{slot}.notes')
            self.flush();field=next(sh for sh in self.cmd('slide.inspect')['shapes'] if sh.get('altText')==f'user.sheet.{slot}.notes')
            self.q('edit.select',ids=[field['id']]);self.q('format.autofit',mode='shrink');self.q('shape.setBounds',ids=[field['id']],x=x+18,y=y+390,w=428,h=155)

def revise_template(template):
    """Revise existing native template slides through DeckCraft; no alternate renderer."""
    template=Path(template).resolve();d=Deck(template.parent,template.parent)
    try:
        d.cmd('file.open',path=str(template));d.cmd('design.slideSize',w=SIZE[0],h=SIZE[1],scale='none')
        for i,slide in reversed(list(enumerate(inspect_pages(d)))):
            if note(slide).get('kind') in ['lyrics','qc']:d.cmd('slide.delete',index=i)
        for i,slide in enumerate(inspect_pages(d)):
            kind=note(slide)['kind'];d.cmd('slide.go',index=i);d.cmd('edit.delete',ids=[sh['id'] for sh in slide['shapes']]);d.page(kind,kind.title(),'Imported source timing',new=False)
            if kind=='toc':
                for slot in range(TOC_PAGE_SIZE):
                    y=220+slot*64;d.box([60,y,960,54],'#1B1D22','layout.toc.row.'+str(slot));d.text('Document page',[78,y+14,780,30],22,INK,False,'auto.toc.label.'+str(slot));d.text('00',[930,y+15,68,28],18,TEAL,False,'auto.toc.meta.'+str(slot))
            elif kind=='overview':
                d.card('Song story','Describe the story, characters and progression of this song.',[60,220,960,280],'story')
                d.card('Background A / main location','Name this location and place its background reference.',[60,540,464,330],'background.main')
                d.card('Background B / alternate location','Name this location and place its background reference.',[556,540,464,330],'background.alternate')
                d.card('Characters and props','Name the cast and reusable props for this song.',[60,910,960,220],'cast')
                d.card('Source material','',[60,1170,960,260],'source')
            elif kind in ['assets','locations']:
                d.sheet()
            elif kind=='scene':
                d.text('SCENE IMAGE / 16:9',[60,195,960,23],17,TEAL,True)
                d.box(SCENE_ART_REGION,'#08090C','layout.scene.picturewell');d.text('Place the scene reference here',[90,455,900,50],28,MUTED,False,'user.scene.reference');d.text('',[80,726,920,24],16,TEAL,False,'state.scene.reference')
                d.box([60,790,960,150],'#F3ECD7','layout.scene.lyrics');d.text('LYRICS',[80,803,920,22],16,'#6D5631',True);d.text('',[80,838,920,90],27,'#211E18',False,'auto.scene.lyrics')
                for title,role,x,y,w,h,instruction in [
                    ('Action / staging','action',60,970,464,190,'Describe the visible action, framing and movement.'),
                    ('Continuity / linked assets','continuity',556,970,464,190,'Link cast, props and locations. Record continuity.'),
                    ('Image prompt','image_prompt',60,1180,464,160,'Describe the reference image to create.'),
                    ('Animation prompt','animation_prompt',556,1180,464,160,'Describe movement, timing and camera action.'),
                    ('Animated text','animated_text',60,1360,464,170,'Text, word IDs, reveal timing and placement.'),
                    ('Drawing cues','drawing',556,1360,464,170,'What is drawn, when it appears and where.')]:d.card(title,instruction,[x,y,w,h],'scene.'+role)
            d.flush()
        d.cmd('file.saveAs',path=str(template))
    finally:d.close()

def inspect_pages(d):
    info=d.cmd('document.inspect');return [d.cmd('slide.inspect',index=i) for i in range(len(info['slides']))]
def note(slide):
    try:return json.loads(slide['notes'])
    except (ValueError,KeyError):return {}
def snapshot(d):return [dict(id=s['id'],notes=s['notes'],shapes=s['shapes']) for s in inspect_pages(d)]
def source_fingerprint(c,groups):
    return hashlib.sha256(json.dumps(dict(config=c,inputs={k:sha(c[k]) for k in ['transcript','captions','audiomap'] if c.get(k)},groups=groups),sort_keys=True).encode()).hexdigest()
def toc_entries(pages):return [(i,p) for i,p in enumerate(pages) if p['kind']!='toc']
def sheet_pages(kind,records,label):
    categories={}
    for record in records or [dict(id='inventory',name='No linked artwork yet')]:categories.setdefault(record.get('category',label),[]).append(record)
    pages=[]
    for category_index,(category,items) in enumerate(categories.items()):
        for offset in range(0,len(items),4):pages.append(dict(kind=kind,key=('asset:' if kind=='assets' else 'location:')+f'{category_index+1}:{offset//4+1}',title=f'{category} / sheet {offset//4+1}',sheet=items[offset:offset+4]))
    return pages

def build_plan(c,cues,groups,representative=False):
    if representative:return [dict(kind='scene',key='scene:'+groups[0]['id'],title='Scene 01',group=groups[0])]
    pages=[dict(kind='overview',key='overview',title='Song story & backgrounds')]
    pages += sheet_pages('assets',c.get('assets',[]),'Characters & props')
    pages += sheet_pages('locations',c.get('locations',[]),'Backgrounds & locations')
    pages += [dict(kind='scene',key='scene:'+x['id'],title=(x.get('section_label')+' / ' if x.get('section_label') else '')+f'Scene {i+1:02}',group=x) for i,x in enumerate(groups)]
    count=math.ceil(len(pages)/TOC_PAGE_SIZE)
    contents=[dict(kind='toc',key=f'toc:{i+1}',title='Contents' if count==1 else f'Contents {i+1}/{count}') for i in range(count)]
    return pages[:1]+contents+pages[1:]

def fill_text(d,slide,role,value,size=None,fit=False):
    sh=next((x for x in slide['shapes'] if x.get('altText')==role),None)
    if sh:
        d.q('text.set',id=sh['id'],text=value)
        if size:d.q('edit.select',ids=[sh['id']]);d.q('format.size',size=size)
        if fit:
            card=next((x for x in slide['shapes'] if x.get('altText')=='layout.card.'+role[5:]),None)
            if card:
                x,y,w,h=card['box'];height=h-76
                if role=='user.scene.continuity':height=min(height,1130-(y+56)-12)
                d.q('edit.select',ids=[sh['id']]);d.q('format.autofit',mode='shrink')
                d.q('shape.setBounds',ids=[sh['id']],x=x+18,y=y+56,w=w-36,h=height)
def place_scene_picture(d,slide,record,group_id,region=None):
    path=Path(record['file']).resolve()
    if not path.is_file():raise ValueError('Configured scene reference does not exist: '+str(path))
    d.flush()
    # Native insertion without a rect supplies the decoded image's natural aspect.
    picture=d.cmd('insert.picture',path=str(path),name=record.get('name','Reference only'))
    pid=picture['id'];native=d.cmd('shape.inspect',id=pid)['box']
    fitted=fit_rect(native['w'],native['h'],region or SCENE_ART_REGION)
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
    compact=140 if kind=='scene' and page['group'].get('panels') else 0
    if compact:
        for shape in slide['shapes']:
            x,y,w,h=shape['box']
            if y>=790:d.q('shape.setBounds',ids=[shape['id']],x=x,y=y-compact,w=w,h=h)
            if shape.get('altText')=='layout.scene.picturewell':d.q('shape.setBounds',ids=[shape['id']],x=x,y=y,w=w,h=400)
        d.flush();slide=d.cmd('slide.inspect',index=slide['index'])

    duration_label='Measured recording' if c.get('recording_duration_verified',False) else 'Supplied timing extent; recording duration unknown'
    fill('auto.title',c['title']+' / '+page['title'],36)
    fill('auto.subtitle',f'{clock(duration)} / '+page['title'],18)
    nav=next(x for x in slide['shapes'] if x.get('altText')=='nav.contents');d.q('shape.action',ids=[nav['id']],action='none' if representative or kind=='toc' else 'slide',**({} if representative or kind=='toc' else {'slide':next(i for i,p in enumerate(all_pages) if p['kind']=='toc')}))
    if kind=='toc':
        at=int(page['key'].split(':')[1])-1
        for slot in range(TOC_PAGE_SIZE):
            entry=at*TOC_PAGE_SIZE+slot;entries=toc_entries(all_pages);index,target=entries[entry] if entry<len(entries) else (None,None)
            if target is None:
                ids=[x['id'] for x in slide['shapes'] if x.get('altText') in ['layout.toc.row.'+str(slot),'auto.toc.label.'+str(slot),'auto.toc.meta.'+str(slot)]]
                d.q('edit.delete',ids=ids);continue
            fill('auto.toc.label.'+str(slot),target['title'] if target else '',22)
            fill('auto.toc.meta.'+str(slot),f'{index+1:02}' if target else '',18)
            sh=next(x for x in slide['shapes'] if x.get('altText')=='auto.toc.label.'+str(slot))
            d.q('shape.action',ids=[sh['id']],action='slide' if target else 'none',**({'slide':index} if target else {}))
    if kind=='overview':
        if c.get('story'):fill('user.story',c['story'],24)
        warning='\nTIMING WARNING: '+c['timing_warning'] if c.get('timing_warning') else ''
        source_note='Audio linked; lyrics, scene structure and artwork await your choices.' if c.get('empty_template') else 'Existing transcript words/timing consumed unchanged.'
        fill('user.source',f'{duration_label}: {clock(duration)}\n{len(words)} timed words / {len(groups)} scenes\n'+source_note+warning,22)
        links=[('Audio',c.get('audio')),('Transcript',c['transcript']),('Captions',c.get('captions')),('Project',c.get('project'))]
        for i,(label,path) in enumerate(links):
            if path and Path(path).exists():d.text(label+' ↗',[60+i*240,1470,220,30],18,TEAL,True,'source.'+str(i));d.q('insert.hyperlink',url=Path(path).resolve().as_uri(),tooltip=label)
    if kind=='scene':
        g=page['group'];ids=g['word_ids'];raw=g['text'] or ''
        fill('auto.scene.lyrics',raw,27)
        lyric_shape=next(x for x in slide['shapes'] if x.get('altText')=='auto.scene.lyrics')
        d.q('edit.select',ids=[lyric_shape['id']]);d.q('format.autofit',mode='shrink');d.q('shape.setBounds',ids=[lyric_shape['id']],x=160 if g.get('lyric_lines') else 80,y=838-compact,w=840 if g.get('lyric_lines') else 920,h=90)
        for i,line in enumerate(g.get('lyric_lines',[])):
            d.text(line['id'],[80,838-compact+i*32.4,76,24],13,TEAL,False,'auto.scene.lyric_id.'+line['id'])
        existing=c.get('scene_content',{}).get(g['id'],{})
        sections=existing.get('sections') or {}
        for source_role,target_role in [('reference_prompt','user.scene.image_prompt'),('animation_prompt','user.scene.animation_prompt'),('layer_stack','user.scene.continuity'),('action','user.scene.action'),('animated_text','user.scene.animated_text'),('drawing_cues','user.scene.drawing')]:
            if sections.get(source_role):fill_text(d,slide,target_role,sections[source_role],21,fit=c.get('fit_imported_text',False))
        t=f'Scene span {clock(g["span_start"])}–{clock(g["span_end"])}'
        if ids:t+=f' / Words {clock(g["lyric_start"])}–{clock(g["lyric_end"])} / {ids[0]}…{ids[-1]}'
        fill('auto.subtitle',t,18)
        for label,targetkey,x in [('Assets','asset:',574),('Locations','location:',780)]:
            target=next((i for i,p in enumerate(all_pages) if p['key'].startswith(targetkey)),None)
            if target is not None:d.text(label,[x,1130,140,24],16,TEAL,True,'nav.'+targetkey);d.q('shape.action',action='slide',slide=target,tooltip=label)
    if kind in ['assets','locations']:
        for slot in range(4):
            if slot>=len(page['sheet']):
                ids=[sh['id'] for sh in slide['shapes'] if sh.get('altText','').startswith((f'layout.sheet.{slot}.',f'auto.sheet.{slot}.',f'user.sheet.{slot}.'))]
                if ids:d.q('edit.delete',ids=ids)
                continue
            record=page['sheet'][slot];fill(f'auto.sheet.{slot}.title',record.get('name',record['id']),20)
            fill(f'user.sheet.{slot}.notes',record['notes'] if record.get('notes_exact') else record.get('notes',record.get('id',''))+'\n'+(Path(record['file']).name if record.get('file') else 'Register existing artwork to populate this slot.'),18)
            if record.get('file'):
                path=Path(record['file']).resolve()
                if not path.is_file():raise ValueError('Configured asset does not exist: '+str(path))
                region=next(sh['box'] for sh in slide['shapes'] if sh.get('altText')==f'layout.sheet.{slot}.picture')
                d.flush();picture=d.cmd('insert.picture',path=str(path),name=record.get('name',path.stem));native=d.cmd('shape.inspect',id=picture['id'])['box'];rect=fit_rect(native['w'],native['h'],region)
                d.cmd('shape.sizePosition',ids=[picture['id']],x=rect[0],y=rect[1],w=rect[2],h=rect[3],lockAspect=False)
                d.cmd('edit.select',ids=[picture['id']]);d.cmd('shape.altText',text='user.picture.'+record['id']);d.cmd('insert.hyperlink',url=path.as_uri(),tooltip='Existing asset; original pixels and alpha unchanged')
    reference=None
    if kind=='scene' and page['group']['id'] in c.get('scene_images',{}):
        record=c['scene_images'][page['group']['id']]
        if page['group'].get('panels'):
            panels=[]
            background=next((r for r in c.get('locations',[]) if 'backdrop' in r['id'].lower()),None)
            if not background:raise ValueError('Four chorus panels need an existing registered backdrop; no background is generated')
            for i,line in enumerate(page['group']['panels']):
                image=record if i in [0,3] else background
                placed=place_scene_picture(d,slide,image,page['group']['id']+f'.panel.{i+1}',panel_region(i))
                rect=panel_region(i);label=('START' if i==0 else 'END' if i==3 else str(i+1))
                d.text(f'{label}  {line.get("line_id") or ""}  {clock(line["start"])}–{clock(line["end"])}  '+line['text'],[rect[0],rect[1]+160,464,40],14,INK,False,'user.scene.panel.'+str(i+1))
                panels.append(dict(line,reference=placed,staging='Original cast reference' if i in [0,3] else 'Existing background; animated subjects pending'))
            reference=dict(panels=panels,original=record['file'])
            fill('state.scene.reference','')
        else:reference=place_scene_picture(d,slide,record,page['group']['id'])
    for field in c.get('page_content',{}).get(page['key'],[]):
        fill_text(d,slide,field['role'],field['value'],field.get('font_size'),fit=True)
    meta=dict(sb_generator=VERSION,page_key=page['key'],kind=kind,status='needs review',group=page.get('group'),source_transcript=c['transcript'],source_captions=c.get('captions'),sheet=page.get('sheet'))
    if kind=='scene':meta['source_packet']=c.get('scene_content',{}).get(page['group']['id'],{});meta['asset_links']=c.get('asset_links',[]);meta['official_lyrics_source']=c.get('official_lyrics_source')
    if reference:meta['reference_image']=reference
    d.q('slide.notes',text=json.dumps(meta,indent=2));d.q('slide.rename',name=page['key']);d.flush()

def validate_deck(d,pages,words,cues,groups,duration,representative=False,empty_template=False):
    slides=inspect_pages(d);assert len(slides)==len(pages),'Page count mismatch'
    keys=[note(s).get('page_key') for s in slides];assert keys==[p['key'] for p in pages],'Page mapping mismatch'
    overflow=[];links=0;file_links=0;actual_cues=[];creative_instructions=0;link_regions=[]
    for i,(sl,page) in enumerate(zip(slides,pages)):
        d.cmd('slide.go',index=i)
        cards={x['altText'][12:]:x['box'] for x in sl['shapes'] if x.get('altText','').startswith('layout.card.')}
        for sh in sl['shapes']:
            x,y,w,h=sh['box'];role=sh.get('altText','')
            if x<-.1 or y<-.1 or x+w>SIZE[0]+1 or y+h>SIZE[1]+1:overflow.append(dict(page=i+1,role=role,box=sh['box']))
            if role.startswith('user.'):creative_instructions+=1
            if role.startswith('user.') and role[5:] in cards:
                card=cards[role[5:]]
                if y+h>card[1]+card[3]-12:overflow.append(dict(page=i+1,role=role,reason='Text collides with its card status; shorten or enlarge the editable card',box=sh['box']))
            if role=='auto.scene.lyrics' and y+h>940:overflow.append(dict(page=i+1,role=role,reason='Lyric block exceeds its allocated area',box=sh['box']))
            if role.startswith('auto.cue.raw.'):
                limit=220+int(role.split('.')[-1])*194+128
                if y+h>limit:overflow.append(dict(page=i+1,role=role,reason='Caption text exceeds its verification row; reflow this template before saving',box=sh['box']))
            target=None
            if role=='nav.contents' and not representative and page['kind']!='toc':target=next(j for j,p in enumerate(pages) if p['kind']=='toc')
            if role.startswith('auto.toc.label.'):
                at=(int(page['key'].split(':')[1])-1)*TOC_PAGE_SIZE+int(role.split('.')[-1]);entries=toc_entries(pages);target=entries[at][0] if at<len(entries) else None
            if role in ['nav.asset:','nav.location:']:
                prefix=role[4:];target=next(j for j,p in enumerate(pages) if p['key'].startswith(prefix))
            if target is not None:
                full=d.cmd('shape.inspect',id=sh['id']);assert full['shape']['click']['action']==dict(kind='slide',slide=slides[target]['id']),'Native TOC target mismatch';links+=1;link_regions.append(dict(page=i+1,to_page=target+1,rect=[x,y,x+w,y+h]))
            if role.startswith('source.'):
                from urllib.parse import unquote,urlparse
                action=d.cmd('shape.inspect',id=sh['id'])['shape']['click']['action'];uri=urlparse(action['url'])
                assert action['kind']=='url' and uri.scheme=='file' and Path(unquote(uri.path)).is_file(),'Source file link does not resolve';file_links+=1
        if page['kind']=='scene':actual_cues.extend(page['group']['word_ids'])
    if not representative:
        assert actual_cues==[w['id'] for w in words],'Original word occurrence lost from scenes'
        required={'toc','overview','assets','locations'} | (set() if empty_template else {'scene'})
        assert required.issubset({x['kind'] for x in pages})
        assert not {'lyrics','qc'}.intersection(x['kind'] for x in pages)
    if not empty_template:validate_groups(groups,words,duration)
    return dict(ok=not overflow,slides=len(slides),native_links=links,link_regions=link_regions,resolving_source_links=file_links,caption_occurrences=len(actual_cues),source_words=len(words),scene_spans=len(groups),duration=duration,source_sequence_preserved=True,provisional_grouping=True,overflow=overflow,editable_fields=creative_instructions,approval='needs review',pages=[dict(key=p['key'],id=s['id'],kind=p['kind']) for p,s in zip(pages,slides)])

def generate(template,config,output,grouping=None,representative=False):
    template=Path(template).resolve();output=Path(output).resolve();output.parent.mkdir(parents=True,exist_ok=True)
    state=output.with_suffix('.state.json');lock=output.with_suffix('.lock');fd=os.open(lock,os.O_CREAT|os.O_EXCL|os.O_WRONLY);os.close(fd)
    d=None
    try:
        c,words,cues,groups,duration=read_inputs(config,grouping);fingerprint=source_fingerprint(c,groups);pages=build_plan(c,cues,groups,representative);previous_hash=sha(output) if output.exists() else None
        if output.exists() and not state.exists():raise ValueError('Existing deck has no generator state; refusing to overwrite authored work')
        if state.exists() and json.loads(state.read_text())['fingerprint']!=fingerprint:raise ValueError('Source/grouping changed. Generate to a new output revision; existing authored work is preserved.')
        read_root=Path(c['project']).resolve().parent if c.get('project') else template.parent
        runtime_template=template
        if not template.is_relative_to(read_root):
            # Byte-identical dependency staging keeps native file access within this song project.
            runtime_template=output.parent/'source-template.deckcraft'
            if not runtime_template.exists():shutil.copy2(template,runtime_template)
            if sha(runtime_template)!=sha(template):raise ValueError('Staged template differs from canonical template')
        d=Deck(read_root,output.parent)
        if output.exists():
            d.cmd('file.open',path=str(output));before=snapshot(d)
        else:
            d.cmd('file.open',path=str(runtime_template));originals=inspect_pages(d);sources={note(s)['kind']:s['id'] for s in originals}
            for page in pages:
                d.cmd('slide.go',id=sources[page['kind']]);d.cmd('slide.duplicate');info=d.cmd('document.inspect')
                # Duplicate is current; move it to the end so original template indices stay fixed.
                active=d.cmd('slide.inspect');d.cmd('slide.move',**{'from':active['index'],'to':len(info['slides'])-1})
            for i in reversed(range(len(originals))):d.cmd('slide.delete',index=i)
            for i,page in enumerate(pages):d.cmd('slide.go',index=i);fill_page(d,d.cmd('slide.inspect',index=i),page,c,words,cues,groups,duration,pages,representative)
            before=None
        validation=validate_deck(d,pages,words,cues,groups,duration,representative,c.get('empty_template',False))
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
        if before is None or not output.with_suffix('.pdf').is_file():
            d.cmd('file.export',path=str(output.with_suffix('.pdf')),format='pdf')
        scene=next((i for i,p in enumerate(pages) if p['kind']=='scene'),0);d.cmd('file.export',path=str(output.with_name(output.stem+'-scene.png')),format='png',slide=scene,scale=1)
        if not representative:
            for kind in ['toc','overview','assets','locations']:
                i=next(i for i,p in enumerate(pages) if p['kind']==kind);d.cmd('file.export',path=str(output.with_name(output.stem+'-'+kind+'.png')),format='png',slide=i,scale=1)
        validation['authored_edits_preserved']=before is not None;dump(output.with_suffix('.validation.json'),validation)
        return dict(ok=True,page_plan=pages,deck=str(output),pdf=str(output.with_suffix('.pdf')),proof=str(output.with_name(output.stem+'-scene.png')),reopened_existing=before is not None,validation=validation)
    finally:
        if d:d.close()
        lock.unlink(missing_ok=True)


def build_character_qa_deck(slug: str, out_dir: str | Path | None = None) -> dict:
    from pipeline.step_deckcraft_qa import build_character_qa_deck as _build
    return _build(slug, out_dir)


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--character", default=None, help="Build 5-slide QA review deck for character slug")
    ap.add_argument("--out-dir", default=None)
    ap.add_argument("--revise-template", default=None)
    ap.add_argument("--template", default=None)
    ap.add_argument("--config", default=None)
    ap.add_argument("--output", default=None)
    ap.add_argument("--grouping", default=None)
    ap.add_argument("--representative", action="store_true")
    args = ap.parse_args(argv)
    if args.character:
        print(json.dumps(build_character_qa_deck(args.character, args.out_dir), indent=2))
        return 0
    if args.revise_template:
        revise_template(args.revise_template)
        return 0
    if args.template and args.config and args.output:
        print(json.dumps(generate(args.template, args.config, args.output, args.grouping, args.representative), indent=2))
        return 0
    ap.error("Provide --character <slug> or --template/--config/--output")


if __name__ == "__main__":
    raise SystemExit(main())
