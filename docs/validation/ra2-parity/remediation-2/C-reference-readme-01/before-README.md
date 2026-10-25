# RA2 reference tables

Reference data of the RA2 parity programme (`docs/RA2_PARITY_PLAN.md` section 3.1). The data
parity audit compares the shipped `assets/data/*.ron` with these tab separated tables.

* `ra2_mapping.tsv`: game id to `rules.ini` section. Columns `game_id`, `kind` (`unit`,
  `building`), `section`, `side` (`allied`, `soviet`, `none`), `note` (`twin`: second section
  of one game item; `-`: none). Written by hand; the script checks every row against the ini.
* `ra2_combat.tsv`: game id of a weapon or a warhead to its `rules.ini` section (since RA3).
  Columns `game_id`, `kind` (`weapon`, `warhead`), `section`. Written by hand. Two sections
  may share a game id (the teeth of the two dogs); the generator writes the weapons and
  warheads in the order of the table and fails on one that no type or effect uses, and the
  audit fails on a shipped weapon or warhead that is not mapped.
* `ra2_reference.tsv`: the values the game uses. Columns `file` (`rules`, `art`), `section`,
  `key`, `value`, sorted. Generated, never edited by hand. Since RA3 it also holds the entries
  of the type lists (`[InfantryTypes]`, `[VehicleTypes]`, `[AircraftTypes]`, `[BuildingTypes]`)
  that name a mapped section: the list a unit is in decides whether it is infantry.
* `ra2_differences.tsv`: intentional differences between the shipped data and the reference.
  Columns `game_id`, `field`, `game_value`, `reference_value`, `reason`. Written by hand. A row
  matches one audited value: the game id (`rules` for a value of `rules.ron`), the field as the
  audit names it (`cost`, `strength`, `sight`, `speed`, `prerequisites`, `power_produced`,
  `power_consumed`, `sell_value` (the section's `Soylent` if it has one that is not 0, else the
  cost times `RefundPercent`), `free_units`, `refinery`, `producer`, `factory`, `side`,
  `domain`, `capacity`, `category`,
  `owner_sides`, `required_countries`, `forbidden_countries`, `tech_level`, `present`, since
  RA2 `build_time`, `build_limit`, `lane`, `adjacent`, `base_normal`, `radar`, `sellable`,
  `repairable`, `powered`, `is_powered`, `unit_repair`, since RA3 `armour`, `weapons`,
  `elite_weapons` (the game ids of the slots, comma separated), `air_range_bonus`,
  `land_targeting`, `naval_targeting`, `underwater`, `organic`, `wall`, `infantry`, `crusher`,
  `crushable`, `size`, since RA4 `trainable`, `veteran_abilities`, `elite_abilities`, `crewed`,
  `self_healing`, `ammo`, `airport_bound`, `docks`, `spawns`, `spawns_number`, `spawn_regen`,
  `spawn_reload`, `launcher`, `missile`, `cloakable`, `sensors_sight`, or `group.field` for
  `rules.ron`), the shipped value, and the reference value as the ini writes it. Since RA3 a
  weapon is listed as `weapon.<id>` (fields `damage`, `range`, `min_range`, `reload`, `burst`,
  `warhead`, `projectile_speed`, `anti_air`, `anti_ground`, `scatter`; the reference value of
  `projectile_speed` is `<Projectile>:instant`, `<Projectile>:arcing:<Range>` or
  `<Projectile>:<Speed>`), a warhead as `warhead.<id>` (`versus`, `cell_spread`,
  `percent_at_max`, `wall`, `wood`) and an area effect as `effect.<id>` (`damage`, `warhead`),
  since a weapon and a unit may share an id. A value that is the game's own where the
  reference has none (the damage of the effects `c4` and `radiation`) has a row with the
  reference value `-`. The audit fails on a difference without a row and on a row that no longer
  applies; and on a field of an item's entry in a shipped file that it neither compares nor has
  on its list of the game's own fields (`ITEM_FIELDS` in `ra9_data_parity.rs`).

