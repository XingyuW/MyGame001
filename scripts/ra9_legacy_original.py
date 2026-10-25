"""UNRUN metadata producer delta. Never import/run in preparation. No expected rows as observations.
Invoke from future approved ra2_scope.write_scope after actual R2 source identity verification.
Only query target section identities are configuration; actual facts come from rules object.
Output hashes, not copied original comments/names/assets or a second 904 source table.
"""
import hashlib,json,re
from pathlib import Path

def legacy_original(rules,targets):
    norm=lambda v:re.sub(r"\s*,\s*",",",v.strip())
    all_rows=[(s,k,norm(v))for s in rules.sections()for k,v in rules.section(s).items()]
    out=[]
    for family,sections in sorted(targets.items()):
        facts=[]
        for target in sections:
            section=rules.section(target)
            facts.append(("section",target,"present",str(section is not None).lower()))
            if section is not None:
                facts.extend(("own",rules.section_name(target),k,norm(v))for k,v in section.items())
            for s,k,v in all_rows:
                if target.lower() in [part.lower()for part in v.split(",")]:
                    facts.append(("incoming:"+target,s,k,v))
        # F251's full original eligibility census is a separate source-side scope premise.
        # No INI text is emitted: hash facts only. New eligible identities fail pending
        # exact policy reconsideration; they cannot disappear behind a mapped-roster filter.
        if family == "F251":
            eligible = sorted(s for s in rules.sections()
                if rules.section(s).get("EligibleForDelayKill", "no").strip().lower() in ("yes","true","1"))
            if eligible != ["AMMOCRAT", "CAMISC01", "CAMISC02"]:
                raise ValueError("F251 complete original EligibleForDelayKill census changed")
            for s in eligible:
                facts.append(("eligible",s,"EligibleForDelayKill","yes"))
                facts.extend(("eligible-own",rules.section_name(s),k,norm(v))
                    for k,v in rules.section(s).items())
        text="".join("\t".join(row)+"\n"for row in sorted(facts))
        out.append((family,str(len(sections)),str(sum(row[0].startswith("incoming:")for row in facts)),hashlib.sha256(text.encode()).hexdigest()))
    return "family_id\tsection_count\tincoming_count\tmetadata_closure_sha256\n"+"".join("\t".join(row)+"\n"for row in out)

def write_legacy_original(rules,reference_dir,producer_dir):
    targets=json.loads((Path(producer_dir)/"ra9_legacy_original_targets.json").read_text())
    result=legacy_original(rules,targets)
    path=Path(reference_dir)/"ra9_legacy_original.tsv"
    path.write_text(result)
    return hashlib.sha256(path.read_bytes()).hexdigest()
