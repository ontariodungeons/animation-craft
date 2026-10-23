"""Bounded MLT cuts/audio assembly. HyperFrames remains the film compositor.

Input is a small declarative JSON timeline, not arbitrary MLT XML or shell code.
The worker freezes referenced files and writes an editable native .mlt document.
"""
import hashlib
import json
import math
import os
from pathlib import Path
import re
import shutil
import subprocess
import xml.etree.ElementTree as ET
from fractions import Fraction


def sha(path):
    with Path(path).open('rb') as handle:
        return hashlib.file_digest(handle,'sha256').hexdigest()


def integer(value,low,high,label):
    if type(value) is not int or not low<=value<=high: raise ValueError(f'{label} must be integer {low}..{high}')
    return value


def validate_timeline(doc):
    if not isinstance(doc,dict) or set(doc)-{'width','height','fps','clips','audio','audio_mode'}:
        raise ValueError('Unsupported timeline fields')
    width=integer(doc.get('width'),2,4096,'width');height=integer(doc.get('height'),2,4096,'height')
    if width%2 or height%2: raise ValueError('H264 output requires even width/height')
    rate=doc.get('fps')
    if not isinstance(rate,list) or len(rate)!=2: raise ValueError('fps must be [numerator,denominator]')
    numerator=integer(rate[0],1,120000,'fps numerator');denominator=integer(rate[1],1,10000,'fps denominator')
    fps=numerator/denominator
    if not 1<=fps<=120: raise ValueError('fps must be 1..120')
    clips=doc.get('clips')
    if not isinstance(clips,list) or not 1<=len(clips)<=256: raise ValueError('clips must have 1..256 entries')
    count=0
    for clip in clips:
        if not isinstance(clip,dict) or set(clip)-{'path','in','out'} or not isinstance(clip.get('path'),str): raise ValueError('Clip requires local path, in, out only')
        begin=integer(clip.get('in'),0,18000000,'clip in');end=integer(clip.get('out'),begin,18000000,'clip out');count+=end-begin+1
    if count>18000 or width*height*count>256*1024*1024: raise ValueError('Timeline exceeds output frame/pixel budget')
    audio=doc.get('audio')
    if audio is not None:
        if doc.get('audio_mode')!='replace': raise ValueError('Separate audio requires explicit audio_mode=replace; embedded clip audio will be replaced')
        if not isinstance(audio,dict) or set(audio)-{'path','in','out','start'} or not isinstance(audio.get('path'),str): raise ValueError('Audio requires path/in/out/start only')
        begin=integer(audio.get('in'),0,18000000,'audio in');end=integer(audio.get('out'),begin,18000000,'audio out');start=integer(audio.get('start',0),0,count-1,'audio start')
        if start+end-begin+1>count: raise ValueError('Audio extends beyond the video timeline')
    elif doc.get('audio_mode','embedded')!='embedded': raise ValueError('Without separate audio use audio_mode=embedded')
    return width,height,numerator,denominator,count


def probe(path, executable='ffprobe', count=False):
    args=[executable,'-v','error']
    if count: args+=['-count_frames']
    return json.loads(subprocess.check_output(args+['-show_streams','-show_format','-of','json',str(path)],text=True))


