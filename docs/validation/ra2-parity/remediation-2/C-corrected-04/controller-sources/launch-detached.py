"""Launch a trusted validation receipt controller independently of tool-session lifetime."""
from pathlib import Path
import datetime,hashlib,json,os,subprocess,sys
base=Path(__file__).resolve().parent
runner=Path(sys.argv[1]).resolve();args=sys.argv[2:]
assert runner.parent==base and runner.is_file()
name='detached-'+datetime.datetime.now(datetime.timezone.utc).strftime('%Y%m%dT%H%M%S%fZ')
raw=base/(name+'.log');receipt=base/(name+'.json')
assert not raw.exists() and not receipt.exists()
with raw.open('xb') as out:
 process=subprocess.Popen([sys.executable,str(runner),*args],cwd='/Users/xingyuwang/GameProjects/MyGame001',env=os.environ.copy(),stdin=subprocess.DEVNULL,stdout=out,stderr=subprocess.STDOUT,start_new_session=True,close_fds=True)
record={'utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'pid':process.pid,'controller_pgid':process.pid,'runner':str(runner),'runner_sha256':hashlib.sha256(runner.read_bytes()).hexdigest(),'args':args,'stdout':str(raw),'scope':'Durable detached receipt controller; actual native child exit/commands and source guards remain recorded by controller. Launcher exit is not a validation result.'}
receipt.write_text(json.dumps(record,indent=2)+'\n');print(json.dumps(record))
