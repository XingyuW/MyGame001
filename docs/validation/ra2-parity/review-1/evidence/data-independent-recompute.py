"""Independent reviewer data calculations; does not import implementation or audit modules."""
from pathlib import Path
from fractions import Fraction as F
import re,csv,json,hashlib,collections
ROOT=Path('/Users/xingyuwang/GameProjects/MyGame001'); OUT=Path(__file__).parent

def ini(p):
 d={};section=None
 for line in p.read_text(encoding='latin1').splitlines():
  line=line.split(';')[0].strip()
  if line.startswith('[') and ']' in line:
   section=line[1:line.index(']')].strip().lower();d.setdefault(section,{})
  elif section and '=' in line:
   k,v=line.split('=',1);d[section][k.strip().lower()]=v.strip()
 return d
ref=Path.home()/'Library/Caches/MyGame001/ra2-reference'
R=ini(ref/'rules.ini'); A=ini(ref/'art.ini')
def get(s,k,default=None):return R.get(s.lower(),{}).get(k.lower(),default)
def q(v):return F(str(v).rstrip('%'))
def rnd(x):return (2*x.numerator+x.denominator)//(2*x.denominator) if x>=0 else -rnd(-x)
def val(s,k,default=0):return q(get(s,k,default))
def flag(s,k,default='no'):return int(get(s,k,default).lower() in ('yes','true','1'))

def ron(p):
 text=p.read_text(); toks=re.findall(r'"(?:\\.|[^"\\])*"|//[^\n]*|[A-Za-z_][A-Za-z0-9_]*|-?\d+|[^\s]',text)
 toks=[t for t in toks if not t.startswith('//')]; at=0
 def read():
  nonlocal at
  t=toks[at];at+=1
  if t=='(':
   out={}
   while toks[at]!=')':
    k=toks[at];assert toks[at+1]==':';at+=2;assert k not in out;out[k]=read()
    if toks[at]==',':at+=1
   at+=1;return out
  if t=='[':
   out=[]
   while toks[at]!=']':
    out.append(read())
    if toks[at]==',':at+=1
   at+=1;return out
  if t.startswith('"'):return json.loads(t)
  if re.fullmatch(r'-?\d+',t):return int(t)
  return t
 out=read();assert at==len(toks);return out
D={p.stem:ron(p) for p in (ROOT/'assets/data').glob('*.ron')}
def tsv(p):return list(csv.DictReader(p.open(),delimiter='\t'))
M=tsv(ROOT/'crates/sim/tests/reference/ra2_mapping.tsv'); C=tsv(ROOT/'crates/sim/tests/reference/ra2_combat.tsv')
checks=[]
def check(group,id,field,actual,expected,source):checks.append(dict(group=group,id=id,field=field,actual=actual,expected=expected,source=source,match=actual==expected))
U={x['id']:x for x in D['units']['units']}; E={x.get('id',x.get('unit')):x for kind in ('units','buildings') for x in D['economy'][kind]}; B={x.get('id',x.get('unit',x.get('building'))):x for kind in ('units','buildings') for x in D['combat'][kind]}; AR={x['id']:x for x in D['arsenal']['entries']}
inf=set(R['infantrytypes'].values())
for row in M:
 id,s,kind=row['game_id'],row['section'],row['kind']; group='building' if kind=='building' else 'unit';e=E[id];b=B[id];ar=AR[id]
 for field,key in [('cost','Cost')]:check(group,id,field,e[field],int(val(s,key)),s+'.'+key)
 wall=flag(s,'Wall')
 bt=max(1,rnd(val(s,'Cost')*val('General','BuildSpeed')*1200/1000));bt=max(1,rnd(bt*val('General','WallBuildSpeedCoefficient'))) if wall else bt
 check(group,id,'build_time',e['build_time'],bt,s+'.Cost*General.BuildSpeed'+('*WallBuildSpeedCoefficient' if wall else ''))
 for field,key in [('hit_points','Strength'),('sight','Sight')]:check(group,id,field,b[field],int(val(s,key)),s+'.'+key)
 if kind=='building':
  pw=int(val(s,'Power'));check(group,id,'power_produced',e['power_produced'],max(pw,0),s+'.Power');check(group,id,'power_consumed',e['power_consumed'],max(-pw,0),s+'.Power');check(group,id,'sell_value',e['sell_value'],int(val(s,'Cost')*val('General','RefundPercent')/100),s+'.Cost*RefundPercent')
  im=get(s,'Image',s);foundation=A.get(im.lower(),{}).get('foundation')
  if foundation and re.fullmatch(r'\d+x\d+',foundation):
   w,h=map(int,foundation.split('x'));check('footprint',id,'footprint.width',e['width'],w,im+'.Foundation='+foundation);check('footprint',id,'footprint.height',e['height'],h,im+'.Foundation='+foundation)
 else:
  jj=get(s,'JumpjetSpeed');lpf=int(q(jj)) if jj is not None else min(int(val(s,'Speed')*256/100),255)
  if jj is None and s in inf:lpf=lpf*3//2
  check('speed',id,'speed',U[id]['speed'],max(1,rnd(F(lpf*3,4))),s+'.JumpjetSpeed' if jj is not None else s+'.Speed')
 for field,key,default in [('size','Size',0 if kind=='building' else 1),('size_limit','SizeLimit',0),('ammo','Ammo',0),('docks','NumberOfDocks',0),('spawns_number','SpawnsNumber',0)]:
  if field=='ammo' and kind=='building':continue # building Ammo is explicitly unparsed; assess separately
  if field=='docks':
   if flag(s,'UnitRepair'):field='vehicle_docks'
   else:continue # airport docks require reverse AirportBound/Dock association, not this scalar recipe
  check('metadata',id,field,ar[field],int(val(s,key,default)),s+'.'+key)
 for field,key in [('spawn_regen','SpawnRegenRate'),('spawn_reload','SpawnReloadRate')]:check('spawns',id,field,ar[field],max(0,rnd(val(s,key)*F(4,3))) if get(s,key) else 0,s+'.'+key)
