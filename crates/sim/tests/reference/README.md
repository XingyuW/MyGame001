# RA2 reference tables

Reference data of the RA2 parity programme (`docs/RA2_PARITY_PLAN.md` section 3.1). The data
parity audit compares the shipped `assets/data/*.ron` with these tab separated tables.

* `ra2_mapping.tsv`: game id to `rules.ini` section. Columns `game_id`, `kind` (`unit`,
  `building`), `section`, `side` (`allied`, `soviet`, `none`), `note` (`twin`: second section
  of one game item; `-`: none). Written by hand; the script checks every row against the ini.
* `ra2_reference.tsv`: the values the game uses. Columns `file` (`rules`, `art`), `section`,
  `key`, `value`, sorted. Generated, never edited by hand.
* `ra2_differences.tsv`: intentional differences between the shipped data and the reference.
  Columns `game_id`, `field`, `game_value`, `reference_value`, `reason`. Written by hand. A row
  matches one audited value: the game id (`rules` for a value of `rules.ron`), the field as the
  audit names it (`cost`, `strength`, `sight`, `speed`, `prerequisites`, `power`, `sell_value`,
  `owner_sides`, `required_countries`, `forbidden_countries`, `tech_level`, `present`, or
  `group.field` for `rules.ron`), the shipped value, and the reference value as the ini writes
  it. The audit fails on a difference without a row and on a row that no longer applies.

The shipped data is generated from these tables, and audited against them:

```
python3 scripts/generate_ra2_data.py            # rewrites assets/data/*.ron
python3 scripts/generate_ra2_data.py --check    # exit 1 if a shipped file is stale
cargo test --release -p sim --test ra9_data_parity
```

`scripts/generate_ra2_data.py` reads only `ra2_reference.tsv` and `ra2_mapping.tsv` (never the
ini files) and its own tables of game specific values; `crates/sim/tests/ra9_data_parity.rs`
recomputes every conversion in Rust and compares the shipped files with the reference.

Regenerating:

```
python3 scripts/ra2_reference.py            # rewrites ra2_reference.tsv
python3 scripts/ra2_reference.py --check    # exit 1 if the committed table is stale
python3 scripts/ra2_reference.py --stats    # counts per category, dangling references
```

The script reads the vanilla Red Alert 2 `rules.ini` and `art.ini` from
`~/Library/Caches/MyGame001/ra2-reference/` (or `RA2_REFERENCE_DIR`) and refuses files whose
SHA-256 differs from the recorded one. Without that cache `--check` passes with a note.

Provenance: repository `https://github.com/otherdeniz/CnC_RulesEditor`, commit
`b7278f2e1dab098c5a2f05d4784fffe70a4e5f9c`; file names and hashes are in `docs/DECISIONS.md`
(R2) and in the header of `scripts/ra2_reference.py`. The ini files are never committed, and
neither is any text of them: the tables hold only numbers, percentages, yes/no flags and
identifiers (section, weapon, warhead, projectile and type ids, house names).
