"""UNRUN proposed source-metadata producer. Called only by authorized ra2_reference --ra9-scope.
Never read expected scope tables/approvals to manufacture observations. No native proof.
"""
import hashlib,json,re
from pathlib import Path
TARGETS={'atom-ammo': ['GiantNukeDown', 'NukeCarrier', 'NukeDown', 'NukePayload', 'NukeUp'], 'gems': ['Cruentus'], 'inactive-droppod': ['Vulcan2'], 'inactive-firestorm': ['FirestormWH', 'GAFIRE'], 'inactive-gates': ['GACTWR', 'GAGATE_A', 'GAGATE_B', 'NAGATE_A', 'NAGATE_B'], 'inactive-hunter': ['GAPLUG', 'GHUNTER', 'NATMPL', 'NHUNTER'], 'inactive-ion-emp': ['EMPuls', 'IonCannonWH', 'PulsPr'], 'inactive-powerup': ['GAPOWRUP'], 'inactive-trigger': [], 'inactive-tunnel': [], 'inactive-veins': ['VEINTREE', 'VeinholeWH'], 'inactive-visceroid': ['VISC_LRG', 'VISC_SML'], 'ore-growthrate': [], 'paratrooper-typo': ['E1'], 'prereq-override': ['CARUS03', 'CAWASH16'], 'radiation-typo': ['NANRCT'], 'solo-crates': [], 'technical-countries': ['GDI', 'Neutral', 'Nod', 'Special'], 'turbo': ['Medusa', 'MedusaE', 'RedEye2'], 'upgrades': ['GAPOWRUP']}
SPECIAL_QUERIES=[('NASAM', 'NASAM', 'VHPScan'), ('NASAM', 'NASAM', 'Primary'), ('NASAM', 'NASAM', 'Secondary'), ('NASAM', 'NASAM', 'ElitePrimary'), ('NASAM', 'NASAM', 'EliteSecondary'), ('NASAM', 'NASAM', 'Powered'), ('NASAM', 'NASAM', 'Power'), ('NASAM', 'NASAM', 'Strength'), ('NASAM', 'NASAM', 'Armor'), ('NASAM', 'NASAM', 'Sight'), ('NASAM', 'NASAM', 'LandTargeting'), ('NASAM', 'NASAM', 'NavalTargeting'), ('NASAM', 'RedEye2', 'Damage'), ('NASAM', 'RedEye2', 'Range'), ('NASAM', 'RedEye2', 'MinimumRange'), ('NASAM', 'RedEye2', 'ROF'), ('NASAM', 'RedEye2', 'Burst'), ('NASAM', 'RedEye2', 'Projectile'), ('NASAM', 'RedEye2', 'Warhead'), ('NASAM', 'RedEye2', 'Speed'), ('NASAM', 'AAHeatSeeker', 'AA'), ('NASAM', 'AAHeatSeeker', 'AG'), ('NASAM', 'AAHeatSeeker', 'Inviso'), ('NASAM', 'AAHeatSeeker', 'Inaccurate'), ('NASAM', 'AAHeatSeeker', 'Arcing'), ('NASAM', 'AAHeatSeeker', 'ROT'), ('NASAM', 'SAMWH', 'Verses'), ('NASAM', 'SAMWH', 'CellSpread'), ('NASAM', 'SAMWH', 'PercentAtMax'), ('NASAM', 'ZEP', 'Armor'), ('NASAM', 'ZEP', 'SelfHealing'), ('NASAM', 'ZEP', 'Strength'), ('Spark', 'Spark', 'Damage'), ('Spark', 'Spark', 'Warhead'), ('Spark', 'Spark', 'NextParticle')]
REGISTRIES=("BuildingTypes","InfantryTypes","VehicleTypes","AircraftTypes","TerrainTypes","Warheads")
def norm(v):return re.sub(r"\s*,\s*",",",v.strip())
def all_rows(rules):return [(s,k,norm(v)) for s in rules.sections() for k,v in rules.section(s).items()]
def original75(rules):
    actual=all_rows(rules);out=[]
    for group,targets in sorted(TARGETS.items()):
        out.extend([(group,"universe","rules","keys",str(len(actual))),(group,"universe","rules","sections",str(len(rules.sections())))])
        for target in targets:
            out.append((group,"section",target,"present",str(rules.has_section(target)).lower()))
            members=[]
            for registry in REGISTRIES:
                for key,value in (rules.section(registry) or {}).items():
                    if norm(value)==target:members.append(registry+":"+key)
            out.append((group,"registry",target,"members",",".join(sorted(members))))
            # Retain multiset duplication where one actual row names two queried targets.
            for section,key,value in actual:
                if target in value.split(","):out.append((group,"incoming",section,key,value))
        if group in ("gems","ore-growthrate"):
            for section in ("Riparius","Cruentus"):
                for key in ("Growth","GrowthPercentage","Spread","SpreadPercentage"):
                    v=rules.get(section,key)
                    if v is None:raise ValueError("actual ore source key absent "+section+"."+key)
                    out.append((group,"ore-primary",section,key,norm(v)))
        if group in ("inactive-powerup","upgrades"):
            participants=[(s,k,v) for s,k,v in actual if k.lower().startswith("powersup")]
            out.append((group,"upgrade-universe","rules","participants",str(len({s for s,_,_ in participants}))))
            out.extend((group,"upgrade-participant",s,k,v) for s,k,v in participants)
        if group=="radiation-typo":
            v=rules.get("NANRCT","ImmuneToRadiation")
            out.append((group,"correct-key","NANRCT","ImmuneToRadiation","<ABSENT>" if v is None else norm(v)))
    return "group\tfact_kind\tsection_or_target\tkey\tvalue\n"+"".join("\t".join(r)+"\n" for r in sorted(out))
