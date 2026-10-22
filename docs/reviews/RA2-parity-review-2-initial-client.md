# Independent CLIENT review, round 2

Frozen TIP: `1cedaee83b9cb299869a581d13a25e2736c8c41f`. Cumulative base: `8230a955b251e9341d349f99eaeb86f7f8253664`. Reviewer: `/root/review2_client`, independent of implementation, remediation and preparation. Canonical source was read only. The service-outage downtime interrupted this review and is excluded from its active review window.

**Slice conclusion: NO-GO. New confirmed findings: 0 P0, 2 P1. Retained confirmed nonblocking findings: 2 P2.** The first P1 has two separately traced client surfaces. Shared final validation/acceptance is also incomplete; that is a programme gate, not a third unique client defect. Overall adjudication belongs to the fresh lead. No visual/audio/feel approval is given.

## Confirmed P1 findings

### R2-CLIENT-P1-1 — Ordinary order preview and commands still disclose unseen scenery and bridge changes

Primary locations: `crates/game/src/selection.rs:723`, `:851`, `:855`, `:1078`; connected `crates/game/src/battlefield.rs:488`. Complete source paths were inspected; no native reproduction was launched.

**A. Props destruction and occupation.** `shown_scenery` obtains the current `Props.id_at`/kind at lines725–729, rejects any *current* garrison at lines736–741, and otherwise requires only `is_explored` at746. `preview_order` calls it for an armed selection at851–852, feeds the resulting enemy into `order_intent`, and stores the target. The normal intent cursor/text and marker consume that result. Ordinary `issue_orders` independently calls the same function at1094 before choosing Attack versus movement; it refuses an Invalid preview at1081. This bypasses the newly added last-observed scenery snapshot.

Concrete premises and contrast: enable optional fog; observe an impassable unoccupied House, then move the observer away; keep an ordinary armed own Grizzly selected and hover that explored, currently unseen House in Normal mode. While the live Prop remains unoccupied, the preview is `Attack`/ATTACK and a right click issues Attack to its actual ID. If an enemy destroys the House outside sight, the repaired `raze_scenery` correctly retains the observed sprite, MapLooks, impassable MapTerrain and hover HP. But live Props now returns no ID: enemy becomes None, the remembered terrain is impassable, and the cursor immediately becomes `Invalid`/CANNOT GO; ordinary right click issues nothing. If an enemy instead garrisons that unseen House, the live garrison exclusion similarly changes the preview, while `shown_enemy_building` and `interaction_label` correctly refuse the unseen occupied host. Thus two observer-identical worlds produce different public cursor/marker/sink/acknowledgement consequences before re-observation. This is a gameplay privacy defect, not aesthetic cursor quality.

The discrepancy also reaches Ctrl force fire and F Follow through `preview.target` taking precedence over `known_at` in `issue_orders`/`extended_keys`. The explicit I path itself is repaired: it calls observer-qualified `known_at`, so it is not independently reported again. `known_at`'s own current-visible/special-hidden protections do not repair a target supplied first by `shown_scenery`.

**B. Bridge terrain.** `bridge_states` unconditionally copies the live simulation `Grid.cell` into client MapTerrain at488–489. Its visibility guard protects only MapLooks at493. The deck sprite's `animate_ground` correctly retains its observed collapsed state while unseen, and the look-based minimap can retain its deck colour, but `preview_order` reads the now-current MapTerrain at855–870. Over an explored bridge cell outside current sight, an ordinary land unit's cursor therefore changes between Move and CANNOT GO as an enemy collapses/repairs that bridge unseen. A right click is respectively admitted into the sink or suppressed by the Invalid guard. A weaponless MCV supplies a land control that avoids scenery-target precedence; a weaponless naval transport supplies the inverse land/water contrast. Stock Engineer carries DefuseKit and the current client counts any Weapon component as armed, so it must not be assumed to isolate this branch. No actor at the hovered point or special context is needed.

