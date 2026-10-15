# Combined RA6 acceptance ledger

Implementation evidence; the final full thirteen-command RA6 gate is pending. This is not an
independent review. R71–R81 explain schemas, production paths, migrations and assumptions.
Tests use CommandQueue + SimSchedule with shipped definitions unless explicitly labeled as a
fixture. Scenario setup may set HP, rank, timing, terrain or ownership to reach a boundary.
The [RA6-A ledger](RA6A_ACCEPTANCE.md) retains exact positive/negative/boundary names for its
infantry rows and validated first-part results.

Every row of `docs/RA2_GAP_ANALYSIS.md` section9 is accounted for below. Names without a target
prefix are in `crates/sim/tests/ra6_vehicles.rs`; `A` means the linked RA6-A ledger.

| Gap9 item | Evidence / disposition |
|---|---|
| GI | A: `ra6_gi_deploy_persists_uses_para_and_does_not_move_or_gain_armour`, elite and undeploy boundary cases. R73 D-only undeploy assumption remains. |
| Engineer | A Ivan due-tick defuse; RA5 repair-hut tests; new `ra6_depot_and_repair_ifv_remove_drone_before_its_first_damage_tick` covers infected full-health vehicles. |
| Attack Dog | A high-HP instant bite and disguise detection; IFV mode0 boarding/unloading. |
| Spy | A default/copied disguise and Dog detection; R73 effective-Sight detector range assumption remains. R76 first-spawn concealment risk is handed to RA7. |
| Spy infiltration | A refinery/power boundaries, each factory/lab flag, SW recharge reset, radar shroud reset and satellite exemption; paid-only veterancy and stolen-unit limits. |
| Tanya | A C4 delay, naval Sapper, swimming, immune repair-hut exception and options. |
| Chrono Legionnaire | A multiple beams, elite rate, frozen actors, unharmed release, no death payload/crew, distance delay. |
| Tesla Trooper | A one/two/three trooper power and overcharge thresholds. |
| Prism Tower | `ra6_prism_caps_eight_supporters_and_supports_again_at_tick_80`; existing `ra2_shipped::low_power_stops_the_powered_buildings_that_consume_power_and_no_others` and `ra2_charging` powered positive control. Eight support links, ninth excluded, exact damage and second volley. |
| Mirage Tank | `ra6_stationary_mirage_is_a_tree_to_adjacent_nondetectors`: tree state, adjacent nondetector, Dog detection, moving clears tree. Original per-observer procedural tree needs user visual verification. |
| Crazy Ivan | A normal/elite bombs, final-tick defuse, independent Chrono Ivan ability; `ra6_ivan_ifv_death_uses_crnuke_and_loses_its_passenger`. |
| Yuri | A ownership chain/release/immunity/PsiWave and repeated-deploy cooldown. IFV mode8 uses CRMindControl area damage, not unit mind control. |
| Desolator | A deployed radiation, immune targets, diagonal decay, exact edge/expiry, overlap cap and mutation evidence. IFV mode9 beam is ported. |
| Terror Drone and Squid | `ra6_drone_enters_then_ticks_and_exits_only_on_its_own_kill`, `ra6_drone_infantry_bite_and_protected_vehicle_negative`, `ra6_drone_blocked_escape_retries_with_identity_hp_and_experience`, `ra6_squid_culls_on_the_crossing_hit_and_cannot_cull_through_curtain`, `ra6_squid_prevents_motion_but_ship_keeps_its_weapon`, `ra6_dolphin_sonic_hit_releases_squid_without_killing_it`, depot/repair IFV test. Exact80/132 intervals and elite threshold; long-game attached-owner survival in `match_setup::ra6_parasite_tests`. |
| IFV | `ra6_ifv_uses_reference_modes_and_restores_empty_missiles` covers shipped passenger modes0–11 and unloading; `ra6_ifv_mode_twelve_uses_cow_shot_with_an_independent_mode_fixture` covers12 (cow absent from representative roster). `ra6_repair_ifv_reaches_diagonal_but_not_two_cells_and_obeys_reload` covers1.8-cell range and107-tick cadence. Independent audit verifies all thirteen normal/elite source entries. |
| Nuclear Reactor | `ra6_reactor_chain_radiation_and_temporal_suppression`; ordered chain, 600NUKE/rad500 and erased-reactor suppression. Ivan IFV, Demo Truck and Kirov death cases cover transport cargo loss/crash dispatch. |
| Chronosphere | `ra6_chronosphere_square_moves_both_owners_kills_organic_and_preserves_offsets`, `ra6_chronosphere_rejects_unseen_area_and_kills_bad_or_occupied_landings`; 3x3 edge, enemy ownership, offsets, wrong terrain/occupancy and I6'. `ra1_commands` matrix and `ra4_shipped::the_chronosphere_does_not_move_a_launched_rocket` retain hidden/missing and launched-unit restrictions. Two-click client uses explicit source/destination;80-tick freeze. |
| Iron Curtain | `ra6_curtain_covers_both_owners_kills_infantry_and_expires_at_1000`, `ra6_shipped_curtain_death_prevents_later_same_tick_infantry_command`; any owner, area edge, organic death, exact expiry. |
| Weather Control | `ra6_storm_warning_exclusivity_duration_and_cloud_separation_are_deterministic`, `ra6_storm_rejects_a_second_launch_and_preserves_its_charge`;333 warning,240 duration, deterministic bounded square and active-cloud separation; radar suppression in runtime. R80 explicit13-tick cloud-lifetime approximation remains. |
| Nuclear Missile | `ra6_nuclear_missile_delays_200_ticks_and_radiates`;199/200 boundary, source blast/radiation. `ra3_shipped::a_nuclear_strike_destroys_a_power_plant_and_not_a_construction_yard` retains original exact per-footprint damage after flight. Flight200 is the documented game timing assumption. |
| Recharge at1x | `ra6_strategic_recharges_power_option_and_readiness_edges`:12000/6000/8400/4800, no charge under required power loss, rejected launch, edge-triggered readiness, major-weapon option. `ra9_data_parity` independently checks source RechargeTime/IsPowered. |
| Warning to other players | `ra6_public_timers_and_nuclear_marks_do_not_disclose_unseen_sources`: public owner/type/time only; no unseen source identity or location; nuclear target only owner/15-cell powered Psychic Sensor. Owner-only readiness consumers. Public built/launch events and original audio/UI feedback need user verification. |
| Paradrop | `ra6_airport_paradrops_one_shootable_plane_and_full_side_payload`: six GI/nine Conscripts/eight American GI,400HP plane, radius4; `ra6_anti_air_destroys_paradrop_plane_before_it_drops` and `ra6_paradrop_blocked_landing_retains_payload_until_space_opens`. R80 explicit instant landing/exit approximation remains; blocked passengers retry and AA loss discards remainder. |

Cross-cutting evidence: `ra6_pending_parasite_and_strategic_state_changes_affect_replay_hash`,
four `ra6_definitions` required/bound/negative-damage/hash tests, full `ra9_data_parity` field
inventories, unchanged `m9_arsenal`/`m11_repair`, shipped specialist transport cases from A,
and the expanded `ra1_commands` matrix. R80–R81 list every intended existing-test migration.

Remaining precise differences: persistent Sonic wave frame accumulation is not modeled
(source4/8 damage applied once per shot); no parachute fall/plane exit animation; Storm cloud
lifetime uses13 ticks; representative cow roster omission is covered with an explicit mode12
fixture; original turret/tree/beam/audio presentation requires user verification. R67 naval
bridge-repair refusal and RA6-A documented assumptions remain. Broader RA7 vision/AI and
RA8/RA9 audit/stress/presentation obligations and the independent final review remain open.
