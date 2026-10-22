# Independent CLIENT review, round 1

Frozen TIP: `23647de4220408d42eeef6522deb4d4541528452`. Cumulative base: `8230a955b251e9341d349f99eaeb86f7f8253664`. Read-only review in the canonical checkout. Reviewer did not implement/remediate this programme. Report written 2026-10-22 UTC.

**Slice conclusion: NO-GO. Findings: 0 P0, 6 P1, 3 P2.** The six P1 findings are source-confirmed observer/privacy failures, rather than judgments of visual quality. No native probe was launched and no canonical file, Git index, branch or authority span was changed. All findings were sent promptly to root. Programme-level validation is incomplete independently of these findings.

## Confirmed findings

### CLIENT-P1-1 — Hovering an undetected disguised Spy reveals its actual identity and owner

Primary: `crates/game/src/hover_info.rs:158–172`, also `:218–242`.

`SpecialState::disguise_for` (`special/state.rs:98`) supplies the observer's apparent infantry kind only while that observer's hidden set contains the Spy. `units::place_unit_views` reads that disguise, picks its sprite, paints it HUMAN and deliberately keeps the view visible (`units.rs:647–709`). This is the ordinary hidden-disguised-Spy presentation path, not a synthetic view-only fixture. `hovered_entity` accepts every view with Visibility other than Hidden, then returns the real `Unit.kind` and `view.owner`. `show_hover_label` resolves the actual type label, enemy country/team, real health/armour and rank. Consequently a friendly-looking GI/Conscript Spy is exposed as an enemy Spy by ordinary mouse hover without a Dog or Sensor.

Expected: undiscovered disguise must not expose true type/owner or other hidden identity information through hover. Use observer-qualified apparent information or omit sensitive hover information. Add actual scheduled disguise/detection → client hover coverage. Existing `ra7_visible_disguise_is_not_an_enemy_cursor_target` checks only targeting exclusion and does not exercise this handler.

### CLIENT-P1-2 — Mind control leaves selected enemy units trackable through fog and shroud

Primary: `crates/game/src/selection.rs:425–431`, also `:1161–1179`.

Mind control changes the victim's Owner in `special/infantry.rs:725–747`; release restores it through the same live entity. `units::place_unit_views` updates `view.owner`, live Transform and Visibility while retaining the same UnitViews registry entry. `prune_selection` removes Selection/control-group entries only when the registry entry disappears, not when ownership changes or the actor becomes uncommandable. `draw_selection` draws any selected ID at its live Transform without checking Owner or Visibility. A previously selected unit captured by enemy Yuri therefore retains its green marker when it moves outside sight or into Gap shroud. Recalling an old control group can restore this tracking after the visible selection changes. Simulation ownership rejection does not prevent the information leak.

Expected: prune ownership/commandability changes from both current selection and stored groups; rendering must independently refuse hidden/non-owned markers. Add real mind-control and restoration → selection/group/render controls. This is newly reachable cumulative RA6 behavior and not an aesthetic choice.

### CLIENT-P1-3 — Damage smoke reveals specially concealed naval and Mirage units

Primary: `crates/game/src/weapon_fx.rs:1692–1735`, also `:1975–2007`.

`smoulder` collects all damaged noninfantry Conditions, resolves true actor positions via `spots`, and admits smoke based only on `vision.is_visible(spot.cell)` and cadence. It never checks `SpecialState.hidden`. `Painter::sprite` creates an independent visible Fx with only cell coordinates; `animate` again checks terrain visibility only, so no later stage repairs this. Ordinary damaged Typhoons/Squid can re-submerge after their surfaced timer; a stationary damaged Mirage can disguise. `special/reconnaissance.rs` permits both concealed states regardless of this damage condition and `units.rs` then correctly hides their actor sprite. Their smoke continues to disclose their actual location in visible/explored terrain even without a detector.

