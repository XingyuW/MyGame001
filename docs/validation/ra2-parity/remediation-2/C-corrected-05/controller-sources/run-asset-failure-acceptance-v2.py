"""Actual release and packaged processes: missing tileset -> exit1; restored ->600-frame pass."""
from pathlib import Path
import datetime,hashlib,json,os,signal,subprocess,sys,time
root=Path('/Users/xingyuwang/GameProjects/MyGame001');os.chdir(root)
tip,label=sys.argv[1:3];assert subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip()==tip
cache=Path(__file__).resolve().parent
out=root/'docs/validation/ra2-parity'/label;assert not out.exists();out.mkdir(parents=True)
overlay=cache/(label+'-asset-root');assert not overlay.exists();assets=overlay/'assets';assets.mkdir(parents=True)
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
def utc():return datetime.datetime.now(datetime.timezone.utc).isoformat()
# Independent directory entries, no clone/copy of assets; only the one owned symlink changes.
for source in (root/'assets').iterdir():
 if source.name!='maps':(assets/source.name).symlink_to(source,target_is_directory=source.is_dir())
(assets/'maps').mkdir()
for source in (root/'assets/maps').iterdir():
 if source.name!='demo':(assets/'maps'/source.name).symlink_to(source,target_is_directory=source.is_dir())
(assets/'maps/demo').mkdir()
for source in (root/'assets/maps/demo').iterdir():(assets/'maps/demo'/source.name).symlink_to(source,target_is_directory=source.is_dir())
image=assets/'maps/demo/demo_tiles.png';assert image.is_symlink()
original=root/'assets/maps/demo/demo_tiles.png';original_hash=sha(original)
target=Path(os.environ['CARGO_TARGET_DIR']);executables=[('release',target/'release/game'),('packaged',target/'package/MyGame001.app/Contents/MacOS/MyGame001')]
state={'status':'RUNNING','start':utc(),'tip':tip,'runner_sha256':sha(Path(__file__)),'overlay':str(overlay),'overlay_scope':'Symlinks to original assets; only one owned demo image symlink is removed/restored. Canonical/package assets remain unchanged.','original_image':{'path':str(original),'sha256':original_hash},'commands':[]};receipt=out/'process.json'
def save():
 p=receipt.with_suffix('.next');p.write_text(json.dumps(state,indent=2)+'\n');p.replace(receipt)
def invoke(kind,exe,missing):
 assert subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip()==tip
 assert sha(original)==original_hash
 raw=out/(kind+('-missing-image.log' if missing else '-restored-image.log'))
 env=os.environ.copy();env['BEVY_ASSET_ROOT']=str(overlay);env.pop('CARGO_MANIFEST_DIR',None)
 cmd=[str(exe),'--smoke-test','600'];entry={'command':cmd,'cwd':str(cache),'env':{'BEVY_ASSET_ROOT':str(overlay),'CARGO_MANIFEST_DIR':'UNSET'},'executable_sha256':sha(exe),'start':utc(),'expected_exit':1 if missing else 0,'image_present':image.exists(),'raw':str(raw.relative_to(root))};state['commands'].append(entry);save()
 with raw.open('xb') as log:
  p=subprocess.Popen(cmd,cwd=cache,env=env,stdin=subprocess.DEVNULL,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
  entry['pid']=p.pid;entry['pgid']=p.pid;save()
  try:entry['exit']=p.wait(timeout=90 if missing else 300)
  except subprocess.TimeoutExpired:
   entry['timeout']=True;os.killpg(p.pid,signal.SIGTERM)
   try:entry['exit']=p.wait(timeout=10)
   except subprocess.TimeoutExpired:os.killpg(p.pid,signal.SIGKILL);entry['exit']=p.wait()
 entry['end']=utc();entry['raw_bytes']=raw.stat().st_size;entry['raw_sha256']=sha(raw)
 text=raw.read_text(errors='replace');entry['missing_image_context']=('demo_tiles.png' in text and ('cannot' in text.lower() or 'error' in text.lower()))
 entry['pass']=entry['exit']==entry['expected_exit'] and not entry.get('timeout') and (entry['missing_image_context'] if missing else ('600 frames' in text and 'map created: true' in text and 'smoke test failed' not in text))
 save();assert entry['pass'],entry
awake=subprocess.Popen(['caffeinate','-d','-i','-w',str(os.getpid())],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
state['caffeinate_pid']=awake.pid
save()
try:
 for kind,exe in executables:
  assert exe.is_file()
  image.unlink();invoke(kind,exe,True)
  image.symlink_to(original);invoke(kind,exe,False)
 state['status']='PASSED'
except BaseException as error:state['status']='FAILED';state['error']=repr(error)
finally:
 awake.terminate();awake.wait()
 if not image.exists() and not image.is_symlink():image.symlink_to(original)
 state['end']=utc();state['canonical_image_unchanged']=sha(original)==original_hash;state['overlay_restored']=image.is_symlink() and image.resolve()==original.resolve();save()
print(state['status']);sys.exit(0 if state['status']=='PASSED' else 1)
