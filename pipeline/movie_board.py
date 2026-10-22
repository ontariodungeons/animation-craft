#!/usr/bin/env python3
"""Build an editable Backlot-style scene document through DeckCraft, headlessly."""
import argparse, json, math, subprocess, shlex, re, hashlib
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


def normalize_scene_content(packets, expected_fields):
    """Map existing labelled source text; preserve unknowns and full native slide notes."""
    aliases={'reference_prompt':('reference_prompt','image_prompt'),'animation_prompt':('animation_prompt',),
             'layer_stack':('layer_stack','continuity','linked_assets'),'action':('action','action_staging','staging'),
             'animated_text':('animated_text',),'drawing_cues':('drawing_cues','drawing','teaching_cues')}
    targets={'reference_prompt':'image_prompt','layer_stack':'continuity','drawing_cues':'drawing'}
    notes_only={'skills','build_steps','acceptance_checks','plan_vs_actual','visual_review'}
    report=dict(expected_fields=list(expected_fields),mapped=[],missing=[],unmapped=[],retained_in_slide_notes=[],fit_review=[])
    content={}
    for packet in packets:
        source=packet.get('sections') or {};sections={};used=set()
        normalized={re.sub(r'[^a-z0-9]+','_',k.lower()).strip('_'):(k,v) for k,v in source.items()}
        for field,names in aliases.items():
            role='user.scene.'+targets.get(field,field)
            if role not in expected_fields:continue
            found=next((normalized[n] for n in names if n in normalized and normalized[n][1]),None)
            if found:label,text=found;used.add(label)
            elif field=='action':
                plans=[n[len('Scene plan: '):] for n in packet.get('notes',[]) if n.startswith('Scene plan: ')]
                label='Notes / Scene plan';text='\n'.join(plans)
            else:label=field;text=''
            if text:
                sections[field]=text;report['mapped'].append(dict(scene=packet['id'],source=label,target=role))
                if len(text)>220:report['fit_review'].append(dict(scene=packet['id'],field=role,characters=len(text),reason='Full source wording retained; review native shrink-to-fit at viewing size'))
            else:report['missing'].append(dict(scene=packet['id'],field=role))
        for label,text in source.items():
            if label in used:continue
            key=re.sub(r'[^a-z0-9]+','_',label.lower()).strip('_')
            item=dict(scene=packet['id'],source=label,text=text)
            report['retained_in_slide_notes'].append(dict(scene=packet['id'],source=label,characters=len(text))) if key in notes_only else report['unmapped'].append(item)
        content[packet['id']]=dict(sections=sections,notes=packet.get('notes',[]),source=packet.get('source'),
                                  source_sections=source,lines=packet.get('lines',[]),title=packet.get('title'))
    report['needs_correction']=bool(report['unmapped'])
    return content,report

def template_schema(template,out):
    from deckcraft_template import Deck,inspect_pages,note
    d=Deck(template.parent,out)
    try:
        d.cmd('file.open',path=str(template))
        return {note(sl)['kind']:[dict(role=sh['altText'],rect=sh['box']) for sh in sl['shapes']
                if sh.get('altText','').startswith('user.')] for sl in inspect_pages(d)}
    finally:d.close()

