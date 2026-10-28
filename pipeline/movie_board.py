#!/usr/bin/env python3
"""Build the editable Magic Studio storyboard from its single DeckCraft template."""
import argparse, json, math, shutil, subprocess, shlex, re, hashlib, sys
from pathlib import Path

_REPO_ROOT = str(Path(__file__).resolve().parents[1])
if _REPO_ROOT not in sys.path:
    sys.path.insert(0, _REPO_ROOT)


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

def align_imported_sections(packets,words,source,evidence=None):
    """Repair cuts through an exact known lyric line; never estimate word times."""
    if not source:return []
    root=Path(source);plan=root/'artifacts/scene_plan.json';script=root/'artifacts/script.json'
    if not plan.is_file() or not script.is_file():return []
    scenes=json.loads(plan.read_text()).get('scenes',[])
    sections={s['id']:s for s in json.loads(script.read_text()).get('sections',[])}
    line_source=root/'hyperframes/film-data.js';linked={}
    if line_source.is_file():
        raw=line_source.read_text().strip()
        if raw.startswith('window.FISH_DATA = '):linked={l['id']:l for l in json.loads(raw.split('=',1)[1].strip().removesuffix(';')).get('lyrics',[])}
    if len(scenes)!=len(packets):
        if evidence is not None:evidence.append(dict(status='unassessed',reason='Supplied scene plan and storyboard scene counts differ'))
        return []
    token=lambda text:re.sub(r'[^\w]+','',text.casefold())
    tokens=[token(w['text']) for w in words];changes=[]
    for i,(packet,original) in enumerate(zip(packets,scenes)):
        lyrics=[sections[k] for k in original.get('script_section_id','').split(',') if k in sections and sections[k].get('lyrics',True) and sections[k].get('text')!='[instrumental]']
        if not lyrics:
            if evidence is not None and i:evidence.append(dict(scene_id=packet['id'],status='unassessed',reason='No supplied lyric section opening'))
            continue
        packet['source_lines']=[dict(id=l['id'],text=linked.get(l['id'],l)['text'],source=str(line_source if l['id'] in linked else script)) for l in lyrics]
        label=lyrics[0].get('label','').split(' line')[0]
        if label.lower().startswith('chorus'):
            occurrence=re.match(r'ch(\d+)',lyrics[0]['id']);packet['kind']='chorus';packet['section_label']='Chorus'+(' '+occurrence[1] if occurrence else '')
        elif label.lower().startswith('verse'):packet['kind']='verse';packet['section_label']=label
        if not i:continue
        expected=[token(t) for t in lyrics[0]['text'].split()]
        hits=[n for n in range(len(words)-len(expected)+1) if tokens[n:n+len(expected)]==expected and packets[i-1]['start_seconds']<=words[n]['start']<packet['end_seconds']]
        if evidence is not None:evidence.append(dict(scene_id=packet['id'],status='assessed' if len(hits)==1 else 'unassessed',source_line=lyrics[0]['id'],reason='Unique exact native-word match' if len(hits)==1 else f'Expected one native-word match; found {len(hits)}'))
        if len(hits)!=1:continue
        at=hits[0];onset=words[at]['start'];end=words[at+len(expected)-1]['end'];cut=packet['start_seconds']
        if onset<cut:
            packets[i-1]['end_seconds']=onset;packet['start_seconds']=onset
            changes.append(dict(scene=packet['id'],previous_scene=packets[i-1]['id'],old_boundary=cut,boundary=onset,word=words[at]['id'],source_line=lyrics[0]['id'],convention='next source line onset; half-open intervals'))
    return changes