Expected: continuous actor-derived presentation must obey the observer's special concealment as well as terrain sight. Add actual damaged/recloaked submarine and still-Mirage client-effect controls. The promotion path in the same file already checks `hidden`; its correctness does not cover smoulder.

### CLIENT-P1-4 — The explicit Interact mode queries hidden occupancy and discloses it through mode, acknowledgement and rejection text

Primary: `crates/game/src/special_controls.rs:363–375`; downstream `:419–422`, `crates/sim/src/command.rs:353–355`.

Press I with a selected unit, then click a hidden cell. `input` chooses Grid.occupant/air occupant/building/Props ID without checking the human vision or SpecialState.hidden. An occupied unseen cell enqueues Interact with the real target ID and leaves Interact mode; an otherwise identical empty cell enqueues nothing and remains in the mode. The accepted sink command also acknowledges, and native validation correctly rejects the hidden target as UnknownTarget. The actual hint then prints “no unit or building <actual ID> is visible to the player.” This exposes hidden occupancy and identity before/through otherwise correct simulation validation. HoveredCell supplies map bounds, not visibility. Right-click's separate `known_at`/interaction_label protections do not apply to this explicit I path.

Expected: choose only observer-known targets for I mode; hidden and missing world states must have the same client mode/sink/audio/text consequences. Preserve known own/visible targets and legitimate scenery contexts with a one-cause hidden/missing test.

### CLIENT-P1-5 — Fogged scenery uses current hidden destruction and health rather than observed state

Primary: `crates/game/src/battlefield.rs:521–570`; related `crates/game/src/hover_info.rs:256–263`.

`raze_scenery` consumes every nonbridge PropDestroyed event, despawns the scenery and rewrites MapTerrain/MapLooks immediately; it has no SimHost/vision argument. With optional fog enabled, an explored cell outside current sight therefore loses its remembered scenery and changes minimap terrain as soon as an enemy destroys the Prop. Independently, scenery hover checks `is_explored` only and reads live Props/hit_points, so hovering fogged unoccupied scenery exposes unseen damage/removal. The later special check is only for occupied garrisons. Ore and bridge draw-state code explicitly preserve last observed state; that does not protect these two paths.

Expected: retain last observed scenery state while out of sight, reconcile it when observed again, and avoid live hidden HP/removal in hover. Add actual fog → unseen damage/destruction → re-observation controls. Default shroud-only behavior does not waive the retained fog extension.

### CLIENT-P1-6 — Remembered bridge-hut approach consults unseen enemy buildings

Primary: `crates/sim/src/ai.rs:1506–1512`, emitted command `:1535–1539`.

R69 intentionally permits an idle Engineer to approach a remembered hut after its scout loses sight. `repair_bridge` chooses its approach goal using `pathfinding::nearest_cell(view.grid, from, |p| grid.is_passable(p) && grid.building(p).is_none() && chebyshev(p,hut)==1)`. Neither candidate occupancy check nor the search is restricted to observed building knowledge. `nearest_cell` delegates to destination_cells and its accepted endpoint changes when an unseen building occupies the first eligible neighboring cell. The returned endpoint is placed directly in the AI's Move command. Thus two worlds with the same AI own state, vision and hut/collapse memory but an unseen hostile building difference can cause different decisions. This is decision-time hidden-world selection, distinct from the documented physical pathfinding consequences after command admission. The later visibility gate protects fresh Interact only.

Expected: plan the remembered approach from public static terrain plus legitimately observed/remembered occupancy; let actual movement resolve unseen blockers. Add equal-knowledge/different-hidden-building command comparison at the actual AI boundary, alongside the existing R69 positive and memory-removal causal control.

### CLIENT-P2-1 — Ordinary text notices do not maintain their advertised burst cap

Primary: `crates/game/src/notices.rs:193–231`.

