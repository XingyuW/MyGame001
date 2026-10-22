from pathlib import Path
import subprocess,hashlib,re,json,collections
ROOT=Path('/Users/xingyuwang/GameProjects/MyGame001');OUT=Path(__file__).parent
comments=[];originals=[]
for name in ('rules.ini','art.ini'):
 b=(Path.home()/'Library/Caches/MyGame001/ra2-reference'/name).read_bytes();originals.append((name,hashlib.sha1(b'blob '+str(len(b)).encode()+b'\0'+b).hexdigest()))
 for n,line in enumerate(b.decode('latin1').splitlines(),1):
  stripped=line.lstrip(';').strip()
  if line.startswith(';') and len(stripped)>=65 and len(stripped.split())>=8 and not set(stripped)<=set('-=* '):comments.append((name,n,stripped.encode()))
objects=subprocess.check_output(['git','rev-list','--objects','8230a955b251e9341d349f99eaeb86f7f8253664..23647de4220408d42eeef6522deb4d4541528452'],cwd=ROOT).decode().splitlines();names={line.partition(' ')[0]:line.partition(' ')[2] for line in objects};oids=set(names)
check=subprocess.check_output(['git','cat-file','--batch-check=%(objectname) %(objecttype) %(objectsize)'],input=('\n'.join(names)+'\n').encode(),cwd=ROOT).decode().splitlines();selected=[];large=[]
for line in check:
 oid,kind,size=line.split();name=names[oid]
 if kind!='blob' or Path(name).suffix not in ('.md','.rs','.py','.ron','.tsv','.txt','.log','.sh','.json'):continue
 if int(size)>5_000_000:large.append((oid,name,int(size)));continue
 selected.append((oid,name,int(size)))
# Prefix regex is merely an index; a hit requires the entire original comment line verbatim.
prefixes=collections.defaultdict(list)
for source,n,text in comments:prefixes[text[:40]].append((source,n,text))
pattern=re.compile(b'|'.join(re.escape(x) for x in prefixes))
p=subprocess.Popen(['git','cat-file','--batch'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,cwd=ROOT)
hits=[];bytecount=0
for oid,name,size in selected:
 p.stdin.write((oid+'\n').encode());p.stdin.flush();header=p.stdout.readline();actual=p.stdout.read(size);assert p.stdout.read(1)==b'\n';bytecount+=size
 for match in pattern.finditer(actual):
  for source,n,text in prefixes[match.group()]:
   if actual[match.start():].startswith(text):hits.append({'object':oid,'path':name,'source':source,'source_line':n,'comment_sha256':hashlib.sha256(text).hexdigest(),'repository_line':actual[:match.start()].count(b'\n')+1})
p.stdin.close();p.wait()
result={'cumulative_objects':len(oids),'original_blobs_present':[(name,oid in oids) for name,oid in originals],'forbidden_formats':[name for name in names.values() if re.search(r'\.(ini|mix|csf|shp|vxl|hva|aud)$',name,re.I)],'long_source_comments':len(comments),'text_blobs_scanned':len(selected),'bytes_scanned':bytecount,'unscanned_large_text_blobs':large,'verbatim_comment_hits':hits,'scope_limit':'This checks distinctive long original INI comments and whole original file identities/formats. It does not assert that every short generic unit/building name is copyrighted source prose, nor authenticate media pixels or all possible paraphrases.'}
(OUT/'data-provenance-history.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