Expected: every preview, label, order marker and command-choice path must use observer-qualified scenery/terrain state, with hidden and unchanged worlds producing equal pre-admission consequences. Preserve actual visible/owned contacts, re-observation, ordinary known House/garrison contexts, authoritative RA2 Tree ordinary-versus-Ctrl distinction and legacy tree behavior. `observed_scenery` already supplies last observed kind/HP/occupation for display, but contains no remembered SimId: resolving it through live Props would reintroduce a hidden-presence sink difference. A consistent visible-only targeting policy can avoid new memory state; preserving remembered entity attacks needs a legitimate remembered identity/cell-target policy. Do not fix only the visible label while leaving the independently recomputed sink target live.

Test gap: `privacy_scenery_tests.rs` observes sprite/top count, MapLooks, MapTerrain and hover for a House, plus hover after hidden occupation; it never runs normal preview/issue_orders/modifier/Follow consequences. No new control exercises unseen bridge terrain through the actual cursor and sink. Add actual observed→fog→native unseen damage/destruction/occupation/collapse/repair→re-observation comparisons, including equal hidden/missing observations and visible positives.

### R2-CLIENT-P1-2 — Remembered AI bridge approach still learns unseen terrain mutations

Locations: `crates/sim/src/ai.rs:1486`, `:1545`, `:1571`; `crates/sim/src/props.rs:347`; `crates/sim/src/pathfinding.rs:172`; connected bridge mutations `crates/sim/src/bridges.rs:253`, `:285`. Complete producer→planner→issued-command path inspected; no native reproduction was run.

The building-occupancy correction is real: the planning grid reconstructs only owned, visible and legitimately remembered building footprints. However, its supposedly static terrain is copied without any visibility qualification from the **live** grid at ai1486–1492. Prop Cleanup changes that terrain's passable/buildable values on destruction at props347–355; bridge collapse/repair also changes terrain. `nearest_cell` uses `destination_cells`, which consults `grid.is_passable` both for every traversed neighbor and every accepted endpoint, then ai1571–1575 puts that selected goal directly in a Move command before ordinary movement executes. No subsequent knowledge filter repairs this decision.

A feasible source counterexample retains the existing native control's geometry: an idle Engineer at(10,22), remembered hut at(21,22), and a House initially occupying the cheapest eligible adjacent cell(20,22). Keep both worlds' original map, own actors, vision and hut/collapse/controller memories equal; retire the scout; destroy that House through enemy activity outside sight in only one world. The Engineer's stock four-cell sight does not reach the cell. In the intact world that endpoint is not passable, so the planner chooses another adjacent endpoint; in the unseen-destruction world it is newly passable and the cheapest endpoint. The AI's outgoing goal changes solely due to hidden destruction. A hidden route cut/opening can also change reachability. These are decision-time differences, separate from legitimate physical pathfinding after an unchanged Move is admitted.

Expected: bridge planning uses static public map terrain plus legitimately observed/remembered terrain updates, alongside its corrected knowledge-qualified occupancy. Hidden physical changes are resolved after ordinary command admission. Preserve fresh visible hut repair and old remembered approach/memory-removal controls. The current `ra9_privacy_remembered_bridge_approach_ignores_unseen_endpoint_and_corridor_buildings` varies only ECS buildings; its two B-focused-02 cases pass and correctly prove that narrower fix. It cannot establish static/observed terrain. Add equal-knowledge native command comparisons for unseen Prop and bridge terrain changes. This is the RA8 honest-vision acceptance criterion and R195's explicit static-terrain promise; it is not a claim of original-engine AI fidelity.

## Retained P2 findings

* **R1 CLIENT-P2-1 remains:** `notices.rs:193–231` still counts deferred additions but chooses removals from the unchanged Query on each event. A burst into an empty column can exceed MAX_NOTICES, and a burst into a populated column can schedule the same oldest deletion repeatedly. `announce` also appends without enforcing the cap. Source confirmed; existing one-event-per-update tests do not cover the same-frame batch. PublicBuildingNotices' synchronous cap is a separate correct implementation.
* **R1 CLIENT-P2-2 remains:** `tileset_image.rs:166–175` still performs pathname metadata then blocking File::open. The file can become a FIFO between those operations. Direct existing-FIFO rejection and bounded regular reads remain correct; the shared nonblocking opened-handle reader protects data/maps/settings but not PNG/HD sheets. Race not executed; retain the prior P2 classification rather than inventing a new observed hang.