Deferred Commands are invisible to the `notices` Query during this loop. If the column starts empty and more than six defeat/promotion/crew/crate events arrive in one frame, the seventh event finds no oldest queried Notice to remove and every notice is spawned. With existing notices, repeated events can queue deletion of the same oldest entity. The test ages each event on a separate app.update, so it does not cover a multi-tick frame burst. `announce` also appends directly. This is a bounded-duration UI spill, not the separately correct synchronous PublicBuildingNotices cap. Track pending additions/removals or trim once after applying the whole batch.

### CLIENT-P2-2 — PNG reader retains the pathname metadata/open race

Primary: `crates/game/src/tileset_image.rs:166–175`.

The PNG path checks `std::fs::metadata(path).is_file()` then opens that pathname with blocking File::open. Replacement by a FIFO between these operations can still block startup. The opened-handle/O_NONBLOCK fix in `sim/fileguard.rs` is used by data/maps/settings, not by this image reader (including HD sheets). Direct existing-FIFO rejection and bounded regular-file reads are correct. The race was source-traced, not executed; earlier M1/M7 records used P2 for this boundary class. Align the shared image path with the opened-handle reader while retaining contextual image errors and regular-symlink support.

### CLIENT-P2-3 — Current documents still say broad headless matches omit Props

Primary examples: `docs/RA2_GAP_ANALYSIS.md:354`, `:435`, `docs/RA2_HISTORICAL_DEBT.tsv:6`.

Current map loaders preserve semantic world features and setup_match installs world objects/Props (`world_objects::install`); the production/duel/soak worlds use that setup. The current generic omission prose is false. It is not evidence that runtime Props are absent, and it must not be used to close or invalidate unrelated controls. Retain the actual historical snapshot limitations and replace present-tense claims with the correct current source/evidence scope. Root already reported this lead; deduplicate at panel level.

## Evidence, coverage and correct paths

The dispatched common instructions, bound CLIENT brief, stop addendum, original and continuation prompts, current module supplement and review requirements were read. The historical stopped-state statements do not remove RA5–RA9 from this review. The complete cumulative changed game module list and client/simulation interfaces were covered by source inspection and targeted test/evidence reading. This is not a claim that every line of every large test body or each historical AI remediation was independently executed.

