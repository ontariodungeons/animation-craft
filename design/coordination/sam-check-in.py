import json, subprocess
r = subprocess.run(['herdr','agent','get','sam'], capture_output=True, text=True, timeout=10)
if r.returncode: raise SystemExit(0)
a = json.loads(r.stdout)['result']['agent']
if a.get('agent_session',{}).get('value') != 'ses_e96a8ee4dffeym2K54594l15kc': raise SystemExit(0)
if a.get('agent_status') not in ('idle','done'): raise SystemExit(0)
subprocess.run(['herdr','agent','prompt','sam','Scheduled bounded commit/handoff check-in: follow your steward assignment. Collect only new deltas, ask idle owners only, no repeated polling or waiting for everyone. Gonzo via issue24 only. Report once to Fozzie, update ledger if changed, then stop. No tests/builds or Git mutations.'], timeout=15, check=True)