The prior CLIENT-P2-3 present-tense “headless matches omit Props” finding is corrected in current GAP/ledger/harness wording. `world_objects::install` is used by shared match setup. Historical claim cells remain historical and are not mistaken for current behavior or exhaustive scenery stress.

## Panel1 findings and remediation adjudication

| Panel1 client finding | Independent round2 result |
|---|---|
| Spy true identity on hover | Narrow original defect corrected. hover_info substitutes disguise type/HUMAN colour and owner, uses apparent armour, suppresses true rank, and retains the displayed health ratio. privacy_hover drives actual scheduled disguise and Dog detection; compares normal GI and disguised Spy at80%, then real detected Spy. No original media-quality credit. |
| Mind-controlled selection/groups/markers | Corrected. prune_selection uses current real ownership/positive Health/mobile/nonmission relation, prunes every group, and draw_selection separately requires nonhidden owned commandable views. Native Yuri capture/controller death control inspects actual GizmoAsset and restored selection. |
| Concealed damage smoke | Corrected for fresh emissions. smoulder checks observer special.hidden before collecting emitters. Native Typhoon/Mirage controls derive concealment/damage Condition via schedule and actual detector departure; no forged hidden relation. Already-emitted visible effects are not presented as new hidden actor tracking. |
| Explicit I hidden occupancy | Corrected. I input calls known_at. Real handler/control compares commands, mode, accepted counter, sound counter and rejection text for hidden/missing Spy, ordinary ground/air/building/scenery; retains known transport/scenery positives. |
| Fogged scenery live HP/destruction | Actual sprite/map/hover paths corrected via SimHost observed snapshots before/after every tick, with install/uninstall/reset controls and hidden occupation hover equality. **Not fully closed across the client:** R2-CLIENT-P1-1 traces remaining preview/sink and bridge terrain surfaces. |
| Remembered AI approach hidden buildings | Corrected for unseen building endpoint/traversal; B-focused-02 records equal commands to(20,22) for empty and two unseen wall worlds. **Honest-vision acceptance not fully closed:** R2-CLIENT-P1-2 traces live terrain. |
| Notice cap / PNG race | Retained P2 above. |
| Incorrect headless-Props documentation | Corrected in current assessments; historical evidence preserved. |

## Cumulative requirement and path coverage

I read the bound dispatch/common round2 instructions, original/continuation prompts, current review requirements and ownership supplement/census, panel1 client report and lead finding/gate adjudications, relevant RA0–RA9 parity-plan acceptance/conversion sections, PLAN invariants/severities and relevant D33/D34/D43/R6/R7/R29/R195/R197 decisions. Historical stopped-state prose was not used to remove RA5–RA9. The cumulative baseline-to-TIP game paths and current interfaces were inspected, including unchanged checked asset/CLI/PNG/projection contracts. This is source/path coverage and selective test-body/evidence inspection, **not a claim that every line of39,065 source/test lines or every historical AI regression was independently executed**.

All39 actual game-source file SHA256 values independently match `round2-current-module-scope-v2.json` (zero mismatches). The six test-only privacy includes are files, not six extra production systems. Every file's coverage is named below; grouping does not omit them.