def special_original(rules):
    out=[]
    for owner,section,key in SPECIAL_QUERIES:
        v=rules.get(section,key)
        out.append((owner,section,key,"absent" if v is None else "present","" if v is None else norm(v)))
    for section,key,value in all_rows(rules):
        if key.lower()=="vhpscan":out.append(("vhp-key-census",section,key,"present",value))
    return "owner\tsection\tkey\tpresence\tvalue\n"+"".join("\t".join(r)+"\n" for r in sorted(out))
def fnv(data):
    h=0xcbf29ce484222325
    for b in data:h=((h^b)*0x100000001b3)&0xffffffffffffffff
    return format(h,"016x")
def tokens(text):
    # Conservative lexical scanner: nested block/line comments, Rust normal/raw strings,
    # attributes and punctuation. Unrecognized chars reject rather than hide a new surface.
    out=[];i=0
    while i<len(text):
        if text[i].isspace():i+=1;continue
        if text.startswith("//",i):
            j=text.find("\n",i);i=len(text) if j<0 else j+1;continue
        if text.startswith("/*",i):
            depth=1;i+=2
            while depth:
                if i>=len(text):raise ValueError("unterminated Rust comment")
                if text.startswith("/*",i):depth+=1;i+=2
                elif text.startswith("*/",i):depth-=1;i+=2
                else:i+=1
            continue
        raw=re.match(r'r(#{0,255})"',text[i:])
        if raw:
            end='"'+raw.group(1);j=text.find(end,i+raw.end())
            if j<0:raise ValueError("unterminated Rust raw string")
            out.append(text[i:j+len(end)]);i=j+len(end);continue
        if text[i]=='"':
            j=i+1
            while j<len(text):
                if text[j]=='\\':j+=2
                elif text[j]=='"':break
                else:j+=1
            if j>=len(text):raise ValueError("unterminated Rust string")
            out.append(text[i:j+1]);i=j+1;continue
        char=re.match(r"'(?:\\.|[^'\\])'",text[i:])
        if char:out.append(char.group(0));i+=char.end();continue
        word=re.match(r"[A-Za-z_][A-Za-z_0-9]*|[0-9]+|::|=>|->",text[i:])
        if word:out.append(word.group(0));i+=word.end();continue
        if text[i] in "{}()[]<>,;:=#&*!?+./|%-'@^$":out.append(text[i]);i+=1;continue
        raise ValueError("unparsed Rust token near "+repr(text[i:i+20]))
    return out
def body(t,at):
    if t[at]!="{":raise ValueError("expected Rust item body")
    depth=1;j=at+1
    while j<len(t) and depth:
        if t[j]=="{":depth+=1
        if t[j]=="}":depth-=1
        j+=1
    if depth:raise ValueError("unclosed Rust item")
    return t[at+1:j-1]
def item_body(t,kind,name):
    places=[i for i in range(len(t)-2) if t[i:i+2]==[kind,name]]
    if len(places)!=1:raise ValueError("ambiguous/missing actual Rust item "+kind+" "+name)
    at=places[0]+2
    while at<len(t) and t[at]!="{":at+=1
    return body(t,at)
