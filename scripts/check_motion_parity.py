"""Compare the engine samples to the exporter's authoritative channels on a supplied project.
Usage: python3 scripts/check_motion_parity.py <project.magic> <spec.json> <character>
"""
import json,sys,subprocess
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
from pipeline.layered_motion import channel,oscillate_at
project,spec_path,character=sys.argv[1:]
raw=json.loads(Path(spec_path).read_text()); spec={**raw,'layers':{l['id']:l for l in raw['layers']}}
cmds=[{'id':'project.open','params':{'path':project}}]+[{'id':'motion.sample','params':{'character':character,'track':t['id'],'fps':raw['fps']}} for t in raw['tracks']]
r=subprocess.run(['magic','run','-'],input='\n'.join(map(json.dumps,cmds)),capture_output=True,text=True,check=True)
results=[json.loads(x)['result'] for x in r.stdout.splitlines()][1:]
checked=0;worst=0
for track,result in zip(raw['tracks'],results):
 for li,layer in enumerate(raw['layers']):
  for prop,size in [('position',2),('rotation',1),('scale',2),('opacity',1)]:
   keys,_=channel(track,spec,layer['id'],prop,raw['fps'],track['duration'])
   terms=track.get('motion',{}).get(layer['id'],{}).get(prop,[])
   for fi,frame in enumerate(result['frames']):
    base=keys[min(fi,len(keys)-1)][1] if keys else [0]*size
    expected=list(base)
    for term in terms:
     if term['kind']=='oscillate':expected=[a+b for a,b in zip(expected,oscillate_at(term,frame['time'],size))]
    if prop=='scale':expected=[(100+v)/100 for v in expected]
    if prop=='opacity':expected=[max(0,min(100,100+expected[0]))/100 if not track.get('layers') or layer['id'] in track['layers'] else 0]
    actual=frame['layers'][li][prop];actual=actual if isinstance(actual,list) else [actual]
    error=max(abs(a-b) for a,b in zip(actual,expected));worst=max(worst,error);checked+=size
    assert error<0.0001,(track['id'],layer['id'],prop,fi,actual,expected,error)
print(f'PASS: {len(results)} tracks, {checked} channel values; max error {worst:.9g}')