| Actual files under crates/game/src | Requirement/path examined |
|---|---|
| main.rs, sim_host.rs | Private simulation World, human-only Sink, bounded pending queue/refusal/discard counters, install/check/uninstall, command transfer, per-tick complete events across multi-tick client frames; observed scenery lifecycle. No new production caller obtains mutable gameplay state. |
| selection.rs, selection_pad_native.rs, ra7_controls.rs, hotkeys.rs | Click/drag/Shift/type/cycle/P/groups/bookmarks/home/Alt centring; actual spawned versus strategic-flight exclusions; known/hidden contacts; current capture pruning/marker guard; Tree ordinary versus force fire, walls, House/garrison/context/modifier/Follow/preview→Sink paths. Native missions establish real relation; controlled UnitView geometry is only input layout, not rendered-native proof. CABHUT remains explicit narrow producer/yard/garrison policy. |
| special_controls.rs | Deploy/repack, known I, strategic source/target mode, public timers, own/visible strategic effects and nuclear marks, actual rejection feedback. |
| hover_info.rs, health.rs | Visible actor picking, apparent Spy identity, ratio/armour/rank, observed scenery/occupation, positive visible bars/dock/rank presentations. |
| units.rs, buildings.rs, fog.rs | View registries, current owner/position/concealment/disguise, spawned/mission actors, building visibility, shroud/fog, carried/removed actor lifecycle; log event classification deliberately global under retained D33 debt. No user art/animation acceptance. |
| battlefield.rs, weapon_fx.rs | Observed scenery reconciliation, bridge memory discrepancy, ore depletion/reappearance, crates, garrison pips, particles, shot/impact/promote gates, concealed smoke correction, shell and effect lifetimes/caps. Source classification and generated death/world effects do not supply original-engine media fidelity. |
| minimap.rs | HUMAN powered radar/offline/click gating, known/owned markers with special.hidden, public BuildingRevealed event-only coordinate notices, six-entry synchronous cap/replacement, real-time pause/offline expiry/revision, off-map bounds/contact-pixel preservation and install/uninstall reset. No hidden actor query maintains public notices and no notice powers radar/grants AI vision. |
| construction.rs, sidebar.rs | Dynamic loaded costs and match replacement, factory/positive HP/disabled census, lanes/build limits/availability/country/prerequisites, actual placement checks, accepted-only placement/rally transitions and partial rejection, preview/ghost and teardown. |
| skirmish.rs, options.rs, game_speed.rs | RA2 defaults and retained optional extensions, MenuSelection→MatchOptions/PlayerSetup, random country/Medium choices,21-map catalogue and map-growth/player bounds, restart choices, settings schema1/seven required keys/4096-byte bounds/errors, nonblocking read/exclusive sibling temp/atomic rename, fixed-step speed0.5–4 and pause. |
| map.rs, projection.rs, camera.rs | Checked source-byte map/tileset path, projection/picking/bounds, focus/sidebar and bookmark camera interaction, map teardown, genuine asset-error exit handler. |
| checked_assets.rs, cli.rs, tileset_image.rs, hd_art.rs, presentation.rs | Checked-byte renderer source/.meta refusal, full u32 smoke argument bounds, PNG bounds/decode and retained race, loaded generated-art names/anchors/crops/team cache/recolour, no imported original art/string/audio claim. |
| announcer.rs, audio.rs, notices.rs | Owner/public cue/event classification, exhaustive sound match, acknowledgement/rejection coupling, rate limits/lifetimes/pause/new-match behavior, original synthesized code and retained burst-cap flaw. Hearing/quality not approved. |
| smoke.rs, visual_review.rs | Real app smoke layout/flow/options/restart/radar/production/movement/AI/asset failure/early-exit assertions; environment-only capture entry points and pre-tick scene setup. Smoke pixels/test fixtures do not substitute for user's visual/audio/feel checks. |
| privacy_native.rs, privacy_hover_tests.rs, privacy_selection_tests.rs, privacy_smoke_tests.rs, privacy_interact_tests.rs, privacy_scenery_tests.rs | Read real host/step/control helper and all seven new privacy control bodies, their meaningful positive/negative premises, and uncovered preview/terrain boundaries. |

AI cross-slice coverage: `ai.rs`, `ai_defs.rs`, `ai/strategy.rs`, current country plans in `assets/data/ai.ron`, bridge controls in `ra8_ai.rs`, current controller/hash/cadence/funded-intent/cancellation/worker/last-stand/wave and own/seen memory boundaries were traced. MAX_AI_COMMANDS stays32 and actual changes go through CommandQueue; ordered new controller state is hashed. Difficulty is the actual ROF/build-time/credit/income/team policy; the ini has no difficulty Firepower key and I do not invent one. Current rules-present native late twins/all21 evidence is scoped below. Sim2 remains primary for every sim production file; this report does not assume its acceptance resolves these client-facing honest-vision defects.