The shipped data is generated from these tables, and audited against them:

```
python3 scripts/generate_ra2_data.py            # rewrites assets/data/*.ron
python3 scripts/generate_ra2_data.py --check    # exit 1 if a shipped file is stale
cargo test --release -p sim --test ra9_data_parity
```

`scripts/generate_ra2_data.py` reads only `ra2_reference.tsv`, `ra2_mapping.tsv` and
`ra2_combat.tsv` (never the ini files) and its own tables of game specific values; `crates/sim/tests/ra9_data_parity.rs`
recomputes every conversion in Rust and compares the shipped files with the reference.

Regenerating:

```
python3 scripts/ra2_reference.py            # rewrites ra2_reference.tsv
python3 scripts/ra2_reference.py --check    # exit 1 if the committed table is stale
python3 scripts/ra2_reference.py --stats    # counts per category, dangling references
```

The script reads the vanilla Red Alert 2 `rules.ini` and `art.ini` from
`~/Library/Caches/MyGame001/ra2-reference/` (or `RA2_REFERENCE_DIR`) and refuses files whose
SHA-256 differs from the recorded one. It writes the SHA-256 of the table beside it
(`ra2_reference.tsv.sha256`); `--check` first compares the committed table with that hash, so a
table edited by hand fails also without the cache, and then with a fresh extraction. Without the
cache it stops after the first comparison and passes with a note that says so. Both `--check`
calls are commands 12 and 13 of `scripts/validate_ra2_parity.sh`.

Provenance: repository `https://github.com/otherdeniz/CnC_RulesEditor`, commit
`b7278f2e1dab098c5a2f05d4784fffe70a4e5f9c`; file names and hashes are in `docs/DECISIONS.md`
(R2) and in the header of `scripts/ra2_reference.py`. The ini files are never committed, and
neither is any text of them: the tables hold only numbers, percentages, yes/no flags and
identifiers (section, weapon, warhead, projectile and type ids, house names; and, because a
`[Powerups]` value is kept as the one list it is, the short identifier of each crate's
animation, which the game does not read).

## Proposed RA9 source dispositions

The bounded compositor writes `ra9_source_dispositions.tsv`, the nine-column proposed
source table consumed by `ra9_audit/source-routes.rs`:

```
python3 -B scripts/ra2_reference.py --ra9-dispositions
python3 -B scripts/ra2_reference.py --ra9-dispositions --check
```

Both commands require the original INI cache with the recorded R2 hashes. They perform the
existing extraction (including the seven Spark metadata premises), require the committed
reference and its recorded hash to match it, and compare every proposed literal with that
fresh extraction. Check mode compares the entire disposition file without rewriting it.

Generation composes the installed static116, metadata147, residual117, consumer62, carry38,
model162, guard75, behavior87, legacy361/543, masked19 and special10 contracts. It reads the
installed Rust route census and typed class mappings (including all eleven residual
categories), rejects duplicate owners or changed
counts/values, verifies exact legacy family atom fingerprints, and requires the later
packages to partition the original pending atoms. At this preliminary integration the
result has 1,737 whole-source rows: the original 1,708, nineteen masked speed atoms and ten
special metadata atoms (VHP, Tree, Mirage elite and seven Spark premises).

These are proposed dispositions. The table records no source acceptance, native field
consumption, original-engine nonuse, or passed behavior. The Rust declare/admit/finish
plans still require independent real physical comparisons, loaded policy guards, original
scope observations, individual recorded decisions, the final root input binding and actual
scoped route receipts. All UNACCEPTED/UNRUN records remain unchanged. Static generator
checks do not replace native regressions or the full validation gate. If a Rust interface
or exact contract changes, regenerate only after reconciling that change; do not adjust a
count or invent source getters to make this table pass.