* SimHost's private-world boundary, human-only CommandSink, explicit pre-tick setup, per-tick drain and multi-tick event copying were traced. No ordinary game module mutates simulation gameplay state directly. The known &World entity-allocator exposure remains a separately recorded API boundary; no new gameplay bypass was found in the actual client callers inspected.
* PublicBuildingNotices uses the exact run_tick batch, never hidden actor lookup. It bounds six entries synchronously, replaces by building ID, expires in Time<Real> while paused/offline, invalidates redraw revision at the same tick, clears on actual install/uninstall and protects off-map and known-contact pixels. `draw_minimap` gates the coordinate primitive on HUMAN's real radar. No new viewer API, AI vision grant or radar power was found in this feature.
* Actual unit views/minimap markers exclude hidden actors; `shown_enemy_units` also filters special concealment even when a Spy disguise remains rendered. Those correct targeting/marker routes do not repair the separate hover/FX/selection/I defects above.
* Command-created DMISL/V3 and strategic PDPLANE paths are genuine separate spawned/flights relations. Read selection_pad_native and mission/selection/marker bodies: input layout may be controlled, while actual relations come from native launcher/superweapon commands. CABHUT remains the narrower producer/yard/garrison selection contract. No universal parser for Selectable or IsSelectableCombatant was inferred.
* Authoritative RA2 Tree ordinary target filtering, Ctrl ForceFire, non-tree/legacy tree contexts, garrison contexts and actual preview→sink command paths were read. Construction changes mode only when expected sink commands are accepted; partial rally stays in mode. Ready/no-longer-ready/cancel paths remain distinct.
* Sidebar costs are read dynamically from current Definitions/PlayerState; match replacement does not freeze Super Me prices. Current positive-health/ownership/disabled working-factory estimate gates and simulation factory census were inspected. Lanes, listing, queue badges and captured-side country/prerequisite restrictions remain command validated.
* Settings retain strict schema 1, seven required keys, range/duplicate/unknown checks, 4096-byte bound, missing-vs-dangling distinction, shared opened-handle regular reads, exclusive sibling create_new temporaries, atomic rename and owned cleanup. Actual FIFO/links/concurrent-write test bodies were read. PNG checked-byte serving prevents post-validation rereads and `.meta` injection; both actual App failure handlers correctly emit Error exit on the injected native failure event.
* Announcer and event_sound classifications are owner/public scoped and the audio match is exhaustive. Diagnostic log_sim_events remains deliberately global under D33 debt and is not mistaken for player-facing permission. Music/cues are synthesized in repository code. Audio quality, waveform appearance and user audibility are not passed by source inspection.
* Camera/projection, game-speed fixed timestep, focus/sidebar isolation, map/asset load flow, fog, three-infantry picking/health, rank marks, original generated terrain/HD art, environment-gated visual review, and packaging source were examined. Capture modes are explicitly opt-in; smoke does not establish user visual approval. package_macos preserves configured CARGO_TARGET_DIR and starts the packaged binary without Cargo asset-root variables from an empty short-lived cwd. Signing/notarization/other-Mac launch remain unestablished.
* AI state/ordered hash coverage, MAX_AI_COMMANDS=32 truncation, due/last-stand cadence, owned and seen snapshots, hidden enemy filtering, visible ore, remembered buildings, funded intents, country plans/difficulty, queue/placement, bridge recovery and strategy reservations were traced. Controller state mutations are ordered and hashed; commands go through CommandQueue. The specific bridge planning leak above prevents a blanket honest-vision acceptance.
* The physical/source compositor, source routes/plans/dispositions, completion Whole/component split, scope admission, model162 names/nonhuman consumers, source-model producer and original target metadata were read in their client interface scope. Actual source policies are proposals/choices, not original-engine nonuse. Typed labels/kinds and static absence cannot cover the exposed client behavior. No native-consumption or policy approval is granted here. Data slice's separately reported admission issue remains a lead for the fresh lead to reconcile.

## At least thirty current documentation claims checked against source

These are independent source checks, not new native execution or acceptance. GAP means current sections of docs/RA2_GAP_ANALYSIS.md; CS means docs/CONSTRUCTION_SYSTEM.md. Failures refer to findings above. Historical prose after GAP's historical-baseline boundary is not treated as current behavior.

