import json,subprocess,time,os,sys
from pathlib import Path
root=Path(__file__).parent
mc=[str(root.parents[2]/'.worktrees/ambient-companions/target/debug/mc'),'--bridge','19931','exec']
def call(cmd,*args):return json.loads(subprocess.check_output(mc+[cmd,*args],text=True))
def cpu(pid):
 pids={pid};records={}
 for p in Path('/proc').iterdir():
  if not p.name.isdigit():continue
  try:
   fields=(p/'stat').read_text().rsplit(')',1)[1].split();records[int(p.name)]=(int(fields[1]),int(fields[11])+int(fields[12]))
  except (OSError,ValueError,IndexError):pass
 while True:
  more={p for p,(parent,_) in records.items() if parent in pids}
  if more<=pids:break
  pids|=more
 return {p:records[p][1] for p in pids if p in records}
label=sys.argv[1];seconds=float(sys.argv[2]);pid=call('ui.feedback')['identity']['pid']
call('ui.eval','js=window.measureObserver?.disconnect();window.measureMutations={all:0,companions:0};window.measureObserver=new MutationObserver(records=>{for(const r of records){measureMutations.all++;if(r.target.parentElement?.closest(".ambient-companion")||r.target.matches?.(".ambient-companion"))measureMutations.companions++;}});measureObserver.observe(document.body,{subtree:true,childList:true,characterData:true,attributes:true});return true;')
a=cpu(pid);started=time.monotonic()
if len(sys.argv)>3 and sys.argv[3]=='save':call('project.save')
time.sleep(seconds);b=cpu(pid);elapsed=time.monotonic()-started
state=call('ui.eval','js=measureObserver.disconnect();return {mutations:measureMutations,companions:window.magicCompanions?.snapshot()||null};')
ticks=sum(max(0,b[p]-a.get(p,b[p])) for p in b);result={'label':label,'seconds':elapsed,'process_tree_cpu_seconds':ticks/os.sysconf('SC_CLK_TCK'),'one_core_cpu_percent':100*ticks/os.sysconf('SC_CLK_TCK')/elapsed,'pids':list(b),'dom':state}
(root/(label+'.json')).write_text(json.dumps(result,indent=2));print(json.dumps(result,indent=2))