def build_xml(doc, paths, width, height, numerator, denominator, count):
    root=ET.Element('mlt',producer='main')
    ET.SubElement(root,'profile',description='Magic Studio assembly',width=str(width),height=str(height),
                  frame_rate_num=str(numerator),frame_rate_den=str(denominator),progressive='1',
                  sample_aspect_num='1',sample_aspect_den='1',display_aspect_num=str(width),display_aspect_den=str(height),colorspace='709')
    def prop(node,key,value): ET.SubElement(node,'property',name=key).text=str(value)
    for i,clip in enumerate(doc['clips']):
        producer=ET.SubElement(root,'producer',id=f'video{i}')
        prop(producer,'mlt_service','avformat');prop(producer,'resource',paths[i]);prop(producer,'threads',2)
        if doc.get('audio') is not None: prop(producer,'audio_index',-1)
    audio=doc.get('audio')
    if audio is not None:
        producer=ET.SubElement(root,'producer',id='sound')
        prop(producer,'mlt_service','avformat');prop(producer,'resource',paths[-1]);prop(producer,'video_index',-1)
    playlist=ET.SubElement(root,'playlist',id='picture')
    for i,clip in enumerate(doc['clips']): ET.SubElement(playlist,'entry',producer=f'video{i}',attrib={'in':str(clip['in']),'out':str(clip['out'])})
    if audio is not None:
        playlist=ET.SubElement(root,'playlist',id='soundtrack')
        if audio.get('start',0): ET.SubElement(playlist,'blank',length=str(audio['start']))
        ET.SubElement(playlist,'entry',producer='sound',attrib={'in':str(audio['in']),'out':str(audio['out'])})
    tractor=ET.SubElement(root,'tractor',id='main',attrib={'in':'0','out':str(count-1)})
    tracks=ET.SubElement(tractor,'multitrack')
    ET.SubElement(tracks,'track',producer='picture',**({'hide':'audio'} if audio is not None else {}))
    if audio is not None:
        ET.SubElement(tracks,'track',producer='soundtrack',hide='video')
        transition=ET.SubElement(tractor,'transition',attrib={'in':'0','out':str(count-1)})
        for key,value in [('mlt_service','mix'),('a_track',0),('b_track',1),('always_active',1),('sum',1),('start',1)]: prop(transition,key,value)
    return ET.tostring(root,encoding='unicode')


