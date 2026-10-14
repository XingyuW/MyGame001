# RA6-A acceptance ledger

Implementation evidence; validation is pending. This is not an independent review.
All `ra6_infantry` cases use shipped definitions and commands/SimSchedule; scenario setup
may set ownership, health, rank or timing to reach a boundary without changing shipped data.
The exact function names below are in `crates/sim/tests/ra6_infantry.rs`.

| Mechanic | Positive, negative and boundary evidence | Exact test function |
|---|---|---|
| GI | Para selected; persists past 130 ticks; Move cannot move deployed unit; explicit undeploy permits movement | `ra6_gi_deploy_persists_uses_para_and_does_not_move_or_gain_armour` |
| GI/Temporal elite | Elite Para selected; elite temporal subtracts 12 per tick | `ra6_elite_gi_uses_elite_para_and_elite_temporal_has_double_rate` |
| Ivan/Engineer | Normal400 and injected elite600; charge600 ticks; Engineer defuses in due-tick Commands | `ra6_ivan_normal_elite_and_engineer_defuse_on_due_tick` |
| Spy money/power | Half current credits once; power entry affects only blackout; exact1333-tick boundary | `ra6_spy_refinery_steals_half_once_and_power_causes_only_blackout` |
| Tanya | No instant C4 death;36-tick delayed destruction | `ra6_tanya_c4_is_delayed_and_does_not_kill_on_entry` |
| Desolator | Immediate deployed eruption; infantry damaged; buildings and immune Desolator survive | `ra6_desolator_emits_on_deploy_and_radiation_spares_buildings_and_immune_infantry` |
| Terrorist | Ordinary death emits shipped blast once, no second tick repeat | `ra6_terrorist_death_explodes_once_with_shipped_weapon` |
| Temporal | Two beams sum12 per tick; target HP protected; moving sources releases target unharmed | `ra6_temporal_sums_beams_freezes_and_releases_unharmed` |
| Yuri | New explicit target releases previous; controller death releases current | `ra6_yuri_releases_previous_and_releases_on_controller_death` |
| Yuri pulse | PsiWave damage; psionic immunity;200-tick undeploy boundary | `ra6_yuri_psi_pulse_and_undeploy_boundary` |
| Spy disguise | Default side appearance; copies infantry; dog detection | `ra6_spy_disguise_default_copy_and_dog_detection` |
| Dog | Kills infantry with1000 HP despite30 nominal damage | `ra6_dog_bite_kills_infantry_even_above_nominal_weapon_damage` |
| Spy factories/labs | Each building entry sets exactly its own flag | `ra6_each_spy_factory_and_lab_entry_sets_only_its_own_unlock` |
| Tesla | 2 low-power threshold;3 overcharge;1 with full power | `ra6_tesla_charge_two_low_power_three_overpowered_one_with_power` |
| Transports | Nine specialist types embark/unload; identity HP/rank preserved | `ra6_specialists_embark_and_unload_preserving_identity_health_and_rank` |
| Radiation decay | Diagonal429→344; exact edge zero; expiry; reirradiation | `ra6_radiation_diagonal_decay_edge_expiry_and_reirradiation` |
| Stolen units | All four prerequisites and barracks; denied missing tech; Prime same-tick queue limit | `ra6_stolen_units_require_lab_flags_and_proper_barracks_and_prime_limit_is_same_tick` |
| Control transfer | P0→P1→P2 chain restores true P0 on second controller death | `ra6_control_chain_returns_to_original_owner_after_second_controller_dies` |
| Production veterancy | Paid GI/vehicle promoted; free scenario spawn not promoted | `ra6_veteran_infiltration_applies_to_paid_production_not_free_spawns` |
| Spy SW/radar | Recharge reset; old shroud forgotten; powered satellite exemption | `ra6_spy_superweapon_resets_charge_and_radar_resets_only_old_shroud` |
| Tanya bridge | Immune hut exception collapses linked bridge only with option enabled | `ra6_tanya_c4_hut_collapses_bridge_only_when_enabled` |
| Tanya swim/Sapper | Water movement and naval attack; nonnaval target rejects Sapper | `ra6_tanya_swims_and_sapper_is_exclusive_to_naval_targets` |
| Radiation overlap | Summed cap500; heavy damage10; empty expired sites removed | `ra6_radiation_overlap_caps_damage_and_empty_sites_expire` |
| Temporal death | Erased Terrorist emits no death blast; frozen actor Move rejected | `ra6_temporal_removal_has_no_terrorist_death_blast_and_frozen_actor_cannot_move` |
| Determinism | Same replay hash; new runtime state mutations change hash | `ra6_specialist_replay_and_each_new_runtime_state_are_hashed` |
| Limited cargo | Prime passenger still counts; carrier death frees limit | `ra6_yuri_prime_limit_counts_cargo_and_releases_on_carrier_death` |
| Ship veterancy | Paid ship remains rookie despite factory infiltration | `ra6_factory_training_never_promotes_a_paid_ship` |
| Psi cooldown | Same-tick D/off/D cannot repeat pulse | `ra6_repeat_psi_deploy_cannot_bypass_weapon_reload` |
| Hospital transport | Tanya/Spy/Ivan/Engineer enter friendly hospital; no bomb or infiltration | `ra6_specialists_enter_friendly_hospital_instead_of_infiltrating_or_bombing` |
| Stolen abilities/chrono | Three chrono types: minimum21 and distance85 ticks; FakeC4 vs actual C4 vs Ivan charge | `ra6_chrono_infantry_distance_delay_and_stolen_abilities_are_independent` |

Additional required evidence:

- `ra1_commands::ra6_shipped_interaction_targets_hidden_unseen_and_missing_have_identical_answers`
  checks specialist enemy targets against missing IDs.
- `ra1_commands::ra6_shipped_specialists_and_transport_passengers_cannot_act_after_same_tick_death`
  covers new Deploy/Interact actors after lethal commands.
- `ra6_definitions::ra6_each_new_flag_is_required_bounded_and_hashed` and
  `ra6_area_fire_one_cell_minimum_is_narrow_and_new_numbers_are_bounded` cover loader boundaries.
- `ra9_data_parity` audits every ported field against committed reference data, including full
  field-inventory guards. R72 enumerates every intended existing-test expectation migration.
- The unchanged `m9_arsenal`24 and `m11_repair`4 preserve no-rules fixture behavior.
- Cheap mutations removing radiation's overlap cap and replacing per-step truncation with
  cumulative truncation both fail the associated tests (exit101); original source restored.
  Logs remain in `ra6a-preflight/radiation-no-cap.log` and `radiation-cumulative-decay.log`.

RA6-B must extend payload/death dispatch for vehicles/buildings, IFV modes, crashes/cargo
loss, structures and superweapons, and complete its own shipped transport interactions.
This ledger closes the RA6-A portion of R56 specialist/transport debt only. D-only GI
undeploy, detector effective-Sight range and elite Ivan600 are explicit R73 assumptions;
visual/audio quality still requires user verification. No independent approval is claimed.
