#!/usr/bin/env python3
"""Build an editable Backlot-style scene document through DeckCraft, headlessly."""
import argparse, json, math, subprocess
from pathlib import Path


def build(transcript, source, out, title):
    out=Path(out); out.mkdir(parents=True,exist_ok=True)
    doc=json.loads(Path(transcript).read_text())
    paragraphs=doc.get('paragraphs',[])
    if not paragraphs: raise ValueError('Transcript needs authored paragraphs with text/start/end')
    assets=[]; sections=[]
    if source:
        source=Path(source)
        scenes=json.loads((source/'artifacts/scene_plan.json').read_text())['scenes']
        assets=json.loads((source/'artifacts/asset_manifest.json').read_text()).get('assets',[])
        script=source/'artifacts/script.json'
        sections=json.loads(script.read_text()).get('sections',[]) if script.exists() else []
    else:
        scenes=[dict(id=f'scene-{i+1:02}',start_seconds=p['start'],end_seconds=p['end'],lyric_text=p['text']) for i,p in enumerate(paragraphs)]
    if not scenes: raise ValueError('No scenes to build')
    # Transcript remains the text authority even when a source plan supplies artwork/actions.
    for scene in scenes:
        start,end=scene['start_seconds'],scene['end_seconds']
        if not (math.isfinite(start) and math.isfinite(end) and 0<=start<end):
            raise ValueError('Every scene needs finite, ordered start/end times')
        scene['lyric_text']=' '.join(p['text'] for p in paragraphs if start-0.001<=p['start']<end-0.001)
    contents_pages=math.ceil(len(scenes)/12)
    first_scene=contents_pages
    commands=[]; links=[];titles=[]
    def cmd(command,**params): commands.append((command,params))
    def text(value,rect,size=18,color='#e8ecf3'):
        cmd('insert.textBox',rect=rect,text=value);cmd('format.size',size=size);cmd('format.color',color=color)
    cmd('slide.layout',layout='blank')
    for _ in range(contents_pages+len(scenes)-1): cmd('slide.new',layout='blank')
    current_page=-1
    def page():
        nonlocal current_page
        current_page+=1
        cmd('slide.go',index=current_page)
        cmd('design.background',color='#101319')
    def clock(n):return f'{int(n)//60}:{int(n)%60:02}'
    for c in range(contents_pages):
        page();text('BACKLOT / MOVIE BOARD',[32,22,800,20],11,'#64d7c2')
        text(title,[32,48,880,44],30)
        text(f'{len(scenes)} scenes · Contents {c+1}/{contents_pages} · Select a scene to jump',[32,102,880,28],15,'#8e97a8')
        for j,scene in enumerate(scenes[c*12:(c+1)*12]):
            i=c*12+j; x=32+(j//6)*456; y=156+(j%6)*56
            label=scene.get('lyric_text') or 'Instrumental'
            label=' '.join(label.split());label=label[:35]+'…' if len(label)>36 else label
            text(f'{i+1:02}   {label}',[x,y,424,30],14)
            cmd('insert.hyperlink',slide=first_scene+i,tooltip=f'Open scene {i+1}')
            text(f'{clock(scene["start_seconds"])}–{clock(scene["end_seconds"])}',[x+32,y+27,390,20],11,'#8e97a8')
            links.append(dict(page=c+1,to_page=first_scene+i+1,rect=[x,y,x+424,y+49]))
    for i,scene in enumerate(scenes):
        page();start,end=scene['start_seconds'],scene['end_seconds']; lyric=scene.get('lyric_text','')
        label=f'{i+1:02} · {clock(start)}–{clock(end)}';titles.append(label+' · '+(lyric or 'Instrumental'))
        text(title.upper(),[32,20,720,20],11,'#8e97a8')
        text(label,[32,48,820,38],28)
        text('Contents',[812,27,116,26],13,'#64d7c2');cmd('insert.hyperlink',slide=i//12)
        links.append(dict(page=first_scene+i+1,to_page=i//12+1,rect=[812,27,928,53]))
        ref=next((a for a in reversed(assets) if a.get('scene_id')==scene['id'] and a.get('type')=='image' and a.get('subtype')=='storyboard_design_reference_only' and (source/a['path']).is_file()),None)
        if ref: cmd('insert.picture',path=str(source/ref['path']),rect=[32,102,620,349])
        else: text('Scene artwork to come',[70,245,540,50],24,'#8e97a8')
        text('ACTION',[680,106,248,24],11,'#64d7c2')
        section=next((s for s in sections if s.get('id')==scene.get('script_section_id',scene['id'])),{})
        acting=' '.join(c.get('description','') for c in section.get('enhancement_cues',[]) if c.get('type')=='animation')
        action=(acting or scene.get('movement') or scene.get('description') or 'Add the movement for this scene.').replace('_',' ')
        words=action.split(); excerpt=' '.join(words[:48])+('…' if len(words)>48 else '')
        text(excerpt,[680,140,248,230],18)
        text('DESIGN REFERENCE\nArtwork and acting still need review.',[680,378,248,70],12,'#8e97a8')
        text(lyric or 'Instrumental',[32,470,896,55],23)
        cmd('slide.notes',text='REFERENCE PROMPT\n'+(ref.get('prompt','') if ref else 'No reference artwork attached.')+'\n\nACTION\n'+action+'\n\nSOURCE\n'+scene['id'])
    deck=out/'movie-board.deckcraft';pdf=out/'movie-board.pdf'
    args=['deckcraft-cli','run']
    for command,params in commands:args+=['--cmd',command+'='+json.dumps(params)]
    args+=['--save',str(deck),'--export',str(pdf)]
    subprocess.run(args,check=True,capture_output=True,text=True)
    previews=out/'pages';subprocess.run(['deckcraft-cli','render',str(deck),'--all','--scale','1.5',str(previews)],check=True,capture_output=True,text=True)
    result=dict(pdf=str(pdf),deck=str(deck),previews=[str(p) for p in sorted(previews.glob('slide-*.png'))],page_size=[960,540],links=links,scene_titles=titles,first_scene_page=first_scene+1,title=title,template='backlot')
    (out/'manifest.json').write_text(json.dumps(result,indent=2))
    return result

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--transcript',required=True);p.add_argument('--source');p.add_argument('--out',required=True);p.add_argument('--title',required=True);a=p.parse_args()
    try: print(json.dumps(build(a.transcript,a.source,a.out,a.title)))
    except subprocess.CalledProcessError as e: raise SystemExit(e.stderr or e.stdout or str(e))