Source-policy interface: inspected actual audit physical→source admission routes (`ra9_audit/mod.rs`, source-routes/source-plans/policy-admission/source-disposition/scope-inputs/completion/completion-whole) and compositor/source-model/legacy-original producer interfaces. Exact original source literals, per-key typed routes, current labels/nonhuman/death/world/bob consumers, static33 presentation versus29 MODEL-ABSENCE splits and Whole/component distinctions were cross-checked in their client scope. `source_model` is a read-only syntax/census observation; typed labels/static absence/exported proposals are not observed actor/render behavior or original-engine nonuse. Unit resale/Grinder absence creates no UI action and does not waive current building refunds. Public reveal, concealment, missions, selection and real current exposed behavior remain obligations despite policy disposition. Data/tests slices own exhaustive per-atom admission/physical closure; I grant no blanket1911-atom semantic acceptance. All183 debt IDs remain, with qualified source/native/remaining obligations rather than documentation approval.

## Thirty-plus current documentation claims sampled against actual consumers

These are source checks, not new native executions. GAP=current RA2_GAP_ANALYSIS, CS=CONSTRUCTION_SYSTEM, README=current descriptive sections. Historical provenance paragraphs are retained historical claims.

| # | Document claim | Independent source result |
|---|---|---|
|1|America human/random AI defaults|MenuSelection initial/countries mapping: consistent.|
|2|Medium AI default|initial uses middle loaded difficulty: consistent.|
|3|10000 credits;5000–10000 steps100|Lobby/options/rules dialog mapping: consistent.|
|4|unit count10;0–10 offered|Lobby position/place/options bounds: consistent.|
|5|MCV start and repack on|MatchOptions/menu/special repack path: consistent.|
|6|short game default on|menu→options mapping: consistent.|
|7|superweapons/crates/bridge destruction on|menu→plan→setup mapping: consistent.|
|8|ore growth option|explicit ore_grows mapping: consistent.|
|9|shroud only default;optional fog|default false, vision/render/sink routing present; unseen client changes fail P1-1.|
|10|allied AI default/FFA selectable|free_for_all default and players/options: consistent.|
|11|Super Me off and human-only|super_players contains only HUMAN when chosen: consistent.|
|12|ALL UNLOCKED off and human-only|unlock_players mapping; buildability separately retained: consistent.|
|13|unit-speed extension requires Super Me|unit speed option only changes super-player setup: consistent.|
|14|map growth default off|MenuSelection default/scaled_for: consistent.|
|15|one–nine computers/map-capacity bounds|MenuCounts/valid/players/map growth: consistent.|
|16|21 maps checked|catalogue iterates every loaded map and check_match trial setup: consistent source; isolated actual21 also below.|
|17|speed0.5–4x|SPEED_LEVELS/fixed timestep/GameClock: consistent.|
|18|edge scrolling off/speed5 defaults|CameraOptions parser/default: consistent.|
|19|restart retains choices|LastMatch/end_effect/begin_match: consistent.|
|20|four tabs;air/navy under Vehicles|Tab::of/Definitions categories: consistent.|
|21|main and armoury lanes|queue_of/status/ready/build buttons: consistent.|
|22|dynamic prices across replacement|update_sidebar resolves current definitions/player costs: consistent.|
|23|placement/rally mode changes only on sink acceptance|expected count compared with issue count: consistent.|
|24|partial rally stays in mode|same comparison plus retained test: consistent.|
|25|validity preview|actual placement_preview invokes check_placement with owned live buildings: consistent.|
|26|RA2 Tree ordinary/force-fire split|shown_scenery+known_at/modified_order: consistent for genuinely observed contexts; preserve on repair.|
|27|mission actors outside ordinary selections|real spawned/strategic relations checked by click/box/type/cycle: consistent.|
|28|CABHUT narrow selection|own_producers/own_yards/own_garrisons: consistent; no universal Selectable claim.|
|29|public reveal grants no sight|event-only notices/draw primitive: consistent.|
|30|public notice requires powered human radar|radar_online(HUMAN PlayerState): consistent.|
|31|notice9 real seconds,including pause/offline|Time<Real> expiry and revision: consistent.|
|32|public notices clear on replacement|install/uninstall synchronous clear: consistent.|
|33|known contacts preserve pixels|draw_public_building_notices excludes contact cells: consistent.|
|34|Spy appears friendly until detected|units apparent art and repaired hover agree; real Dog control read.|
|35|captured units cease own selection|current ownership prune/marker guard: consistent after remediation.|
|36|fog retains last observed scenery|sprite/map/hover fixed; broad client claim fails ordinary targeting and bridge terrain P1-1.|
|37|AI keeps honest vision/static bridge planning|own/visible snapshots/building memory correct; hidden mutable terrain fails P1-2.|
|38|settings schema unchanged|version1/seven exact fields/format/read: consistent.|
|39|settings atomic replacement|create_new temporary/flush/drop/rename/owned cleanup: consistent.|
|40|asset failure exits in normal and smoke modes|both real App handlers emit Error; actual executable/package acceptance pending.|
|41|headless setup includes Props|current setup_match→world_objects install: consistent corrected prose.|
|42|original art/sound differs and requires user verification|generated sprite/world/synth consumers present; no quality approval inferred.|

