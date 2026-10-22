#!/usr/bin/env python3
"""Build an editable Backlot-style scene document through DeckCraft, headlessly."""
import argparse, json, math, subprocess, shlex
from pathlib import Path


def canonical_scenes(scene_map):
    doc=json.loads(Path(scene_map).read_text())
    scenes=[]
    previous=0.0
    targets=set()
    for packet in doc.get('scenes',[]):
        start,end=packet['start'],packet['end']
        if not (math.isfinite(start) and math.isfinite(end) and 0<=start<end and start>=previous-0.001):
            raise ValueError('Canonical scenes need finite ordered non-overlapping recording times')
        target=packet['target']
        if target in targets: raise ValueError('Duplicate canonical scene target: '+target)
        targets.add(target);previous=end
        # Keep each mapped occurrence/correction; do not independently regroup caption paragraphs.
        scenes.append(dict(packet,id=target,start_seconds=start,end_seconds=end,
            lyric_text=' '.join(str(line['lyric']) for line in packet.get('lines',[]))))
    if not scenes: raise ValueError('No canonical scenes; run storyboard.init first')
    return scenes


def build(transcript, source, out, title, scene_map):
    # The transcript is already the CLI schema. No lyric reformatting or second editable document.
    from deckcraft_template import generate, SIZE, SCENE_ART_REGION
    out=Path(out);out.mkdir(parents=True,exist_ok=True)
    packets=canonical_scenes(scene_map)
    metadata=json.loads(Path(scene_map).read_text())
    words_doc=json.loads(Path(transcript).read_text())
    words=words_doc if isinstance(words_doc,list) else words_doc['words']
    groups=[]
    for packet in packets:
        selected=[w for w in words if packet['start_seconds']-0.001<=w['start']<packet['end_seconds']-0.001]
        group=dict(id=packet['id'],word_ids=[w['id'] for w in selected],span_start=packet['start_seconds'],span_end=packet['end_seconds'],kind=packet.get('kind','provisional scene'),status=packet.get('status','needs review'),reason='Canonical CLI scene span; review grouping without forced verse taxonomy.')
        groups.append(group)
    config=dict(title=title,transcript=str(transcript),captions=metadata['captions'],duration_sec=metadata['duration_sec'],recording_duration_verified=metadata.get('recording_duration_verified',False),audio=metadata.get('audio'),project=metadata.get('project'),scene_content={p['id']:dict(sections=p.get('sections',{}),notes=p.get('notes',[]),source=p.get('source'),lines=p.get('lines',[])) for p in packets})
    config_path=out/'template-inputs.json';config_path.write_text(json.dumps(config,indent=2))
    grouping=out/'scene-map-input.json';grouping.write_text(json.dumps(dict(groups=groups),indent=2))
    template=Path(__file__).parent/'templates/studio-template.deckcraft'
    deck=out/'movie-board.deckcraft'
    result=generate(template,config_path,deck,grouping)
    pages=out/'pages'
    subprocess.run(['deckcraft-cli','render',str(deck),'--all','--scale','1',str(pages)],check=True,capture_output=True,text=True)
    validation=result['validation'];page_list=validation['pages']
    first_scene=next(i for i,page in enumerate(page_list) if page['kind']=='scene')
    links=validation['link_regions']
    result.update(template='backlot',template_file=str(template),scene_map=str(scene_map),scene_targets=[p['id'] for p in packets],scene_count=len(packets),first_scene_page=first_scene+1,
        scene_titles=[f'{p["id"]} · {p["start_seconds"]:.3f}–{p["end_seconds"]:.3f}' for p in packets],
        previews=[str(p) for p in sorted(pages.glob('slide-*.png'))],page_size=SIZE,stage_rect=SCENE_ART_REGION,links=links,page_titles=[p['title'] for p in result['page_plan']],title=title,
        editor='Existing DeckCraft template/document and Studio picture placement/Save; no parallel Markdown editor',
        next_commands=[shlex.join(['deckcraft-cli','info',str(deck)]),shlex.join(['deckcraft-cli','run','--in',str(deck),'--print','--cmd','slide.inspect={"index":'+str(first_scene)+'}'])])
    (out/'manifest.json').write_text(json.dumps(result,indent=2));return result

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--transcript',required=True);p.add_argument('--scene-map',required=True);p.add_argument('--source');p.add_argument('--out',required=True);p.add_argument('--title',required=True);a=p.parse_args()
    try: print(json.dumps(build(a.transcript,a.source,a.out,a.title,a.scene_map)))
    except subprocess.CalledProcessError as e: raise SystemExit(e.stderr or e.stdout or str(e))
