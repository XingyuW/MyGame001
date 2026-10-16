# Stopped first frozen attempt

Frozen checkpoint: `b9388f59f1e8f735677ef994c73107ac73ad73de`.
Runtime/data: `81149b64e6ce30929f0a94dd47b92f8cf6226c13`.

The raw captures and incomplete `summary.txt` are preserved unchanged. Command04 exited101:
216 tests passed,1 failed,0 ignored. The exact failure is
`game::units::tests::the_production_data_loads_with_the_combat_file`, which inspected only
`build_order` although country-plan armed defences now live in the separate `defences` list.
R102 records the complete-plan invariant migration and actual paid controller placement proof.

Commands01–03,05–06 exited0. Command07 was compiling when the known-failed attempt's verified
owned process subtree was terminated; its partial capture remains. The outer session exited143.
Commands08–13 did not run. This attempt is failed/incomplete, not a validation pass.

The directory was moved intact from `ra8` to `ra8-attempt1` before another attempt. This note
is a subsequent annotation; no captured output, blank line or summary was edited. All thirteen
commands are required on the corrected committed tree.