def source_line_breaks(words,lines):
    normalize=lambda text:[re.sub(r'[^\w]+','',t.casefold()) for t in text.split()]
    tokens=[normalize(w['text'])[0] for w in words];mapped=[];used=[]
    for line in lines:
        expected=normalize(line['text']);hits=[i for i in range(len(words)-len(expected)+1) if tokens[i:i+len(expected)]==expected]
        if len(hits)!=1:return []
        selected=words[hits[0]:hits[0]+len(expected)];used.extend(w['id'] for w in selected)
        mapped.append(dict(id=line['id'],source=line['source'],text=' '.join(w['text'] for w in selected),word_ids=[w['id'] for w in selected],start=selected[0]['start'],end=selected[-1]['end']))
    return mapped if used==[w['id'] for w in words] else []

def chorus_phrases(words):
    """Four display phrases: repeated opening, response, opening, response."""
    tokens=[re.sub(r'[^\w]+','',w['text'].casefold()) for w in words]
    candidates=[]
    for repeat in range(2,len(words)-2):
        prefix=0
        while prefix<repeat and repeat+prefix<len(words) and tokens[prefix]==tokens[repeat+prefix]:prefix+=1
        if prefix>=2 and prefix<repeat and repeat+prefix<len(words):candidates.append((prefix,repeat))
    if not candidates:raise ValueError('Four chorus panels require an unambiguous repeated opening in the native words')
    prefix,repeat=max(candidates)
    cuts=[0,prefix,repeat,repeat+prefix,len(words)]
    return [dict(text=' '.join(w['text'] for w in words[a:b]),word_ids=[w['id'] for w in words[a:b]],start=words[a]['start'],end=words[b-1]['end']) for a,b in zip(cuts,cuts[1:])]


def doctor(manifest,source):
    """Explicit, read-only check for scenes crossing supplied section onsets."""
    board=json.loads(Path(manifest).read_text());root=Path(source)
    result=dict(ok=True,project=board.get('project'),storyboard=str(manifest),healthy=None,
        status='insufficient_source_evidence',tasks=[],assessed=[],unassessed=[],
        scope_assessed='Scene cuts crossing an exact supplied section opening line',
        scope_unassessed=['Musical structure without supplied section labels','Timing accuracy in the recording','Rendered typography, artwork, template and animation quality'],diagnostic_only=True)
    needed=[root/'artifacts/scene_plan.json',root/'artifacts/script.json']
    if not all(p.is_file() for p in needed):
        result['unassessed']=[dict(reason='No supplied section plan/script',missing=[str(p) for p in needed if not p.is_file()])];return result
    words_doc=json.loads(Path(board['source_transcript']).read_text());words=words_doc if isinstance(words_doc,list) else words_doc['words']
    words=[dict(w,id=w.get('id',f'w{i}')) for i,w in enumerate(words)]
    pages=[(i+1,p['group']) for i,p in enumerate(board.get('page_plan',[])) if p.get('group')]
    packets=[dict(id=g['id'],start_seconds=g['span_start'],end_seconds=g['span_end']) for _,g in pages]
    evidence=[];proposals=align_imported_sections(packets,words,root,evidence)
    result['assessed']=[e for e in evidence if e['status']=='assessed'];result['unassessed']=[e for e in evidence if e['status']!='assessed']
    for fix in proposals:
        page=next((i for i,g in pages if g['id']==fix['previous_scene']),None)
        affected=[w for w in words if fix['boundary']<=w['start']<fix['old_boundary']]
        result['tasks'].append(dict(id='section-boundary:'+fix['previous_scene']+':'+fix['scene'],priority=1,
            scene_id=fix['previous_scene'],page_id=page,next_scene_id=fix['scene'],
            suspected_flaw='Scene includes the opening of the next supplied section',
            source_word_ids=[w['id'] for w in affected],affected_lyrics=' '.join(w['text'] for w in affected),
            source_range=[affected[0]['start'],affected[-1]['end']] if affected else [],
            expected=dict(boundary=fix['boundary'],convention=fix['convention']),actual=dict(boundary=fix['old_boundary']),
            evidence=dict(script=str(needed[1]),section_line=fix['source_line'],native_transcript=board['source_transcript'],onset_word=fix['word']),
            confidence='Exact native-word match to a supplied section line; not a musical inference',
            proposed_fix=f"Review {fix['previous_scene']} / page {page}, then set its end and {fix['scene']} start to {fix['boundary']}s; preserve every native word/timestamp"))
    result['healthy']=False if result['tasks'] else None if result['unassessed'] or not result['assessed'] else True
    result['status']='issues_found' if result['tasks'] else 'no_supported_issues' if result['healthy'] else 'insufficient_source_evidence'
    return result