def variants(t,name):
    b=item_body(t,"enum",name);parts=[];level=0;start=0
    for i,v in enumerate(b):
        if v in ("{","(","["):level+=1
        if v in ("}",")","]"):level-=1
        if v=="," and level==0:parts.append(b[start:i]);start=i+1
    if b[start:]:parts.append(b[start:])
    names=[]
    for part in parts:
        while part[:2]==["#","["]:
            depth=1;j=2
            while j<len(part) and depth:
                if part[j]=="[":depth+=1
                if part[j]=="]":depth-=1
                j+=1
            part=part[j:]
        if not part or not re.fullmatch(r"[A-Za-z_][A-Za-z_0-9]*",part[0]):raise ValueError("unsupported actual variant")
        names.append(part[0])
    if len(names)!=len(set(names)):raise ValueError("duplicate enum surface")
    return names
def actual_ids(t,enum,names):
    out=[]
    for name in names:
        found=[json.loads(t[i+4]) for i in range(len(t)-4) if t[i:i+4]==[enum,"::",name,"=>"] and t[i+4].startswith('"')]
        if len(set(found))!=1:raise ValueError("ambiguous actual id arm "+enum+"::"+name)
        out.append(found[0])
    return out
def source_model(root):
    paths=sorted(p for p in root.glob("crates/*/src/**/*.rs") if p.is_file())
    if any(p.is_symlink() for p in paths):raise ValueError("source symlink needs explicit approval")
    measured=[(p.relative_to(root).as_posix(),p.read_bytes()) for p in paths]
    census="path\tsha256\tfnv1a64\tbytes\n"+"".join(path+"\t"+hashlib.sha256(b).hexdigest()+"\t"+fnv(b)+"\t"+str(len(b))+"\n" for path,b in measured)
    legacy="".join(path+"\t"+hashlib.sha256(b).hexdigest()+"\n" for path,b in measured)
    needed={"crates/sim/src/command.rs","crates/sim/src/arsenal.rs","crates/sim/src/combat_defs.rs","crates/sim/src/crates.rs","crates/sim/src/match_setup.rs","crates/sim/src/world_objects.rs"}
    text={path:tokens(b.decode("utf-8")) for path,b in measured if path in needed}
    cmd=text["crates/sim/src/command.rs"];ars=text["crates/sim/src/arsenal.rs"];combat=text["crates/sim/src/combat_defs.rs"];crates=text["crates/sim/src/crates.rs"]
    effect=actual_ids(combat,"Effect",variants(combat,"Effect"));crate_ids=actual_ids(crates,"CrateContent",variants(crates,"CrateContent"))
    setup=text["crates/sim/src/match_setup.rs"];entries=[setup[i+1] for i in range(len(setup)-1) if setup[i]=="fn" and setup[i+1].startswith("setup_match")]
    world=text["crates/sim/src/world_objects.rs"]
    world_ids=[json.loads(world[i+2]) for i in range(len(world)-2) if world[i].isdigit() and world[i+1]=="=>" and world[i+2].startswith('"')]
    calls=[]
    for i in range(len(world)-5):
        if world[i:i+6]==["crate","::","bridges","::","install","("]:calls.append("bridges::install")
        if world[i:i+5]==["Props","::","from_grid","(","world"]:calls.append("Props::from_grid")
    # This is observed syntax/dispatch inventory, not proof of every subsystem's absence.
    surface="commands="+",".join(variants(cmd,"Command"))+";domains="+",".join(variants(ars,"Domain"))+";roles="+",".join(variants(ars,"Role"))+";effects="+",".join(effect)+";crate-content="+",".join(crate_ids)+";setup-entrypoints="+",".join(entries)+";worldobject-match="+",".join(world_ids)+";worldobject-calls="+",".join(calls)+"\n"
    return census,hashlib.sha256(legacy.encode()).hexdigest(),surface
def produce(root,rules,original_sha):
    root=Path(root);original=original75(rules);special=special_original(rules);census,model_sha,surface=source_model(root)
    from ra9_legacy_original import legacy_original
    legacy=legacy_original(rules,json.loads((root/"scripts/ra9_legacy_original_targets.json").read_text()))
    hashes=[original_sha,model_sha,hashlib.sha256(surface.encode()).hexdigest(),hashlib.sha256(original.encode()).hexdigest(),hashlib.sha256(special.encode()).hexdigest()]
    observation="original_sha256\tmodel_census_sha256\tmodel_surface_sha256\toriginal_closure_sha256\tspecial_original_sha256\n"+"\t".join(hashes)+"\n"
    return {"ra9_legacy_original.tsv":legacy,"ra9_scope75_original.tsv":original,"ra9_special_original.tsv":special,"ra9_model_census.tsv":census,"ra9_model_surface.txt":surface,"ra9_scope_observation.tsv":observation}