| # | Current documented claim | Actual source and result |
|---|---|---|
| 1 | GAP America/random AI | skirmish MenuSelection default/initial, countries array: confirmed |
| 2 | GAP Medium AI default | skirmish initial selects middle loaded difficulty: confirmed |
| 3 | GAP 10000 credits, 5000–10000 dialog | Lobby/rules + menu position/step and options mapping: confirmed |
| 4 | GAP unit count10, selectable0–10 | rules lobby + MenuSelection unit row/options: confirmed |
| 5 | GAP MCV default and repack on | MatchOptions defaults, menu options, special_controls repack_target: confirmed |
| 6 | GAP short game default | menu options maps short_game unchanged: confirmed |
| 7 | GAP superweapons/crates/bridge destruction on | MatchOptions→menu options→plan_on: confirmed |
| 8 | GAP ore growth on | options.ore_grows maps its explicit row: confirmed |
| 9 | GAP fog default off | MatchOptions default and MenuSelection mapping: confirmed |
| 10 | GAP computer allied default/free-for-all option | free_for_all default false, player setup and menu mapping: confirmed |
| 11 | GAP Super Me off/human only | options emits only HUMAN in super_players: confirmed |
| 12 | GAP ALL UNLOCKED off/human only | options emits only HUMAN in unlock_players; buildability gates remain: confirmed |
| 13 | GAP unit-speed extension applies with Super Me | unit_speed_percent and match player mapping: confirmed |
| 14 | GAP map-growth default off | MenuSelection default and match_map/plan_on: confirmed |
| 15 | GAP up to10 players/map-dependent capacity | MenuCounts, valid, players and map_growth bounds: confirmed |
| 16 | GAP game speed0.5–4x | GameSpeed SPEED_LEVELS/apply_game_speed, fixed stepping: confirmed |
| 17 | GAP edge scrolling default off/scroll speed5 | CameraOptions default and options parser: confirmed |
| 18 | CS restart retains choices | LastMatch, end_effect, begin_match mapping: confirmed |
| 19 | CS four tabs/air and ships in Vehicles | sidebar Tab::of and Definitions category: confirmed |
| 20 | CS yard main/armoury lanes | sidebar status queue_of/ready_buildings and production menu: confirmed |
| 21 | CS sidebar dynamic prices across replacement | SidebarText::Cost update resolves current production_cost: confirmed |
| 22 | CS accepted-only placement/rally transition | issue_economy_commands compares accepted count with expected: confirmed |
| 23 | CS partial rally remains in mode | count comparison and actual partial-rally test: confirmed |
| 24 | CS invalid placement has real preview | placement_preview invokes check_placement with own positive-HP buildings: confirmed |
| 25 | CS Tree ordinary/force-fire split | shown_scenery plus known_at/modified_order controls: confirmed |
| 26 | CS carried/missile/strategic selection exclusions | launched helper checks spawned and strategic.flights in ordinary click/box; extended_keys same: confirmed for those paths |
| 27 | CS bridge-hut narrow selection policy | own_producers/own_yards/own_garrisons filters: confirmed |
| 28 | GAP/CS public reveal marker grants no sight | PublicBuildingNotices/SimHost batch/draw-only path: confirmed |
| 29 | GAP/CS marker requires powered human radar | radar_online uses HUMAN PlayerState.radar: confirmed |
| 30 | GAP/CS marker9 seconds, paused/offline aging | receive/Time<Real>/revision; no paused gate: confirmed |
| 31 | GAP/CS marker resets on install/uninstall | synchronous clear before new sink use: confirmed |
| 32 | README mapped RA7 keys and Mac alternatives | hotkeys modified_hotkey and handlers: confirmed |
| 33 | README smoke drives menu functions rather than mouse systems | smoke press/choose/play_flow explicitly calls pure menu effects/begin_match: confirmed |
| 34 | README smoke covers fog on/off | choose_first/choose_last toggle FogOfWar and match_problems compares MatchRules: confirmed for smoke scope |
| 35 | CS settings use exclusive sibling + atomic rename | create_settings_temporary/write_settings: confirmed |
| 36 | CS opened-handle settings reads | read_settings→sim::read_bounded_regular: confirmed; images have separate P2 window |
| 37 | GAP honest AI vision | View filters/visible ore correct; remembered hut planning violates it (P1-6) |
| 38 | GAP no special concealment disclosure/README all presentation respects fog | Spy hover, smoke and captured selection violate it (P1-1/2/3) |
| 39 | GAP optional fog preserved | ordinary vision works; scenery observation leaks (P1-5) |
| 40 | GAP broad headless worlds omit scenery | contradicted by current setup world-object installation (P2-3) |
| 41 | GAP all quality checks pending | actual HOWTOSEE51 rows; global pending statement retained: confirmed |
| 42 | GAP original generated audio | audio.rs/announcer.rs synthesize; no source recordings loaded: confirmed source mechanism |
| 43 | README packaged asset lookup | package script copies assets next to executable; unsets both Cargo asset variables: confirmed source mechanism |
| 44 | GAP exact native catalogue counts distinct from art/comparison counts | actual181 catalogue and loaded type IDs support distinction; final-source catalogue execution not inferred |

## R6, R7, R29 and validation distinctions

R6 explicitly replaces I6 with I6′ (one vehicle or up to three on-foot infantry per surface cell, one aircraft per air cell, no actor on a building). The chosen offsets are acknowledged as assumptions. Client position interpolation, infantry bar spacing and click tests read actual subcell positions; this is a coherent client adaptation. Current source tests enforce logical grid room/registration, rather than claiming geometric interpolation never crosses a third cell. I do not certify all simulation occupancy paths from this slice; unresolved SIM findings remain blocking.

