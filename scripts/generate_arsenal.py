"""Reproducible original RA2-inspired balance data; preserves legacy AI definitions."""
from pathlib import Path
import re
ROOT=Path(__file__).resolve().parents[1]/'assets/data'
def entries(text,name):
    start=re.search(r'(?m)^    '+name+r': \[',text).end()
    depth=1; i=start
    while depth:
        if text[i]=='[': depth+=1
        if text[i]==']': depth-=1
        i+=1
    return start,i-1

def update(file,section,rows,key):
    p=ROOT/file;s=p.read_text();a,b=entries(s,section);old=s[a:b]
    records=[];depth=0;begin=0
    for token in re.finditer(r'"(?:[^"\\]|\\.)*"|[()]',old):
        if token[0]=='(':
            if depth==0:begin=token.start()
            depth+=1
        elif token[0]==')':
            depth-=1
            if depth==0:records.append(old[begin:token.end()])
    for ident,row in rows:
        if ident in {'construction_yard','power_plant','refinery','barracks','war_factory','tech_center','conscript'}: continue
        found=next((i for i,r in enumerate(records) if re.match(r'\(\s*'+key+r': "'+re.escape(ident)+r'"',r)),None)
        if found is None:records.append(row)
        else:records[found]=row
    p.write_text(s[:a]+'\n'+''.join('        '+r+',\n' for r in records)+'    '+s[b:])

