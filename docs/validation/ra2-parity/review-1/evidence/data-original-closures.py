from pathlib import Path
import json,csv,re,ast,hashlib,collections
ROOT=Path('/Users/xingyuwang/GameProjects/MyGame001');OUT=Path(__file__).parent;RF=ROOT/'crates/sim/tests/reference';IN=Path.home()/'Library/Caches/MyGame001/ra2-reference/rules.ini'
# Separate parser retaining source spelling and last-key/merged-section semantics.
d={};sec=None
for line in IN.read_text(encoding='latin1').splitlines():
 line=line.partition(';')[0].strip()
 if line.startswith('[') and ']' in line:
  name=line[1:line.index(']')].strip();sec=d.setdefault(name.lower(),[name,{}])[1]
 elif sec is not None and '=' in line:
  k,_,v=line.partition('=');sec[k.strip().lower()]=(k.strip(),re.sub(r'\s*,\s*',',',v.strip()))
def get(s,k):
 t=d.get(s.lower());x=t[1].get(k.lower()) if t else None;return x[1] if x else None
allrows=[(name,k,v) for name,pairs in d.values() for k,v in pairs.values()]
# Configured query identities are allowed inputs; all observed values/graphs come from actual INI.
tree=ast.parse((ROOT/'scripts/ra9_scope.py').read_text());cfg={}
for n in tree.body:
 if isinstance(n,ast.Assign):
  for target in n.targets:
   if isinstance(target,ast.Name) and target.id in ('TARGETS','SPECIAL_QUERIES','REGISTRIES'):cfg[target.id]=ast.literal_eval(n.value)
out=[]
for group,targets in cfg['TARGETS'].items():
 out.extend([(group,'universe','rules','keys',str(len(allrows))),(group,'universe','rules','sections',str(len(d)))])
 for target in targets:
  out.append((group,'section',target,'present',str(target.lower() in d).lower()))
  members=[]
  for registry in cfg['REGISTRIES']:
   for key,val in d.get(registry.lower(),['',{}])[1].values():
    if val==target:members.append(registry+':'+key)
  out.append((group,'registry',target,'members',','.join(sorted(members))))
  out.extend((group,'incoming',s,k,v) for s,k,v in allrows if target in v.split(','))
 if group in ('gems','ore-growthrate'):
  for section in ('Riparius','Cruentus'):
   for key in ('Growth','GrowthPercentage','Spread','SpreadPercentage'):out.append((group,'ore-primary',section,key,get(section,key)))
 if group in ('inactive-powerup','upgrades'):
  participants=[(s,k,v) for s,k,v in allrows if k.lower().startswith('powersup')];out.append((group,'upgrade-universe','rules','participants',str(len({s for s,_,_ in participants}))))
  out.extend((group,'upgrade-participant',s,k,v) for s,k,v in participants)
 if group=='radiation-typo':out.append((group,'correct-key','NANRCT','ImmuneToRadiation',get('NANRCT','ImmuneToRadiation') or '<ABSENT>'))
text='group\tfact_kind\tsection_or_target\tkey\tvalue\n'+''.join('\t'.join(r)+'\n' for r in sorted(out));result={'guard75':{'facts':len(out),'byte_equal':text==(RF/'ra9_scope75_original.tsv').read_text()}}
special=[]
for owner,section,key in cfg['SPECIAL_QUERIES']:
 v=get(section,key);special.append((owner,section,key,'absent' if v is None else 'present','' if v is None else v))
special.extend(('vhp-key-census',s,k,'present',v) for s,k,v in allrows if k.lower()=='vhpscan')
text='owner\tsection\tkey\tpresence\tvalue\n'+''.join('\t'.join(r)+'\n' for r in sorted(special));result['special']={'facts':len(special),'byte_equal':text==(RF/'ra9_special_original.tsv').read_text()}
targets=json.loads((ROOT/'scripts/ra9_legacy_original_targets.json').read_text());actual=[]
for family,sections in sorted(targets.items()):
 facts=[]
 for target in sections:
  found=d.get(target.lower());facts.append(('section',target,'present',str(found is not None).lower()))
  if found:facts.extend(('own',found[0],k,v) for k,v in found[1].values())
  facts.extend(('incoming:'+target,s,k,v) for s,k,v in allrows if target.lower() in [x.lower() for x in v.split(',')])
 if family=='F251':
  eligible=sorted(name for name,pairs in d.values() if pairs.get('eligiblefordelaykill',('', 'no'))[1].lower() in ('yes','true','1'))
  for name in eligible:
   facts.append(('eligible',name,'EligibleForDelayKill','yes'));facts.extend(('eligible-own',name,k,v) for k,v in d[name.lower()][1].values())
  result['F251_eligible']=eligible
 b=''.join('\t'.join(row)+'\n' for row in sorted(facts)).encode();actual.append((family,str(len(sections)),str(sum(x[0].startswith('incoming:') for x in facts)),hashlib.sha256(b).hexdigest()))
text='family_id\tsection_count\tincoming_count\tmetadata_closure_sha256\n'+''.join('\t'.join(row)+'\n' for row in actual)
result['legacy904']={'families':len(actual),'byte_equal':text==(RF/'ra9_legacy_original.tsv').read_text(),'F146_F147_F251':[r for r in actual if r[0] in ('F146','F147','F251')]}
(OUT/'data-original-closures.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