R7's 15→20 time conversion is coherent with other timers and is explicitly justified rather than silently retaining the old GI anchor. The single-source infantry factor and undocumented JumpjetSpeed meaning remain correctly named assumptions. The slower GI/Grizzly current values11/13 are not incorrectly compared at arbitrary wall-clock speed. This source review does not turn pacing/feel into user approval.

R29's production match base is still72000, scaled by max(128,width+height)/128; ra8_gate_matches applies the same cap (or a stricter one). m13 ten-player72000 is distinct. m14's diagnostic END_LIMIT300000 is distinct. m7_soak remains a36000-tick window: Running at its end is permitted by R56 item6, completed results require Won. A running equal-hash soak window cannot satisfy full-match Won or post36000 activity. Current twin bodies require late firing, production and positive ore delivery plus actual later Won. Their completed individual frozen-source result exists in the interrupted04 log; final all-map distributions/full gate do not.

Actual final-gate evidence read directly: 01 fmt,02 diff,03 clippy exit0. 04-test was interrupted during the756-configuration production sweep; later required commands were not reached. The individual game binary completed268passed/0failed/0ignored/0filtered at04-test.log:301 (124.90s), including notice lifecycle, real mission/marker/selection, settings and asset-handler targets. The unit/integration targets before m6 retain their actual completed scopes. Six of the seven m6 target tests have individual ok lines; the production sweep has no completed result. No complete command04/full13 pass is claimed. Controller RUNNING/WAITING metadata is stale; stop addendum and raw evidence govern.

A read-only declaration comparison against retained baseline257a169 found211 client test declarations then and358 current client declarations, no old names missing and no ignore attribute. These are declaration counts across source/tests/examples, not268 primary binary tests or nested child subprocess counts. I did not independently prove every changed assertion/fixture across all historical AI commits. The TESTS slice must reconcile that broader obligation; it cannot be closed by this count.

Independent SHA256 verification rehashed all97 current Rust model-census rows with zero byte/count/hash mismatch. External references match rules `adc869ca037db5141f562f18092d022045e194de0d1204a46b9ec8372b2b826f` and art `d732bfc8ffb6d52908342254c22731acb730ae6d2bbfb41983ef109e48a8f061`. `client-static-evidence.json` preserves individual source/log hashes. This is factual provenance, not policy admission or original-engine fidelity approval. No source-policy row, disposition, physical-demand result, expected static guard, alias count or proposal export cures the native/client failures above.

## Unverified and user-only work

No native/Cargo/game/benchmark/soak/package probe was launched by this reviewer, as instructed. The six source-confirmed failures still need real production regressions during remediation. I did not rerun all13 validation commands, regenerate Blender HD art, perform packaged failure-process/other-Mac launch, listen to audio, see actual display output or approve feel. Full current-source audit/accepted-baseline negative execution, every source route's native sufficiency, comprehensive per-commit AI regression integrity, performance tails and all-map winner distributions remain broader final gates; related source and partial/preliminary receipts are insufficient substitutes. The known source-policy admission findings from another slice are not independently accepted here.

All51 visual/audio/animation/readability/pacing/feel/normal-human-victory-and-defeat/platform/packaged-launch HOWTOSEE checks remain pending user verification. The observer leaks in this report are code correctness defects and remain independently actionable even though user quality approval is pending.

Storage: this reviewer created only `round-1-client.md` and `client-static-evidence.json` inside the already designated cache review directory. No new substantial external directory, checkout, clone, worktree, target directory, dataset, environment or package cache was created. These two review artifacts remain necessary for panel/remediation evidence. Canonical source/authority files remain unchanged; validator-owned untracked logs already present at dispatch remain outside this reviewer's ownership.