## Validation evidence and limits

I launched **no Cargo/game/native/match/benchmark/soak/package/mutation process**, requested no lease, and created no clone/worktree. Root exclusively owned native validation. I independently rehashed all five `B-isolated-five-01` raw logs against process.json: zero mismatches. The exact frozen HEAD/source is recorded unchanged. All five commands exited0: all21 ten-player maps reached actual Won; fixed3 are24000-tick performance windows (9/6/5 players remain, not all-to-Won); three example guards each pass their named control. All21 Won ticks range35520–147715 under that example's separate bound, **not** the72000 two-player cap. Maximum all21 busiest1000 mean9.727ms; fixed3 busiest1000 means4.875/5.585/4.904ms. These facts do not stand in for the full workspace/duels/soak gate.

I also independently rehashed/read the command results in B-focused-02, B-strict-04, B-focused-03 and B-late-causal-01 (zero raw-hash mismatches): B-focused-02 game debug has275 primary passes and four separate selected one-test child passes (274 filtered each); bridge target2/0/133filtered. The same B-focused-02 complete audit is actually184pass/1fail, retained as failed. B-strict-04 runtime is43 across6 targets plus fmt/diff/strict Clippy; B-focused-03 has4 whole-audit controls/181filtered and a separate late twin1/0/6filtered. Its targeted retries are not an unfiltered185 pass. Corrected twins reach Won41738/72000 with444 late shots/8 late production/6 positive late ore; actual tick36001 RNG mutation and observer-only production omission each fail, then unchanged restored positive passes. These receipts retain exact snapshot scope; no historical run is relabelled final-source acceptance.

The R194 shared full13 has **not started** because source blockers were found. Fresh full185 audit, workspace release tests, both600-frame smoke runs, builds, benchmark, soak, full duels, packaging and both regeneration checks remain required under the unchanged commands/caps. Actual release/package missing-asset failure/restoration acceptance is pending with root. Old interrupted199 grants neither final gate. No performance from overlapping work or a source declaration is accepted as completed runtime evidence.

Limits: no fresh native fail-before reproduction for the two new findings, no exhaustive independent rerun of every old AI repair/test, no exhaustive independent per-atom policy verdict, no recovered closed-engine binary semantics. Source paths establish the reported branches; native regressions must cover their precise equal-knowledge boundaries. The user's51 HOWTOSEE rows/art/audio/readability/animation/pacing/feel, ordinary human victory/defeat and packaged/platform launch quality remain **PENDING—requires user verification**. Those are separate from this report's correctness findings and do not themselves justify NO-GO.

## Preservation

This reviewer created no substantial directory outside the canonical repository. Only this report was written in the pre-existing approved reviews cache; it remains necessary for the fresh lead and subsequent remediation/review. No canonical source, index, refs, evidence or repository/worktree was changed or deleted. Configured CARGO_TARGET_DIR, UV_CACHE_DIR and PIP_CACHE_DIR were preserved. Final readback found the same frozen HEAD and only coordinator-owned untracked validation outputs.
