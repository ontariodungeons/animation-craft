#!/usr/bin/env python3
"""Insert one already-rendered character track into a local HyperFrames composition."""
import argparse, hashlib, html, json, math, re, shutil, time
from pathlib import Path

def place(project,request):
    root=Path(project)/'hyperframes'; entry=root/'index.html'
    folder=root/'assets/animation'; folder.mkdir(parents=True,exist_ok=True)
    key=request['id']
    if not re.fullmatch(r'[a-zA-Z0-9_-]+',key): raise ValueError('id must be a stable filename-safe identifier')
    existing_record_path=folder/f'{key}.json'
    existing_record=json.loads(existing_record_path.read_text()) if existing_record_path.is_file() else {}
    raw_path=request.get('path') or existing_record.get('media') or existing_record.get('path')
    if not raw_path: raise ValueError('path is required for a new placement')
    source=Path(raw_path).resolve(strict=True)
    generation=None
    if 'generation_receipt' in request:
        project_root=Path(project).resolve(strict=True)
        receipt=(project_root/request['generation_receipt']).resolve(strict=True)
        if not receipt.is_relative_to(project_root): raise ValueError('generation_receipt must belong to this project')
        data=json.loads(receipt.read_text())
        if data.get('format')!='magicstudio.image-generation' or data.get('result',{}).get('ok') is not True:
            raise ValueError('generation_receipt is not a successful native image generation')
        if Path(data['project']).resolve().parent!=project_root:
            raise ValueError('generation_receipt belongs to another project folder')
        artwork=Path(data['image']).resolve(strict=True)
        if not artwork.is_relative_to(project_root): raise ValueError('Generated artwork is outside this project')
        digest=hashlib.sha256(artwork.read_bytes()).hexdigest()
        if digest!=data['result']['sha256']: raise ValueError('Generated artwork changed after its receipt; review the new version before placement')
        generation={'receipt':str(receipt),'asset_id':data['result']['asset_id'],'image':str(artwork),
            'sha256':digest,'poses':data.get('poses',[]),'animation_context':data.get('animation_context',{}),
            'relationship':'declared source artwork; video rendering is a separate step'}
    if source.suffix.lower() not in {'.webm','.mp4','.mov'}: raise ValueError('Expected a video track')
    start=float(request.get('start',existing_record.get('start',0)))
    duration=float(request.get('duration',existing_record.get('duration',1)))
    rect=request.get('rect',existing_record.get('rect',[0,0,1920,1080]))
    if start<0 or duration<=0 or len(rect)!=4 or not all(math.isfinite(float(v)) for v in [start,duration,*rect]) or rect[2]<=0 or rect[3]<=0: raise ValueError('Invalid timing or placement')
    playback_rate=float(request.get('playback_rate',existing_record.get('playback_rate',1)))
    if not math.isfinite(playback_rate) or not 0<playback_rate<=16: raise ValueError('playback_rate must be in (0,16]')
    text=entry.read_text(); root_tag=re.search(r'<[^>]+\bdata-composition-id\s*=\s*["\'][^"\']+["\'][^>]*>',text)
    if not root_tag:
        body_tag=re.search(r'<body[^>]*>',text,re.I)
        if body_tag:
            at=body_tag.end()
            text=text[:at]+'\n<div data-composition-id="main">'
            close_body=re.search(r'</body>',text,re.I)
            text=(text[:close_body.start()]+'</div>\n'+text[close_body.start():]) if close_body else (text+'\n</div>')
            root_tag=re.search(r'<[^>]+\bdata-composition-id\s*=\s*["\'][^"\']+["\'][^>]*>',text)
    if not root_tag: raise ValueError('Composition root missing')
    digest=hashlib.sha256(source.read_bytes()).hexdigest()[:12]
    target=folder/f'{key}-{digest}{source.suffix}'
    if source!=target: shutil.copy2(source,target)
    existing=re.compile(r'<video\b(?=[^>]*\bid=["\']'+re.escape(key)+r'["\'])[^>]*>\s*</video>',re.I)
    text=existing.sub('',text)
    track=max([int(v) for v in re.findall(r'data-track-index=["\'](\d+)',text)]+[-1])+1
    x,y,w,h=rect
    char_val=request.get('character',existing_record.get('character',''))
    track_val=request.get('track',existing_record.get('track',''))
    scene_val=request.get('scene',existing_record.get('scene',''))
    character=html.escape(str(char_val),quote=True)
    motion_track=html.escape(str(track_val),quote=True)
    tag=f'<video id="{key}" class="clip" src="{html.escape(str(target.relative_to(root)))}" data-start="{start}" data-duration="{duration}" data-playback-rate="{playback_rate}" data-track-index="{track}" data-magicstudio-scene="{scene_val}" data-magicstudio-character="{character}" data-magicstudio-track="{motion_track}" muted playsinline style="position:absolute;left:{x}px;top:{y}px;width:{w}px;height:{h}px;object-fit:contain;z-index:10"></video>'
    root_tag=re.search(r'<[^>]+\bdata-composition-id\s*=\s*["\'][^"\']+["\'][^>]*>',text)
    backup=root/'.magicstudio-history';backup.mkdir(exist_ok=True);stamp=time.time_ns();shutil.copy2(entry,backup/f'index-{stamp}.html')
    # Stack in placement order: a new placement goes after the last placed track, so it draws on top
    # of everything placed before it (equal z-index; later in the page wins).
    placed=list(re.finditer(r'<video\b[^>]*\bdata-magicstudio-scene=[^>]*>\s*</video>',text))
    at=placed[-1].end() if placed else root_tag.end()
    text=text[:at]+'\n'+tag+text[at:]
    entry.write_text(text)
    merged=dict(existing_record,**request)
    merged.update(ok=True,id=key,start=start,duration=duration,rect=rect,playback_rate=playback_rate,character=char_val,track=track_val,media=str(target),path=str(target),track_index=track,backup=str(backup/f'index-{stamp}.html'))
    if generation is not None: merged['source_generation']=generation
    (folder/f'{key}.json').write_text(json.dumps(merged,indent=2));return merged

if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--project',required=True);p.add_argument('--request',required=True);a=p.parse_args();print(json.dumps(place(a.project,json.loads(a.request))))