def build(transcript, source, out, title, scene_map, allow_timing_overrun=False, panels=None, template=None):
    # The transcript is already the CLI schema. No lyric reformatting or second editable document.
    from deckcraft_template import generate, SIZE, SCENE_ART_REGION
    out=Path(out);out.mkdir(parents=True,exist_ok=True)
    packets=canonical_scenes(scene_map)
    metadata=json.loads(Path(scene_map).read_text())
    words_doc=json.loads(Path(transcript).read_text())
    words=words_doc if isinstance(words_doc,list) else words_doc['words']
    words=[dict(w,id=w.get('id',f'w{i}')) for i,w in enumerate(words)]
    source_root=Path(source) if source else Path(metadata['project']).parent if metadata.get('project') else None
    alignment=[]  # Project creation owns timing; storyboard edits never realign recorded words.
    groups=[]
    for packet in packets:
        selected=[w for w in words if packet['start_seconds']-0.001<=w['start']<packet['end_seconds']-0.001]
        group=dict(id=packet['id'],word_ids=[w['id'] for w in selected],span_start=packet['start_seconds'],span_end=packet['end_seconds'],kind=packet.get('kind','provisional scene'),word_assignment='source-start timestamp; canonical scene cuts preserved',status=packet.get('status','needs review'),notes=packet.get('notes') or [],reason='Canonical CLI scene span; review grouping without forced verse taxonomy.')
        group['section_label']=packet.get('section_label')
        group['lyric_lines']=source_line_breaks(selected,packet.get('source_lines',[]))
        if panels==packet['id'] or packet.get('panels'):
            if group['kind']!='chorus':raise ValueError('Four-line panel layout requires a source-labelled chorus')
            group['panels']=chorus_phrases(selected)
            for panel in group['panels']:panel['line_id']=next((l['id'] for l in group['lyric_lines'] if set(panel['word_ids'])<=set(l['word_ids'])),None)
        groups.append(group)
    config=dict(title=title,transcript=str(transcript),captions=metadata.get('captions'),allow_timing_overrun=allow_timing_overrun,duration_sec=metadata['duration_sec'],recording_duration_verified=metadata.get('recording_duration_verified',False),audio=metadata.get('audio'),project=metadata.get('project'),scene_content={p['id']:dict(sections=p.get('sections',{}),notes=p.get('notes',[]),source=p.get('source'),lines=p.get('lines',[])) for p in packets})
    canonical_template=Path(_REPO_ROOT)/'pipeline/templates/studio-template.deckcraft'
    template=Path(template).resolve() if template else canonical_template.resolve()
    if template!=canonical_template.resolve():
        raise ValueError('Magic Studio uses only pipeline/templates/studio-template.deckcraft')
    schema=template_schema(template,out)
    config['scene_content'],mapping=normalize_scene_content(packets,[f['role'] for f in schema['scene']])
    # Actual saved user fields take precedence over imported notes; lyrics remain native words.
    for packet in packets:
        for f in packet.get('text_fields',[]):
            if f.get('role','').startswith('user.scene.') and f.get('field') in ['action','continuity','image_prompt','animation_prompt','animated_text','drawing']:
                key={'continuity':'layer_stack','image_prompt':'reference_prompt','drawing':'drawing_cues'}.get(f['field'],f['field'])
                config['scene_content'][packet['id']]['sections'][key]=f['value']
    config['fit_imported_text']=bool(source_root)
    config['scene_images']={p['id']:dict(file=p['reference'],name=Path(p['reference']).name) for p in packets if p.get('reference') and Path(p['reference']).is_file()}
    official=source_root/'artifacts/official_lyrics.json' if source_root else None
    if official and official.is_file():config['official_lyrics_source']=str(official)
    mapping['timing_conflicts']=[dict(scene=g['id'],word=w['id'],text=w['text'],start=w['start'],end=w['end'],scene_end=g['span_end'],resolution='Source timing preserved; word occurrence assigned by its original start') for g in groups for w in words if w['id'] in g['word_ids'] and w['end']>g['span_end']+.001]
    config['asset_links']=metadata.get('assets',[])
    previous=metadata.get('previous_board') or {}
    content=metadata.get('page_content') or []
    config['page_content']={page['key']:[f for f in content if f.get('page')==i+1 and f.get('role','').startswith('user.')]
                            for i,page in enumerate(previous.get('page_plan',[])) if page['kind']!='scene' and not (page['kind']=='locations' and metadata.get('backgrounds') is not None)}
    # Keep authored inventory pages; a scene edit must not replace them with a new asset search.
    for kind,key in [('assets','assets'),('locations','locations')]:
        retained=[record for page in previous.get('page_plan',[]) if page['kind']==kind for record in page.get('sheet',[])]
        if retained:config[key]=retained
    if metadata.get('backgrounds') is not None:
        config['locations']=metadata['backgrounds']
        for record in config['locations']:
            for i,page in enumerate(previous.get('page_plan',[])):
                if page['kind']!='locations':continue
                for slot,old in enumerate(page.get('sheet',[])):
                    if old['id']!=record['id']:continue
                    field=next((f for f in content if f.get('page')==i+1 and f.get('field')==f'sheet.{slot}.notes'),None)
                    if field:record.update(notes=field['value'],notes_exact=True)
    overrun=max(w['end'] for w in words)-float(metadata['duration_sec'])
    if overrun>.001:
        config['timing_warning']=f"Source word end {max(w['end'] for w in words):.6f}s exceeds measured audio {metadata['duration_sec']:.6f}s by {overrun:.6f}s. Raw words/timestamps retained; timing is not validated."
        mapping['timing_warning']=config['timing_warning']
    mapping['needs_correction']=bool(mapping['unmapped']);mapping['template_file']=str(template);mapping['template_schema']=schema
    mapping['line_mapping']=[dict(scene=g['id'],line_ids=[l['id'] for l in g['lyric_lines']],status='exact source-word sequence' if g['lyric_lines'] else 'unmapped native words retained; no line IDs invented') for g in groups]
    mapping['import_alignment']=alignment;mapping['source_words']=len(words);mapping['scene_spans']=[[p['start_seconds'],p['end_seconds']] for p in packets]
    config['mapping']=mapping
    (out/'mapping.json').write_text(json.dumps(mapping,indent=2))
    config_path=out/'template-inputs.json';config_path.write_text(json.dumps(config,indent=2))
    grouping=out/'scene-map-input.json';grouping.write_text(json.dumps(dict(groups=groups),indent=2))
    deck=out/'movie-board.deckcraft'
    try:result=generate(template,config_path,deck,grouping)
    except Exception as error:
        return dict(ok=False,status='draft blocked; active storyboard unchanged',error=str(error),template_file=str(template),mapping=mapping,config=str(config_path),scene_map=str(scene_map),correction='Resolve the reported source/template mismatch, then rerun the same CLI command; original data is retained')
    pages=out/'pages'
    pages.mkdir(exist_ok=True)
    pdf_bin = shutil.which('pdfcraft-cli') or 'pdfcraft-cli'
    for i in range(len(result['page_plan'])):
        subprocess.run([pdf_bin,'render',result['pdf'],'--page',str(i+1),'--dpi','72','--out',str(pages/f'slide-{i+1:03}.png')],check=True,capture_output=True,text=True)
    validation=result['validation'];page_list=validation['pages']
    first_scene=next(i for i,page in enumerate(page_list) if page['kind']=='scene')
    links=validation['link_regions']
    result.update(mapping=mapping,template='deckcraft',template_file=str(template),scene_map=str(scene_map),scene_targets=[p['id'] for p in packets],scene_count=len(packets),first_scene_page=first_scene+1,
        scene_titles=[f'{p["id"]} · {p["start_seconds"]:.3f}–{p["end_seconds"]:.3f}' for p in packets],
        previews=[str(p) for p in sorted(pages.glob('slide-*.png'))],page_size=SIZE,stage_rect=SCENE_ART_REGION,links=links,page_titles=[p['title'] for p in result['page_plan']],title=title,
        editor='DeckCraft native document; PDF export',source_transcript=str(transcript),source_transcript_sha256=hashlib.sha256(Path(transcript).read_bytes()).hexdigest(),source_producer_label=Path(transcript).parent.name,producer_executable_version='not recorded by native array',source_words=len(words),timing_valid=overrun<=.001,timing_warning=config.get('timing_warning'),
        next_commands=[shlex.join(['deckcraft-cli','info',str(deck)]),shlex.join(['deckcraft-cli','run','--in',str(deck),'--print','--cmd','slide.inspect={"index":'+str(first_scene)+'}'])])
    # Keep saved placements linked; remap only a full-frame overlay on a newly panelled scene.
    from deckcraft_template import fit_rect,panel_region,SCENE_ART_REGION
    placements=[]
    for packet in packets:
        page=next(i+1 for i,p in enumerate(result['page_plan']) if p.get('group',{}).get('id')==packet['id'])
        for original in packet.get('saved_placements',[]):
            item=dict(original,page=page)
            if panels==packet['id'] and not packet.get('panels'):
                target=fit_rect(SCENE_ART_REGION[2],SCENE_ART_REGION[3],panel_region(3));scale=target[2]/SCENE_ART_REGION[2];r=item['rect']
                item['rect']=[target[0]+(r[0]-60)*scale,target[1]+(r[1]-220)*scale,r[2]*scale,r[3]*scale]
            placements.append(item)
    for original in previous.get('design',{}).get('placements',[]):
        old_page=previous.get('page_plan',[])[original['page']-1]
        if old_page['kind']=='scene' or (old_page['kind']=='locations' and metadata.get('backgrounds') is not None):continue
        page=next((i+1 for i,p in enumerate(result['page_plan']) if p['key']==old_page['key']),None)
        if page:placements.append(dict(original,page=page))
    if placements:result['design']=dict(placements=placements,dirty=False)
    (out/'manifest.json').write_text(json.dumps(result,indent=2));return result

if __name__=='__main__':
    import sys
    if '--doctor' in sys.argv:
        p=argparse.ArgumentParser(description='Read-only source section boundary check');p.add_argument('--doctor',required=True);p.add_argument('--source',required=True);a=p.parse_args()
        print(json.dumps(doctor(a.doctor,a.source)));raise SystemExit(0)
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--allow-timing-overrun',action='store_true');p.add_argument('--transcript',required=True);p.add_argument('--template',default=str(Path(_REPO_ROOT)/'pipeline/templates/studio-template.deckcraft'));p.add_argument('--scene-map',required=True);p.add_argument('--source');p.add_argument('--out',required=True);p.add_argument('--title',required=True);p.add_argument('--panels');a=p.parse_args()
    try: print(json.dumps(build(a.transcript,a.source,a.out,a.title,a.scene_map,a.allow_timing_overrun,a.panels,a.template)))
    except subprocess.CalledProcessError as e: raise SystemExit(e.stderr or e.stdout or str(e))
