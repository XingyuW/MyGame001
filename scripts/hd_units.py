"""Every unit of the roster: dispatch to the model families (all authored in metres)."""
import hd_vehicles, hd_infantry, hd_aircraft, hd_naval

FAMILIES = (hd_vehicles, hd_infantry, hd_aircraft, hd_naval)

def build(m, ident):
    for family in FAMILIES:
        if ident in family.MODELS:
            return family.build(m, ident)
    raise KeyError(f'no model for unit {ident!r}')