def q(s): return '"'+s+'"'
def names(xs): return '['+', '.join(map(q,xs))+']'
# id, label, cost, ticks, prerequisite, role, power output/input, footprint
B=[
('construction_yard','Construction Yard',3000,1200,[],'combat',20,0,3),
('power_plant','Power Plant',800,200,[],'combat',200,0,2),
('tesla_reactor','Tesla Reactor',600,180,[],'combat',150,0,2),
('refinery','Ore Refinery',2000,400,[],'combat',0,40,3),
('ore_purifier','Ore Purifier',2500,700,['tech_center'],'purifier',0,150,2),
('nuclear_reactor','Nuclear Reactor',1500,700,['tech_center'],'combat',2000,0,2),
('barracks','Barracks',500,180,[],'combat',0,10,2),
('war_factory','War Factory',2000,500,['refinery','barracks'],'combat',0,30,3),
('naval_shipyard','Naval Shipyard',1000,400,['refinery'],'shipyard',0,30,3),
('airforce_command','Airforce Command Headquarters',1000,400,['refinery'],'combat',0,50,3),
('service_depot','Service Depot',800,300,['war_factory'],'repair',0,30,2),
('tech_center','Battle Lab',2000,700,['war_factory','radar_tower'],'combat',0,100,2),
('radar_tower','Radar Tower',1000,350,['refinery'],'combat',0,50,2),
('cloning_vats','Cloning Vats',2500,800,['tech_center','barracks'],'cloner',0,100,2),
('fortress_walls','Fortress Walls',100,40,[],'combat',0,0,1),
('pillbox','Pillbox',500,180,['barracks'],'combat',0,0,1),
('sentry_gun','Sentry Gun',500,180,['barracks'],'combat',0,0,1),
('patriot','Patriot Missile System',1000,300,['barracks'],'flak',0,50,1),
('flak_cannon','Flak Cannon',1000,300,['barracks'],'flak',0,50,1),
('prism_tower','Prism Tower',1500,450,['radar_tower'],'splash',0,75,1),
('tesla_coil','Tesla Coil',1500,450,['radar_tower'],'tesla',0,75,1),
('grand_cannon','Grand Cannon',2000,650,['tech_center'],'splash',0,100,2),
('gap_generator','Gap Generator',1000,400,['tech_center'],'gap',0,100,2),
('spy_satellite','Spy Satellite Uplink',1500,500,['tech_center'],'satellite',0,100,2),
('psychic_sensor','Psychic Sensor',1000,350,['radar_tower'],'sensor',0,50,2),
('chronosphere','Chronosphere',3000,1600,['tech_center','airforce_command'],'chronosphere',0,200,2),
('weather_control','Weather Control Device',5000,2000,['tech_center','airforce_command'],'storm',0,200,3),
('iron_curtain','Iron Curtain Device',3000,1600,['tech_center'],'curtain',0,200,2),
('nuclear_silo','Nuclear Missile Silo',5000,2000,['tech_center','nuclear_reactor'],'nuke',0,200,3),
]
# id,label,cost,time,producer,prereqs,role,domain,sprite,hp,speed,weapon,armour
U=[]
def unit(id,label,cost,hp,speed,weapon,role='combat',pre=(),domain='land',sprite='rifleman',producer='barracks',armour='infantry'):
    U.append((id,label,cost,max(80,cost//3),producer,list(pre),role,domain,sprite,hp,speed,weapon,armour))
unit('gi','GI',200,125,14,'rifle')
unit('conscript','Conscript',100,120,14,'carbine',sprite='conscript')
unit('engineer','Engineer',500,100,14,None,'engineer')
unit('attack_dog','Attack Dog',200,100,36,'dog_bite','dog',sprite='scout')
unit('rocketeer','Rocketeer',600,180,30,'air_rifle',pre=['airforce_command'],domain='air',armour='air')
unit('spy','Spy',1000,100,16,None,'spy',pre=['tech_center'])
unit('tanya','Tanya',1000,250,22,'commando_gun','commando',pre=['tech_center'])
unit('chrono_legionnaire','Chrono Legionnaire',1500,150,14,'chrono_beam','chrono',pre=['tech_center'])
unit('sniper','Sniper',600,100,12,'sniper_rifle',pre=['radar_tower'])
unit('flak_trooper','Flak Trooper',300,100,12,'flak_weapon','flak',sprite='rocket_soldier')
unit('tesla_trooper','Tesla Trooper',500,200,10,'tesla_weapon','tesla',pre=['radar_tower'],sprite='rocket_soldier')
unit('crazy_ivan','Crazy Ivan',600,150,16,'bomb_trigger','demolition',pre=['radar_tower'],sprite='grenadier')
unit('yuri','Yuri',1200,100,12,'control_beam','mind_control',pre=['tech_center'])
unit('desolator','Desolator',600,200,10,'radiation_weapon','radiation',pre=['radar_tower'],sprite='grenadier')
unit('terrorist','Terrorist',200,100,22,'suicide_trigger','suicide',sprite='conscript')
def vehicle(id,label,cost,hp,speed,weapon,role='combat',pre=(),domain='land',sprite='light_tank',producer='war_factory',armour='heavy'):
    unit(id,label,cost,hp,speed,weapon,role,pre,domain,sprite,producer,armour)
vehicle('chrono_miner','Chrono Miner',1400,1000,20,None,'chrono_miner',['refinery'],sprite='harvester')
vehicle('war_miner','War Miner',1400,1000,16,'scout_gun',pre=['refinery'],sprite='harvester')
vehicle('grizzly','Grizzly Battle Tank',700,300,28,'tank_cannon')
vehicle('rhino','Rhino Heavy Tank',900,500,20,'heavy_cannon',sprite='heavy_tank')
vehicle('ifv','Infantry Fighting Vehicle (IFV)',600,200,40,'flak_weapon','ifv',sprite='scout',armour='light')
vehicle('flak_track','Flak Track',500,250,32,'flak_weapon','transport',sprite='raider',armour='light')
vehicle('terror_drone','Terror Drone',500,100,64,'drone_claw','drone',sprite='scout',armour='light')
vehicle('v3_launcher','V3 Rocket Launcher',800,180,16,'v3_missile',pre=['radar_tower'],sprite='rocket_launcher',armour='light')
vehicle('mcv','Mobile Construction Vehicle (MCV)',3000,1000,12,None,'mcv',['service_depot'],sprite='harvester')
vehicle('mirage_tank','Mirage Tank',1000,250,28,'mirage_gun','mirage',['tech_center'])
vehicle('prism_tank','Prism Tank',1200,200,20,'prism_beam','splash',['tech_center'],sprite='artillery',armour='light')
vehicle('apocalypse','Apocalypse Tank',1750,1000,12,'apocalypse_gun',pre=['tech_center'],sprite='heavy_tank')
vehicle('tank_destroyer','Tank Destroyer',900,400,20,'destroyer_gun',sprite='artillery')
vehicle('tesla_tank','Tesla Tank',1200,400,22,'tesla_weapon','tesla',['radar_tower'],sprite='heavy_tank')
vehicle('demolition_truck','Demolition Truck',1500,150,24,'suicide_trigger','suicide',['tech_center'],sprite='harvester',armour='light')
for id,label,cost,hp,speed,weapon,role in [('harrier','Harrier',1200,200,96,'air_bomb','strike'),('black_eagle','Black Eagle',1400,300,110,'eagle_bomb','strike'),('nighthawk','NightHawk Transport',1000,300,56,'air_rifle','transport'),('kirov','Kirov Airship',2000,2000,8,'kirov_bomb','splash')]:
    vehicle(id,label,cost,hp,speed,weapon,role,['tech_center'] if id=='kirov' else [],'air','scout' if id!='kirov' else 'heavy_tank','airforce_command','air')
for id,label,cost,hp,speed,weapon,role,armour in [('amphibious_transport','Amphibious Transport',900,1000,28,None,'transport','naval'),('destroyer','Destroyer',1000,600,24,'naval_gun','combat','naval'),('aegis_cruiser','Aegis Cruiser',1200,800,20,'aegis_missile','flak','naval'),('dolphin','Dolphin',500,200,40,'sonic','combat','submerged'),('aircraft_carrier','Aircraft Carrier',2000,1000,16,'carrier_strike','carrier','naval'),('typhoon','Typhoon Attack Submarine',1000,600,24,'torpedo','combat','submerged'),('sea_scorpion','Sea Scorpion',600,400,36,'flak_weapon','flak','naval'),('giant_squid','Giant Squid',1000,300,32,'squid_grip','squid','submerged'),('dreadnought','Dreadnought',2000,1000,14,'dread_missile','combat','naval')]:
    vehicle(id,label,cost,hp,speed,weapon,role,['tech_center'] if id in ['aircraft_carrier','giant_squid','dreadnought'] else [],'amphibious' if id=='amphibious_transport' else 'naval','harvester' if role=='transport' else 'artillery','naval_shipyard',armour)

supercfg={'chronosphere':(3600,3,0),'storm':(6000,5,1600),'curtain':(3600,3,200),'nuke':(6000,6,2500)}
meta=[];build=[];bcombat=[]
for idx,(id,label,cost,time,pre,role,out,draw,size) in enumerate(B):
    sprite= id if id in ['construction_yard','power_plant','refinery','barracks','war_factory','tech_center'] else ('gun_turret' if idx>=14 and size==1 else 'power_plant' if out else 'tech_center' if size==2 else 'war_factory')
    # Generated existing building sheets have fixed footprint dimensions.
    sprite={'naval_shipyard':'war_factory','airforce_command':'war_factory','nuclear_silo':'war_factory','weather_control':'war_factory'}.get(id,sprite)
    free=['chrono_miner'] if id=='refinery' else []
    build.append((id,f'(id: {q(id)}, sprite: {q(sprite)}, width: {size}, height: {size}, cost: {cost}, build_time: {time}, producer: "construction_yard", prerequisites: {names(pre)}, power_produced: {out}, power_consumed: {draw}, sell_value: {cost//2}, free_units: {names(free)})'))
    weapon={'pillbox':'rifle','sentry_gun':'scout_gun','patriot':'aegis_missile','flak_cannon':'aegis_missile','prism_tower':'prism_beam','tesla_coil':'tesla_weapon','grand_cannon':'grand_shell'}.get(id)
    bcombat.append((id,f'(building: {q(id)}, hit_points: {300 if size==1 else 1000}, armour: "structure", sight: {8 if idx>=14 else 6}, weapons: {names([weapon] if weapon else [])})'))
    cool,radius,strength=supercfg.get(role,(100,6,20))
    description={'purifier':'Increases ore delivery income by 25 percent while powered.','repair':'Repairs nearby friendly vehicles while powered.','cloner':'Duplicates trained infantry while powered.','shipyard':'Place on water adjacent to shore. Produces naval units.','gap':'Shrouds this area against enemy vision.','satellite':'Reveals the battlefield while powered.','sensor':'Detects concealed units nearby.','chronosphere':'Select vehicles, choose ability, then click a visible destination.','curtain':'Click a visible area to protect friendly mechanical units for 10 seconds.','storm':'Click a visible area for a devastating lightning storm.','nuke':'Click a visible area for a nuclear strike.'}.get(role,'Base infrastructure.' if idx<14 else 'Automatically engages valid enemy targets.' if weapon else 'Blocks ground movement.')
    meta.append((id,label,description,'buildings' if idx<14 else 'armory','land',role,cool,radius,strength,0))
update('economy.ron','buildings',build,'id');update('combat.ron','buildings',bcombat,'building')
unitrows=[];prod=[];combat=[]
for id,label,cost,time,producer,pre,role,domain,sprite,hp,speed,weapon,armour in U:
    unitrows.append((id,f'(id: {q(id)}, speed: {speed}, sprite: {q(sprite)})'))
    prod.append((id,f'(unit: {q(id)}, cost: {cost}, build_time: {time}, producer: {q(producer)}, prerequisites: {names(pre)})'))
    combat.append((id,f'(unit: {q(id)}, hit_points: {hp}, armour: {q(armour)}, sight: {12 if id=="sniper" else 8}, weapons: {names([weapon] if weapon else [])})'))
    cap=2 if role=='carrier' else 1 if role=='ifv' else 12 if id=='amphibious_transport' else 5 if role=='transport' else 0
    desc={'engineer':'E + click a nearby structure to capture or repair it.','spy':'E + click a nearby enemy building to infiltrate. Dogs detect spies.','mcv':'D deploys into a Construction Yard on clear ground.','transport':'Select passengers; E + click this transport to load. D unloads.','ifv':'Carries one infantry; passenger changes its weapon. D unloads.','chrono':'Teleports when given a move order; beam suppresses targets.','mind_control':'Controls one enemy unit; control ends when Yuri dies.','demolition':'Attaches a delayed explosive when attacking.','radiation':'D contaminates surrounding ground.','suicide':'Detonates on reaching attack range.','mirage':'Concealed while idle; revealed by nearby detectors.','strike':'Fires a strike then returns to an air facility to rearm.','drone':'Immobilizes and damages mechanical targets.','squid':'Immobilizes enemy vessels.','chrono_miner':'Harvests ore and teleports home to unload.','carrier':'Launches recoverable strike aircraft; no airbase required.'}.get(role,'Specialized '+('air' if domain=='air' else 'naval' if domain=='naval' else 'ground')+' combat. Select and right click an enemy to attack.')
    strength=800 if role=='demolition' else 1600 if id=='demolition_truck' else 500 if role=='suicide' else 20
    meta.append((id,label,desc,'infantry' if producer=='barracks' else 'vehicles',domain,role,100,3 if role in ['radiation','suicide'] else 2,strength,cap))
update('units.ron','units',unitrows,'id');update('economy.ron','units',prod,'unit');update('combat.ron','units',combat,'unit')
update('economy.ron','harvesters',[(id,f'(unit: {q(id)}, capacity: {cap}, harvest_rate: 7)') for id,cap in [('chrono_miner',500),('war_miner',1000)]],'unit')
# Add target domains to every warhead, explicitly preventing ground-only fire at aircraft.
p=ROOT/'combat.ron';s=p.read_text();s=s.replace('"heavy", "structure"]','"heavy", "structure", "air", "naval", "submerged"]')
s=re.sub(r'(\(armour: "structure", percent: (\d+)\),)(\s*\])',lambda m:m[1]+f'\n                (armour: "air", percent: 0), (armour: "naval", percent: {m[2]}), (armour: "submerged", percent: 0),'+m[3],s)
p.write_text(s)
WH={'anti_air':[0,0,0,0,100,0,0],'flak':[50,30,15,15,100,40,0],'anti_infantry':[100,0,0,0,0,0,0],'anti_vehicle':[0,100,100,0,0,80,0],'naval':[25,50,50,50,0,100,100],'torpedo':[0,0,0,0,0,100,100],'omni':[50,100,100,100,100,100,0],'control':[100,100,100,0,0,0,0]}
update('combat.ron','warheads',[(id,f'(id: {q(id)}, versus: ['+', '.join(f'(armour: {q(a)}, percent: {n})' for a,n in zip(['infantry','light','heavy','structure','air','naval','submerged'],vs))+'])') for id,vs in WH.items()],'id')
W=[('dog_bite',200,384,12,'anti_infantry',0),('air_rifle',25,1280,15,'bullet',0),('commando_gun',150,1536,12,'bullet',0),('chrono_beam',60,1280,30,'control',0),('sniper_rifle',250,3072,70,'anti_infantry',0),('flak_weapon',30,1536,20,'flak',0),('tesla_weapon',100,1280,40,'high_explosive',0),('bomb_trigger',1,384,120,'high_explosive',0),('control_beam',1,1536,100,'control',0),('radiation_weapon',50,1024,30,'high_explosive',0),('suicide_trigger',1,384,100,'high_explosive',0),('drone_claw',30,384,20,'anti_vehicle',0),('v3_missile',160,3840,140,'shell',128),('mirage_gun',100,1792,35,'high_explosive',0),('prism_beam',100,2560,65,'shell',0),('apocalypse_gun',120,1792,45,'omni',160),('destroyer_gun',140,1536,40,'anti_vehicle',160),('air_bomb',180,1536,60,'shell',0),('eagle_bomb',260,1536,60,'shell',0),('kirov_bomb',400,512,55,'shell',0),('naval_gun',80,1792,40,'naval',160),('aegis_missile',100,2560,20,'anti_air',192),('sonic',100,1536,30,'torpedo',0),('carrier_strike',250,3840,160,'shell',128),('torpedo',120,1792,50,'torpedo',128),('squid_grip',50,384,30,'torpedo',0),('dread_missile',300,4096,180,'shell',128),('grand_shell',200,3584,80,'shell',128)]
update('combat.ron','weapons',[(id,f'(id: {q(id)}, damage: {d}, range: {r}, reload: {t}, warhead: {q(w)}, projectile_speed: {v})') for id,d,r,t,w,v in W],'id')
(ROOT/'arsenal.ron').write_text('// Unified original RA2-inspired arsenal. All durations use 20 ticks per second.\n(schema_version: 1, entries: [\n'+''.join(f'    (id: {q(id)}, label: {q(label)}, description: {q(desc)}, category: {q(cat)}, domain: {q(dom)}, role: {q(role)}, cooldown: {cool}, radius: {rad}, strength: {strength}, capacity: {cap}),\n' for id,label,desc,cat,dom,role,cool,rad,strength,cap in meta)+'])\n')