def build(transcript, source, out, title, scene_map, allow_timing_overrun=False):
    # The transcript is already the CLI schema. No lyric reformatting or second editable document.
    from deckcraft_template import generate, SIZE, SCENE_ART_REGION
    out=Path(out);out.mkdir(parents=True,exist_ok=True)
    packets=canonical_scenes(scene_map)
    metadata=json.loads(Path(scene_map).read_text())
    words_doc=json.loads(Path(transcript).read_text())
    words=words_doc if isinstance(words_doc,list) else words_doc['words']
    words=[dict(w,id=w.get('id',f'w{i}')) for i,w in enumerate(words)]
    groups=[]
    for packet in packets:
        selected=[w for w in words if packet['start_seconds']-0.001<=w['start']<packet['end_seconds']-0.001]
        group=dict(id=packet['id'],word_ids=[w['id'] for w in selected],span_start=packet['start_seconds'],span_end=packet['end_seconds'],kind=packet.get('kind','provisional scene'),word_assignment='source-start timestamp; canonical scene cuts preserved',status=packet.get('status','needs review'),reason='Canonical CLI scene span; review grouping without forced verse taxonomy.')
        groups.append(group)
    config=dict(title=title,transcript=str(transcript),captions=metadata.get('captions'),allow_timing_overrun=allow_timing_overrun,duration_sec=metadata['duration_sec'],recording_duration_verified=metadata.get('recording_duration_verified',False),audio=metadata.get('audio'),project=metadata.get('project'),scene_content={p['id']:dict(sections=p.get('sections',{}),notes=p.get('notes',[]),source=p.get('source'),lines=p.get('lines',[])) for p in packets})
    template=Path(__file__).parent/'templates/studio-template.deckcraft'
    schema=template_schema(template,out)
    config['scene_content'],mapping=normalize_scene_content(packets,[f['role'] for f in schema['scene']])
    config['fit_imported_text']=bool(source)
    config['scene_images']={p['id']:dict(file=p['reference'],name=Path(p['reference']).name) for p in packets if p.get('reference') and Path(p['reference']).is_file()}
    source_root=Path(source) if source else None
    official=source_root/'artifacts/official_lyrics.json' if source_root else None
    if official and official.is_file():config['official_lyrics_source']=str(official)
    mapping['timing_conflicts']=[dict(scene=g['id'],word=w['id'],text=w['text'],start=w['start'],end=w['end'],scene_end=g['span_end'],resolution='Source timing preserved; word occurrence assigned by its original start') for g in groups for w in words if w['id'] in g['word_ids'] and w['end']>g['span_end']+.001]
    config['asset_links']=metadata.get('assets',[])
    references=[a for a in config['asset_links'] if a.get('path') and 'approved' in Path(a['path']).parent.name and Path(a['path']).is_file()]
    if references:config['assets']=[dict(id=a['id'],name=a.get('name',a['id']),file=a['path']) for a in references]
    locations=sorted((source_root/'assets/backgrounds').glob('*.png')) if source_root else []
    if locations:config['locations']=[dict(id=p.stem,name=p.stem,file=str(p)) for p in locations]
    overrun=max(w['end'] for w in words)-float(metadata['duration_sec'])
    if overrun>.001:
        config['timing_warning']=f"Source word end {max(w['end'] for w in words):.6f}s exceeds measured audio {metadata['duration_sec']:.6f}s by {overrun:.6f}s. Raw words/timestamps retained; timing is not validated."
        mapping['timing_warning']=config['timing_warning']
    mapping['needs_correction']=bool(mapping['unmapped']);mapping['template_file']=str(template);mapping['template_schema']=schema
    mapping['source_words']=len(words);mapping['scene_spans']=[[p['start_seconds'],p['end_seconds']] for p in packets]
    config['mapping']=mapping
    (out/'mapping.json').write_text(json.dumps(mapping,indent=2))
    config_path=out/'template-inputs.json';config_path.write_text(json.dumps(config,indent=2))
    grouping=out/'scene-map-input.json';grouping.write_text(json.dumps(dict(groups=groups),indent=2))
    template=Path(__file__).parent/'templates/studio-template.deckcraft'
    deck=out/'movie-board.deckcraft'
    try:result=generate(template,config_path,deck,grouping)
    except Exception as error:
        return dict(ok=False,status='draft blocked; active storyboard unchanged',error=str(error),template_file=str(template),mapping=mapping,config=str(config_path),scene_map=str(scene_map),correction='Resolve the reported source/template mismatch, then rerun the same CLI command; original data is retained')
    pages=out/'pages'
    pages.mkdir(exist_ok=True)
    for i in range(len(result['page_plan'])):
        subprocess.run(['pdfcraft-cli','render',result['pdf'],'--page',str(i+1),'--dpi','72','--out',str(pages/f'slide-{i+1:03}.png')],check=True,capture_output=True,text=True)
    validation=result['validation'];page_list=validation['pages']
    first_scene=next(i for i,page in enumerate(page_list) if page['kind']=='scene')
    links=validation['link_regions']
    result.update(mapping=mapping,template='backlot',template_file=str(template),scene_map=str(scene_map),scene_targets=[p['id'] for p in packets],scene_count=len(packets),first_scene_page=first_scene+1,
        scene_titles=[f'{p["id"]} · {p["start_seconds"]:.3f}–{p["end_seconds"]:.3f}' for p in packets],
        previews=[str(p) for p in sorted(pages.glob('slide-*.png'))],page_size=SIZE,stage_rect=SCENE_ART_REGION,links=links,page_titles=[p['title'] for p in result['page_plan']],title=title,
        editor='Editable PDF / native PDFCraft fields; existing Studio picture placement and Save',source_transcript=str(transcript),source_transcript_sha256=hashlib.sha256(Path(transcript).read_bytes()).hexdigest(),source_producer_label=Path(transcript).parent.name,producer_executable_version='not recorded by native array',source_words=len(words),timing_valid=overrun<=.001,timing_warning=config.get('timing_warning'),
        next_commands=[shlex.join(['deckcraft-cli','info',str(deck)]),shlex.join(['deckcraft-cli','run','--in',str(deck),'--print','--cmd','slide.inspect={"index":'+str(first_scene)+'}'])])
    (out/'manifest.json').write_text(json.dumps(result,indent=2));return result

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--allow-timing-overrun',action='store_true');p.add_argument('--transcript',required=True);p.add_argument('--scene-map',required=True);p.add_argument('--source');p.add_argument('--out',required=True);p.add_argument('--title',required=True);a=p.parse_args()
    try: print(json.dumps(build(a.transcript,a.source,a.out,a.title,a.scene_map,a.allow_timing_overrun)))
    except subprocess.CalledProcessError as e: raise SystemExit(e.stderr or e.stdout or str(e))