def render(request,job):
    from worker import write_json
    source=Path(request['timeline']).resolve()
    with source.open('rb') as handle: raw=handle.read(1024*1024+1)
    if len(raw)>1024*1024: raise ValueError('Timeline exceeds 1 MiB')
    doc=json.loads(raw);width,height,num,den,count=validate_timeline(doc);fps=num/den
    frozen=job/'source-bundle';frozen.mkdir();(frozen/'original-timeline.json').write_bytes(raw)
    entries=doc['clips']+([doc['audio']] if doc.get('audio') is not None else [])
    paths=[];manifest=[];bytes_total=0
    for i,entry in enumerate(entries):
        path=Path(entry['path']);path=(source.parent/path).resolve() if not path.is_absolute() else path.resolve()
        if not path.is_file(): raise ValueError('Missing local source media: '+str(path))
        bytes_total+=path.stat().st_size
        if bytes_total>1024**3: raise ValueError('Source media exceeds 1 GiB')
        out=frozen/f'media-{i:03d}{path.suffix.lower()}';shutil.copyfile(path,out)
        digest=sha(out)
        if sha(path)!=digest: raise ValueError('Media changed while freezing')
        info=probe(out,request['ffprobe']);streams=info.get('streams',[])
        video=i<len(doc['clips']);kind='video' if video else 'audio';stream=next((s for s in streams if s.get('codec_type')==kind),None)
        if not stream: raise ValueError('Source has no '+kind+' stream')
        duration=float(stream.get('duration',info.get('format',{}).get('duration','nan')))
        if not math.isfinite(duration) or (entry['out']+1)/fps>duration+0.001: raise ValueError('Source trim exceeds measured media duration')
        if video:
            rate=float(Fraction(stream.get('r_frame_rate','0/1')))
            if abs(rate-fps)>fps*1e-5: raise ValueError('Input video FPS must match timeline FPS; rate conversion is not supported')
        paths.append(str(out));manifest.append({'path':str(out),'original_path':str(path),'sha256':digest,'probe':info})
    xml=job/'timeline.mlt';xml.write_text(build_xml(doc,paths,width,height,num,den,count))
    frozen_doc=json.loads(raw)
    for entry,path in zip(frozen_doc['clips']+([frozen_doc['audio']] if frozen_doc.get('audio') is not None else []),paths): entry['path']=path
    write_json(frozen/'timeline.json',frozen_doc)
    profile=job/'profile';profile.write_text('\n'.join(f'{key}={value}' for key,value in {
        'description':'Magic Studio assembly','frame_rate_num':num,'frame_rate_den':den,'width':width,'height':height,
        'progressive':1,'sample_aspect_num':1,'sample_aspect_den':1,'display_aspect_num':width,'display_aspect_den':height,'colorspace':709}.items())+'\n')
    env=dict(os.environ,QT_QPA_PLATFORM='offscreen',SDL_VIDEODRIVER='dummy')
    query=subprocess.run([request['melt'],'-query','consumers'],capture_output=True,text=True,env=env,timeout=15)
    diagnostics=query.stdout+'\n'+query.stderr
    if not re.search(r'^\s*- avformat\s*$',diagnostics,re.M): raise ValueError('Required MLT avformat consumer unavailable: '+diagnostics[-3000:])
    (job/'mlt-discovery.log').write_text(diagnostics)
    version=subprocess.run([request['melt'],'--version'],capture_output=True,text=True,env=env,timeout=15)
    output=job/'assembly.mp4'
    argv=[request['melt'],'-silent','-profile',str(profile),str(xml),'-consumer','avformat:'+str(output),
          'vcodec=libx264','acodec=aac','pix_fmt=yuv420p','crf=18','preset=fast','threads=2','real_time=-2']
    write_json(job/'plan.json',{'argv':argv,'timeline':doc,'source_bundle':manifest})
    write_json(job/'progress.json',{'state':'rendering','completed':0,'total':count})
    subprocess.run(argv,env=env,check=True)
    write_json(job/'progress.json',{'state':'encoding','detail':'validating','completed':count,'total':count})
    info=probe(output,request['ffprobe'],True);video=next((s for s in info['streams'] if s['codec_type']=='video'),None)
    if not video or int(video.get('nb_read_frames',0))!=count or video['width']!=width or video['height']!=height or abs(float(Fraction(video['r_frame_rate']))-fps)>1e-6:
        raise ValueError('MLT output frame count/size/rate mismatch')
    measured_seconds=float(info.get('format',{}).get('duration','nan'))
    if not math.isfinite(measured_seconds) or abs(measured_seconds-count/fps)>1/fps: raise ValueError('Encoded duration differs from requested timeline')
    if doc.get('audio') is not None and not any(s['codec_type']=='audio' for s in info['streams']): raise ValueError('MLT output lost the requested audio stream')
    # Decode both streams, not just inspect a successful encoder exit or file header.
    subprocess.run([request['ffmpeg'],'-v','error','-xerror','-i',str(output),'-f','null','-'],check=True)
    if any(sha(v['path'])!=v['sha256'] for v in manifest): raise ValueError('Frozen media changed during assembly')
    return {'backend':'mlt','state':'completed','clip':str(output),'clip_sha256':sha(output),'clip_seconds':count/fps,
            'fps':fps,'fps_fraction':[num,den],'first_frame':0,'last_frame':count-1,'width':width,'height':height,
            'canvas':[width,height],'world_rect':[0,0,width,height],'partial':False,'frames_total':count,
            'source_bundle':{'path':str(frozen/'timeline.json'),'bundle':str(frozen),'original_path':str(source),'sha256':sha(frozen/'timeline.json'),'dependencies':manifest},
            'native_project':str(xml),'native_project_sha256':sha(xml),'tool':{'path':request['melt'],'sha256':sha(request['melt']),'version':(version.stdout+version.stderr).strip(),'version_exit_code':version.returncode},
            'audio_mode':doc.get('audio_mode','embedded'),'probe':info,'alpha_evidence':{'channel_present':False,'mode':'opaque H264 output'},
            'diagnostics':str(job/'mlt-discovery.log'),'source_job':request['job']}