W={x['id']:x for x in D['combat']['weapons']};H={x['id']:x for x in D['combat']['warheads']}
for row in C:
 id,s,kind=row['game_id'],row['section'],row['kind']
 if kind=='weapon':
  w=W[id]
  for field,key,factor,div,default in [('damage','Damage',1,1,0),('range','Range',256,1,0),('min_range','MinimumRange',256,1,0),('reload','ROF',4,3,1),('burst','Burst',1,1,1)]:
   if field not in w:continue
   expect=rnd(val(s,key,default)*factor/div)
   if field=='reload':expect=max(1,expect)
   check('weapon',id,field,w[field],expect,s+'.'+key)
  p=get(s,'Projectile')
  for field,key,default in [('anti_air','AA','no'),('anti_ground','AG','yes'),('arcing','Arcing','no'),('inaccurate','Inaccurate','no')]:
   if field in w:check('projectile',id,field,w[field],flag(p,key,default),p+'.'+key)
 else:
  h=H[id]
  check('warhead',id,'cell_spread',h['cell_spread'],rnd(val(s,'CellSpread')*256),s+'.CellSpread')
  check('warhead',id,'percent_at_max',h['percent_at_max'],rnd(val(s,'PercentAtMax',1)*100),s+'.PercentAtMax')
  verses=get(s,'Verses',','.join(['100%']*11)).split(',')
  for n,v in enumerate(h['versus']):check('verses',id,str(n),v['percent'],int(q(verses[n])),s+'.Verses['+str(n)+']')
# Independent field/key recipe, not generated mappings.
recipes={
'production': [('build_ticks_per_1000','General','BuildSpeed',1200,1),('multiple_factory_pm','General','MultipleFactory',1000,1),('wall_build_time_pm','General','WallBuildSpeedCoefficient',1000,1),('low_power_penalty_pm','General','LowPowerPenaltyModifier',1000,1),('min_low_power_speed_pm','General','MinLowPowerProductionSpeed',1000,1),('max_low_power_speed_pm','General','MaxLowPowerProductionSpeed',1000,1),('max_queued_objects','General','MaximumQueuedObjects',1,1)],
'economy':[('refund_percent','General','RefundPercent',1,1),('repair_percent','General','RepairPercent',1,1),('repair_step','General','RepairStep',1,1),('repair_interval','General','RepairRate',1200,1),('unit_repair_interval','General','URepairRate',1200,1),('infantry_repair_step','General','IRepairStep',1,1),('infantry_repair_interval','General','IRepairRate',1200,1),('reload_interval','General','ReloadRate',1200,1),('purifier_bonus_percent','General','PurifierBonus',100,1),('ore_value','Riparius','Value',1,1),('gem_value','Cruentus','Value',1,1),('ore_growth_interval','Riparius','Growth',4,3),('ore_growth_pm','Riparius','GrowthPercentage',1000,1),('ore_spread_interval','Riparius','Spread',4,3),('ore_spread_pm','Riparius','SpreadPercentage',1000,1)],
'veterancy':[(f,'General',k,scale,1) for f,k,scale in [('veteran_ratio_pm','VeteranRatio',1000),('veteran_cap','VeteranCap',1),('combat_pm','VeteranCombat',1000),('speed_pm','VeteranSpeed',1000),('sight_pm','VeteranSight',1000),('armor_pm','VeteranArmor',1000),('rof_pm','VeteranROF',1000)]],
'crew':[('crew_escape_percent','General','CrewEscape',1,1),('survivor_rate_pm','General','SurvivorRate',1000,1)],
'radiation':[('application_delay','Radiation','RadApplicationDelay',4,3),('level_delay','Radiation','RadLevelDelay',4,3),('level_factor_pm','Radiation','RadLevelFactor',1000,1),('level_max','Radiation','RadLevelMax',1,1),('duration_multiple_pm','Radiation','RadDurationMultiple',1000,1)],
'superweapons':[('nuclear_recharge','MultiSpecial','RechargeTime',1200,1),('lightning_recharge','LightningStormSpecial','RechargeTime',1200,1),('iron_curtain_recharge','IronCurtainSpecial','RechargeTime',1200,1),('chronosphere_recharge','ChronoSphereSpecial','RechargeTime',1200,1),('paradrop_recharge','ParaDropSpecial','RechargeTime',1200,1),('american_paradrop_recharge','AmericanParaDropSpecial','RechargeTime',1200,1),('iron_curtain_duration','CombatDamage','IronCurtainDuration',4,3),('lightning_deferment','General','LightningDeferment',4,3),('lightning_duration','General','LightningStormDuration',4,3)],
'crates':[('maximum','CrateRules','CrateMaximum',1,1),('minimum','CrateRules','CrateMinimum',1,1),('radius','CrateRules','CrateRadius',256,1),('regen','CrateRules','CrateRegen',1200,1)]}
for g,rows in recipes.items():
 for f,s,k,n,d in rows:check('rules.'+g,g,f,D['rules'][g][f],rnd(val(s,k)*n/d),s+'.'+k)
