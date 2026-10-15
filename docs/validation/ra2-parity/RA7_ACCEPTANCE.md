# RA7 acceptance ledger

Source freeze and complete gate results will be recorded here after validation. This is an
implementation/validation ledger, not an independent review or approval. R84–R85 record the
rules, compatibility choices and exact intended expectation migrations. Simulation scenarios
use shipped definitions through CommandQueue + SimSchedule; explicit scenario setup supplies
positions, HP, charge, terrain and availability to reach boundaries.

| Scope | Positive, negative and boundary evidence |
|---|---|
| Fog default OFF and effective explored visibility | `ra1_shipped::the_default_match_is_that_of_red_alert_2s_lobby`; `ra7_vision::ra7_explored_enemy_is_known_with_fog_off_but_hidden_with_fog_on` checks the normal explored enemy under both flags. SimPlugin's standalone scenario MatchState explicitly retains fog ON for legacy fixtures; actual setup_match/MatchOptions default OFF. Client smoke chooses fog ON then restores OFF and checks MatchRules. |
| Gap radius10, local sight, hostility and powered satellite | `ra7_gap_restores_circle_shroud_every_tick_but_local_sight_and_allies_counter_it` checks circle edge10/11, repeated reshroud, local sight counter, allied exemption and dead Gap. Existing `ra6_spy_superweapon_resets_charge_and_radar_resets_only_old_shroud` checks powered Satellite radar-Spy exemption. `ra9_data_parity` independently checks GapRadiusInCells10. |
| Psychic Sensor | `ra7_sensor_detects_spy_at_fifteen_but_not_sixteen_and_needs_power` checks disguise edge15/16 and power. `ra7_psychic_order_boundary_power_death_and_special_concealment` checks order endpoint15/16, live sensor/source, owner-only query and hidden-source exclusion. Client purple lines consume that query only. |
| First-spawn and same-tick concealment | `ra7_new_spy_is_concealed_before_first_combat_and_command`, `ra7_new_cloaked_sub_is_hidden_before_automatic_first_combat`, `ra7_paid_spy_production_is_concealed_before_automatic_spawn_tick_fire` protect actual spawn/production timing. `ra7_detector_killed_by_an_earlier_command_cannot_reveal_spy_to_later_attack`, `ra7_selling_the_only_sensor_hides_spy_from_later_same_tick_regular_commands` and `ra7_stopping_moving_mirage_conceals_it_from_later_same_tick_attack` protect within-batch changes. Concealed targets reject exactly like missing. |
| Guard/Scatter/Follow | `ra7_follow_tracks_a_moving_friendly_then_stop_cancels_every_order`, `ra7_scatter_moves_members_apart_and_guard_fires_then_returns`, `ra7_guard_uses_explicit_reference_guard_range_instead_of_longer_weapon_range`, `ra7_follow_forgets_concealed_target_without_tracking_new_position_and_interact_clears_waypoints`. Guard's explicit reference range or primary weapon Range fallback is R85's assumption; Follow forgets hidden targets. |
| Force fire | `ra7_ground_and_allied_force_fire_use_damage_reload_and_source_credit`, `ra7_force_fire_kill_credits_the_actual_attacker` (exact900XP), `ra7_force_ground_fire_spends_air_ammo_and_launches_a_missile_with_fixed_aim`, `ra7_force_fire_backs_out_of_reference_minimum_range_before_launching`. Genuine weapon/warhead/splash/reload/burst/ammunition/launcher paths; reserved ground sentinel is never inserted into the ordinary acquisition index. Missile ground aim survives Stop. |
| Force move and bounded waypoints | `ra7_force_move_crushes_without_acquiring_and_waypoints_run_in_order`; `ra7_new_orders_reject_empty_foreign_offmap_duplicates_and_waypoint_overflow_atomically` checks empty/foreign/offmap/duplicate and32/33 queue boundary, without partial writes. `ra7_deploy_and_chronoshift_clear_pending_waypoints` and the Follow/Interact test check replacement/teleport/boarding clearing. |
| Command privacy and determinism | Extended unchanged `ra1_commands` missing/unseen/submerged matrices cover six new actors plus Follow/ForceFire targets, including empty/unarmed target cases. `ra7_new_command_actors_and_targets_destroyed_earlier_in_the_same_tick_are_missing` covers six actors and two targets. `ra7_new_order_state_and_fog_affect_hash_and_replay_is_deterministic` mutates all new state and replays. `ra7_guard_range_is_required_bounded_and_hashed` checks required/negative/64/65/hash parser boundaries. |
| Context actions and cursors | `ra7_context_actions_validate_engineer_spy_boarding_depot_garrison_and_tanya` checks actual boarding and shared validator labels, hidden/missing/dead negatives. Tanya ships retain ordinary Attack/Sapper. Client `ra7_visible_disguise_is_not_an_enemy_cursor_target` preserves friendly disguise rendering but excludes hidden true identities; `ra7_modifier_cursor_names_the_command_with_the_same_precedence` checks modifier labels/precedence. |
| Keys and camera/selection | `ra7_every_new_binding_maps_input_to_its_order_or_ui_action`, `ra7_selection_cycle_type_all_and_bookmark_actions_execute_on_client_state`, `ra7_deploy_key_maps_every_selected_unit_without_modifier_collisions`, existing exhaustive hotkey/group tests. QWER/KL/GX/FZ/SD/TP/HNM, four bookmark pairs and Mac secondary bindings, CtrlShift attack move, Ctrl/Cmd fire, Alt move/group centring and Y/Pause/F9/backslash/F10/apostrophe replacements. README previous/current table. |
| Original audio and notices | `every_original_cue_is_bounded_distinct_and_has_text`, `announcer_rate_limits_bursts_and_resets_categories`, `ra7_announcements_filter_private_events_and_keep_public_warnings`, `ra7_simulation_event_reaches_the_actual_notice_column`. Thirteen original synthesized motifs, bounded8 queue, category8s/global1s cooldown; private events remain private, public warnings omit source identity/position. |

Regression/mutation evidence is retained in `ra7-preflight/`: first-spawn command acceptance
and firing (`first-spawn-before2.log`), sold Sensor disclosure (`sensor-sale-before.log`),
disguised cursor disclosure (`disguise-cursor-before2.log`), and deliberately retaining Gap
exploration (`mutation-gap-explored.log`) each fail before the corresponding protection. The
mutation source is restored exactly before final checks. Earlier compile/fixture/preflight
attempts are preserved, including the two intentional compatibility migrations documented in
R85; no test was deleted, ignored, skipped or weakened.

Visual/audio/play feel requires user verification. R67 naval bridge-repair refusal and the
RA6 assumptions remain. RA9 still owns Size/SizeLimit/loaded boarding and actual attached Squid
host WeaponFired coverage (R83). Broader AI arsenal use/performance remains RA8; final review
remains independent. No new substantial external directory is created: Cargo uses the existing
configured cache; UV/PIP cache variables remain unchanged; logs stay in the canonical repository.