for x in D['economy']['harvesters']:
 s=next(r['section'] for r in M if r['game_id']==x['unit']);check('ore',x['unit'],'capacity',x['capacity'],int(val(s,'Storage')),s+'.Storage')
for x in D['economy']['refunds']:
 s=next(r['section'] for r in M if r['game_id']==x['building']);check('ore',x['building'],'Soylent',x['credits'],int(val(s,'Soylent')),s+'.Soylent')
for p in D['rules']['powerups']:
 key={'armour':'Armor','heal_base':'HealBase'}.get(p['id'],p['id']);v=get('Powerups',key).split(',');check('crate',p['id'],'share',p['share'],int(v[0]),'Powerups.'+key+'[0]')
 if len(v)==4:check('crate',p['id'],'value',p['value'],rnd(q(v[3])*(1000 if p['id'] in ('armour','firepower','speed') else 1)),'Powerups.'+key+'[3]')
for difficulty,sec,i in [('easy','Difficult',2),('medium','Normal',1),('brutal','Easy',0)]:
 x=next(x for x in D['rules']['difficulty'] if x['id']==difficulty)
 for field,key in [('rof_pm','ROF'),('build_time_pm','BuildTime')]:check('difficulty',difficulty,field,x[field],rnd(val(sec,key)*1000),sec+'.'+key)
 for field,key,mul in [('credits_bonus_percent','MultiplayerAICM',F(1)),('virtual_purifiers','AIVirtualPurifiers',F(1)),('team_delay','TeamDelays',F(4,3)),('allied_base_defences','AlliedBaseDefenseCounts',F(1)),('soviet_base_defences','SovietBaseDefenseCounts',F(1))]:check('difficulty',difficulty,field,x[field],rnd(q(get('General',key).split(',')[i])*mul),'General.'+key+'['+str(i)+']')
# Compare extracted source rows independently with authenticated inputs.
source=tsv(ROOT/'crates/sim/tests/reference/ra2_reference.tsv');source_errors=[]
for x in source:
 raw=(A if x['file']=='art' else R).get(x['section'].lower(),{}).get(x['key'].lower());normal=lambda s:re.sub(r'\s*,\s*',',',s.strip())
 if raw is None or normal(raw)!=x['value']:source_errors.append((x,raw))
# Exact difference rows account for sampled footprint mismatches, not arbitrary waivers.
diffs=tsv(ROOT/'crates/sim/tests/reference/ra2_differences.tsv');unexplained=[]
for x in checks:
 if x['match']:continue
 reference_display=x['source'].split('=',1)[1] if x['group']=='footprint' else get(*x['source'].rsplit('.',1),'0') if x['group'] in ('weapon','projectile') else str(x['expected'])
 x['reference_display']=reference_display
 candidates=[d for d in diffs if d['game_id']==(('weapon.' if x['group'] in ('weapon','projectile') else '')+x['id']) and d['field']==x['field'] and d['game_value']==str(x['actual']) and d['reference_value']==reference_display]
 x['listed_difference']=len(candidates)>0
 if not candidates:unexplained.append(x)
summary={'checks':len(checks),'groups':dict(collections.Counter(x['group'] for x in checks)),'equal':sum(x['match'] for x in checks),'differences':sum(not x['match'] and x.get('listed_difference',False) for x in checks),'unexplained':unexplained,'source_rows':len(source),'source_errors':source_errors,'input_hashes':{p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in (ref/'rules.ini',ref/'art.ini')},'schemas':{k:v['schema_version'] for k,v in D.items()}}
(OUT/'data-independent-recompute-v3.json').write_text(json.dumps({'summary':summary,'checks':checks},indent=2)+'\n');print(json.dumps(summary,indent=2))
