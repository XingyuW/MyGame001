// Native RA9 key controls; final validation and independent acceptance remain pending.
// Real BEFORE failures and AFTER results/fixture corrections are preserved separately in
// ra9-preliminary-logs/. Controlled fixture premises are stated at each scenario.
mod ra7_common;
use bevy_app::App;
use ra7_common::*;
use sim::{
    Cell, CellPos, Command, ForceTarget, Grid, Owner, Players, SimEvent, SimId, SpecialState,
};

fn hp(app: &mut App, id: SimId) -> Option<i32> {
    let e = entity(app, id)?;
    app.world().get::<sim::Health>(e).map(|h| h.hit_points)
}
fn owner(app: &mut App, id: SimId) -> Option<sim::PlayerId> {
    let e = entity(app, id)?;
    app.world().get::<Owner>(e).map(|o| o.0)
}
fn run(app: &mut App, n: usize) -> Vec<SimEvent> {
    let mut events = Vec::new();
    for _ in 0..n {
        events.extend(tick(app, vec![]));
    }
    events
}
fn no_rejections(events: &[SimEvent]) {
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, SimEvent::CommandRejected { .. })),
        "{events:?}"
    );
}
fn bomb(app: &mut App, target: SimId) -> bool {
    app.world()
        .resource::<SpecialState>()
        .charges
        .get(&target)
        .is_some_and(|c| !c.c4)
}

#[test]
fn ra9_typeimmune_dolphin_requires_same_kind_and_same_owner_but_allows_targeting() {
    // Separate worlds prevent control targets interfering with the measured shot.
    for (target_name, target_owner, immune) in [
        ("dolphin", P0, true),
        ("dolphin", P1, false),
        ("destroyer", P0, false),
        ("destroyer", P1, false),
    ] {
        let mut app = world();
        app.world_mut().insert_resource(
            Grid::from_cells(
                48,
                48,
                vec![
                    Cell {
                        passable: true,
                        water: true,
                        buildable: false,
                        ore: 0,
                    };
                    48 * 48
                ],
            )
            .expect("water grid"),
        );
        let source = put(&mut app, P0, "dolphin", 8, 8);
        let target = put(&mut app, target_owner, target_name, 10, 8);
        // Suppress defensive return fire using a pre-tick fixture setup change only.
        let e = entity(&mut app, target).expect("target");
        app.world_mut().entity_mut(e).remove::<sim::Weapon>();
        tick(&mut app, vec![]); // Real visibility/sensors, not forged view state.
        let before = hp(&mut app, target).expect("target alive");
        let mut events = tick(
            &mut app,
            vec![issue(
                P0,
                Command::ForceFire {
                    units: vec![source],
                    target: ForceTarget::Entity(target),
                },
            )],
        );
        // A genuine projectile is enough; stop before another reload can fire.
        events.extend(tick(
            &mut app,
            vec![issue(
                P0,
                Command::Stop {
                    units: vec![source],
                },
            )],
        ));
        events.extend(run(&mut app, 30));
        no_rejections(&events);
        assert!(
            events.iter().any(|e| matches!(e,SimEvent::WeaponFired {
            attacker,target:t,weapon,
        } if *attacker==source && *t==target &&
            *weapon==defs(&app).weapon_kind("sonic_zap").expect("weapon"))),
            "immunity must not reject targeting: {events:?}"
        );
        let after = hp(&mut app, target).expect("small shot cannot kill native target");
        if immune {
            assert_eq!(after, before, "same type AND same owner");
        } else {
            assert!(after < before, "control must take real damage");
        }
    }
}

#[test]
fn ra9_stock_yuri_immunity_is_masked_by_psionic_immunity_not_typeimmune_proof() {
    for target_owner in [P0, P1] {
        let mut app = world();
        let caster = put(&mut app, P0, "yuri", 10, 10);
        let other = put(&mut app, target_owner, "yuri", 11, 10);
        let victim = put(&mut app, P0, "gi", 11, 11);
        tick(&mut app, vec![]);
        let before = hp(&mut app, other);
        let events = tick(&mut app, vec![issue(P0, Command::Deploy { unit: caster })]);
        no_rejections(&events);
        assert!(
            events
                .iter()
                .any(|e| matches!(e,SimEvent::Impact { warhead,.. }
            if *warhead==defs(&app).warhead_kind("psi_pulse").expect("warhead")))
        );
        assert_eq!(
            hp(&mut app, other),
            before,
            "psychic immunity includes different owners"
        );
        assert_eq!(
            hp(&mut app, victim),
            None,
            "positive control: actual PsiWave kills GI"
        );
    }
    // A TypeImmune=false mutation is EXPECTED NOT to break this test. It is a dependency
    // witness for the masking disposition, not the causal test of TypeImmune itself.
}

#[test]
fn ra9_bombsight_four_detects_a_real_bomb_then_approaches_without_interact() {
    // ENGINEER/SENGINEER map to one game kind; both actual country/owner contexts run.
    for p in [P0, P1] {
        let mut app = world();
        let host = put(&mut app, p, "construction_yard", 16, 16);
        let ivan = put(&mut app, p, "crazy_ivan", 15, 16);
        let engineer = put(&mut app, p, "engineer", 11, 16); // nearest footprint distance 5
        tick(&mut app, vec![]);
        let events = tick(
            &mut app,
            vec![issue(
                p,
                Command::Interact {
                    unit: ivan,
                    target: host,
                },
            )],
        );
        no_rejections(&events);
        assert!(
            bomb(&mut app, host),
            "real plant before Engineer enters detection radius"
        );
        run(&mut app, 80);
        assert!(
            bomb(&mut app, host),
            "outside radius four must not select bomb"
        );
        let e = entity(&mut app, engineer).expect("engineer");
        assert_eq!(
            app.world().get::<sim::Mobile>(e).expect("mobile").cell,
            CellPos::new(11, 16)
        );
        let events = tick(
            &mut app,
            vec![issue(
                p,
                Command::Move {
                    units: vec![engineer],
                    target: CellPos::new(12, 16),
                },
            )],
        );
        no_rejections(&events);
        let before = hp(&mut app, host);
        let mut defused = false;
        for _ in 0..240 {
            tick(&mut app, vec![]);
            if !bomb(&mut app, host) {
                defused = true;
                break;
            }
        }
        assert!(
            defused,
            "entered radius four but never attempted an automatic defuse"
        );
        assert_eq!(
            hp(&mut app, host),
            before,
            "must defuse before real fuse damage"
        );
        assert_eq!(
            owner(&mut app, host),
            Some(p),
            "defuse must not capture host"
        );
        assert!(
            entity(&mut app, engineer).is_some(),
            "defuse does not consume Engineer"
        );
    }
}

#[test]
fn ra9_bombsight_does_not_know_an_enemy_spys_real_bomb() {
    let mut app = world();
    let host = put(&mut app, P1, "spy", 16, 16);
    let ivan = put(&mut app, P1, "crazy_ivan", 17, 16);
    let engineer = put(&mut app, P0, "engineer", 13, 16);
    tick(&mut app, vec![]);
    assert!(
        app.world()
            .resource::<Players>()
            .get(P0)
            .expect("observer")
            .vision()
            .is_visible(CellPos::new(16, 16)),
        "cell seen, host concealed"
    );
    assert!(
        app.world()
            .resource::<SpecialState>()
            .hidden
            .contains(&(P0, host))
    );
    let events = tick(
        &mut app,
        vec![issue(
            P1,
            Command::Interact {
                unit: ivan,
                target: host,
            },
        )],
    );
    no_rejections(&events);
    assert!(bomb(&mut app, host));
    run(&mut app, 120);
    assert!(
        bomb(&mut app, host),
        "automatic defuse must obey hidden-host policy"
    );
    assert!(
        !app.world()
            .resource::<SpecialState>()
            .interactions
            .contains_key(&engineer)
    );
    let e = entity(&mut app, engineer).expect("engineer");
    assert_eq!(
        app.world().get::<sim::Mobile>(e).expect("mobile").cell,
        CellPos::new(13, 16)
    );
    // This honest-visibility restriction is an explicit intended-model assumption; the
    // BombSight article alone does not prove native hidden-host behavior.
}

#[test]
fn ra9_automatic_defuse_skips_real_c4_and_explicit_ivan_defuse_still_works() {
    let mut app = world();
    let host = put(&mut app, P0, "construction_yard", 16, 16);
    let ivan = put(&mut app, P0, "crazy_ivan", 15, 16);
    let engineer = put(&mut app, P0, "engineer", 11, 16);
    tick(&mut app, vec![]);
    no_rejections(&tick(
        &mut app,
        vec![issue(
            P0,
            Command::Interact {
                unit: ivan,
                target: host,
            },
        )],
    ));
    assert!(bomb(&mut app, host));
    no_rejections(&tick(
        &mut app,
        vec![issue(
            P0,
            Command::Interact {
                unit: engineer,
                target: host,
            },
        )],
    ));
    run(&mut app, 220);
    assert!(!bomb(&mut app, host));
    assert_eq!(owner(&mut app, host), Some(P0));
    assert!(entity(&mut app, engineer).is_some());

    let mut app = world();
    let host = put(&mut app, P1, "construction_yard", 16, 16);
    let tanya = put(&mut app, P0, "tanya", 15, 16);
    let engineer = put(&mut app, P0, "engineer", 13, 16);
    tick(&mut app, vec![]);
    no_rejections(&tick(
        &mut app,
        vec![issue(
            P0,
            Command::Interact {
                unit: tanya,
                target: host,
            },
        )],
    ));
    assert!(
        app.world()
            .resource::<SpecialState>()
            .charges
            .get(&host)
            .is_some_and(|c| c.c4)
    );
    // Observe before C4's real short fuse ends. No fabricated charges/events.
    run(&mut app, 8);
    assert!(
        app.world()
            .resource::<SpecialState>()
            .charges
            .get(&host)
            .is_some_and(|c| c.c4)
    );
    assert!(
        !app.world()
            .resource::<SpecialState>()
            .interactions
            .contains_key(&engineer)
    );
}

#[test]
fn ra9_bombsight_unrelated_missing_and_genuinely_killed_hosts_create_no_jobs() {
    let mut app = world();
    let engineer = put(&mut app, P0, "engineer", 11, 16);
    let host = put(&mut app, P0, "gi", 16, 16);
    let unrelated = put(&mut app, P0, "construction_yard", 11, 19);
    let ivan = put(&mut app, P0, "crazy_ivan", 17, 16);
    let yuri = put(&mut app, P0, "yuri", 15, 17);
    tick(&mut app, vec![]);
    assert!(!bomb(&mut app, unrelated));
    no_rejections(&tick(
        &mut app,
        vec![issue(
            P0,
            Command::Interact {
                unit: ivan,
                target: host,
            },
        )],
    ));
    assert!(bomb(&mut app, host));
    no_rejections(&tick(
        &mut app,
        vec![issue(P0, Command::Deploy { unit: yuri })],
    ));
    assert_eq!(
        hp(&mut app, host),
        None,
        "real PsiWave damage destroys host in schedule"
    );
    no_rejections(&tick(
        &mut app,
        vec![issue(
            P0,
            Command::Move {
                units: vec![engineer],
                target: CellPos::new(14, 16),
            },
        )],
    ));
    run(&mut app, 100);
    assert!(!bomb(&mut app, host));
    assert!(!bomb(&mut app, unrelated));
    assert!(
        !app.world()
            .resource::<SpecialState>()
            .interactions
            .contains_key(&engineer)
    );
    let missing = SimId(u32::MAX - 17);
    let events = tick(
        &mut app,
        vec![issue(
            P0,
            Command::Interact {
                unit: engineer,
                target: missing,
            },
        )],
    );
    assert!(events.iter().any(|e| matches!(e,SimEvent::CommandRejected {
        reason:sim::RejectReason::UnknownTarget(id),..
    } if *id==missing)));
}

// UNVALIDATED / PENDING — source-only test bodies. Merge imports/helpers with sibling draft.
// Current reference major SWs are Capturable=false. A successful SW capture is not stock proof.

#[test]
fn ra9_real_construction_reveals_only_sight_and_hidden_targets_remain_hidden() {
    for (name, yard, lab, country) in [
        (
            "chronosphere",
            "construction_yard",
            "tech_center",
            "great_britain",
        ),
        (
            "weather_control",
            "construction_yard",
            "tech_center",
            "great_britain",
        ),
        (
            "iron_curtain",
            "soviet_construction_yard",
            "soviet_battle_lab",
            "russia",
        ),
        (
            "nuclear_silo",
            "soviet_construction_yard",
            "soviet_battle_lab",
            "russia",
        ),
    ] {
        for fog in [false, true] {
            let mut app = world();
            // Scenario setup only, before first tick. No fabricated production/history.
            let country = defs(&app)
                .rules()
                .expect("rules")
                .country_id(country)
                .expect("country");
            assert!(
                app.world_mut()
                    .resource_mut::<Players>()
                    .set_country(P0, Some(country))
            );
            let mut rules = app.world().resource::<sim::MatchState>().rules();
            rules.fog_of_war = fog;
            app.world_mut()
                .resource_mut::<sim::MatchState>()
                .set_rules(rules);
            put(&mut app, P0, yard, 8, 8);
            put(&mut app, P0, lab, 8, 12);
            put(&mut app, P0, "power_plant", 3, 8);
            put(&mut app, P0, "power_plant", 3, 11);
            let scout = put(&mut app, P1, "engineer", 40, 40);
            let spy = put(&mut app, P0, "spy", 15, 11);
            let kind = defs(&app).building_kind(name).expect("building kind");
            let item = sim::ProductionItem::Building(kind);
            let origin = CellPos::new(12, 8);
            let d = defs(&app);
            let building = d.building(kind).expect("building");
            let sight = d.building_combat(kind).expect("combat").sight as i32;
            // Point at exactly Sight from the nearest footprint cell, plus one outside.
            let edge = CellPos::new(origin.x + building.width as i32 - 1 + sight, origin.y);
            let outside = CellPos::new(edge.x + 1, edge.y);
            tick(&mut app, vec![]);
            assert!(
                !app.world()
                    .resource::<Players>()
                    .get(P1)
                    .expect("observer")
                    .vision()
                    .is_explored(origin)
            );
            let events = tick(&mut app, vec![issue(P0, Command::QueueProduction { item })]);
            no_rejections(&events);
            let mut completed = events
                .iter()
                .any(|e| matches!(e,SimEvent::ProductionComplete { item:i,.. } if *i==item));
            for _ in 0..10000 {
                if completed {
                    break;
                }
                let events = tick(&mut app, vec![]);
                completed = events
                    .iter()
                    .any(|e| matches!(e,SimEvent::ProductionComplete { item:i,.. } if *i==item));
            }
            assert!(completed, "native production must finish and be paid");
            assert!(
                !app.world()
                    .resource::<Players>()
                    .get(P1)
                    .expect("observer")
                    .vision()
                    .is_explored(origin),
                "queue completion alone must not reveal unplaced building"
            );
            let events = tick(
                &mut app,
                vec![issue(
                    P0,
                    Command::PlaceBuilding {
                        building: kind,
                        origin,
                    },
                )],
            );
            no_rejections(&events);
            assert!(
                events
                    .iter()
                    .any(|e| matches!(e,SimEvent::BuildingPlaced { kind:k,.. } if *k==kind))
            );
            assert!(
                events
                    .iter()
                    .any(|e| matches!(e,SimEvent::BuildingRevealed {
                player,kind:k,origin:o,..
            } if *player==P0 && *k==kind && *o==origin)),
                "typed public location event on actual placement tick"
            );
            let vision = app
                .world()
                .resource::<Players>()
                .get(P1)
                .expect("observer")
                .vision();
            assert!(
                vision.is_explored(origin),
                "real construction must reveal to distant opponent"
            );
            assert!(vision.is_explored(edge), "source Sight must bound reveal");
            assert!(
                !vision.is_explored(outside),
                "no extra global or source radius reveal"
            );
            tick(&mut app, vec![]);
            let vision = app
                .world()
                .resource::<Players>()
                .get(P1)
                .expect("observer")
                .vision();
            assert!(vision.is_explored(edge));
            assert_eq!(
                vision.is_visible(edge),
                !fog,
                "one reveal does not supply perpetual sight"
            );
            assert!(
                app.world()
                    .resource::<SpecialState>()
                    .hidden
                    .contains(&(P1, spy))
            );
            let events = tick(
                &mut app,
                vec![issue(
                    P1,
                    Command::Follow {
                        units: vec![scout],
                        target: spy,
                    },
                )],
            );
            assert!(
                events.iter().any(|e| matches!(e,SimEvent::CommandRejected {
                reason:sim::RejectReason::UnknownTarget(id),..
            } if *id==spy)),
                "global reveal must not disclose concealed spy: {events:?}"
            );
            assert!(
                !events
                    .iter()
                    .any(|e| matches!(e,SimEvent::BuildingRevealed {
                player,kind:k,origin:o,..
            } if *player==P0 && *k==kind && *o==origin)),
                "already announced construction must not reannounce on a later unrelated command"
            );
        }
    }
}

#[test]
fn ra9_stock_uncapturable_major_superweapons_refuse_engineer_but_power_plant_captures() {
    for name in [
        "chronosphere",
        "weather_control",
        "iron_curtain",
        "nuclear_silo",
        "power_plant",
    ] {
        let mut app = world();
        let host = put(&mut app, P1, name, 16, 16);
        let engineer = put(&mut app, P0, "engineer", 15, 16);
        tick(&mut app, vec![]);
        let events = tick(
            &mut app,
            vec![issue(
                P0,
                Command::Interact {
                    unit: engineer,
                    target: host,
                },
            )],
        );
        if name == "power_plant" {
            no_rejections(&events);
            assert_eq!(owner(&mut app, host), Some(P0));
            assert!(
                entity(&mut app, engineer).is_none(),
                "real capture consumes Engineer"
            );
        } else {
            assert!(
                events
                    .iter()
                    .any(|e| matches!(e, SimEvent::CommandRejected { .. })),
                "reference Capturable=false must refuse capture: {name}: {events:?}"
            );
            assert_eq!(owner(&mut app, host), Some(P1));
            assert!(entity(&mut app, engineer).is_some());
        }
    }
}

#[test]
fn ra9_neutral_major_superweapon_does_not_reveal_distant_cell() {
    for name in [
        "chronosphere",
        "weather_control",
        "iron_curtain",
        "nuclear_silo",
    ] {
        let mut app = world();
        let host = put(&mut app, sim::NEUTRAL, name, 16, 16);
        put(&mut app, P0, "engineer", 40, 40);
        put(&mut app, P1, "engineer", 43, 43);
        let events = tick(&mut app, vec![]);
        for p in [P0, P1] {
            assert!(
                !app.world()
                    .resource::<Players>()
                    .get(p)
                    .expect("observer")
                    .vision()
                    .is_explored(CellPos::new(16, 16))
            );
        }
        assert_eq!(owner(&mut app, host), Some(sim::NEUTRAL));
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, SimEvent::SuperweaponBuilt { .. }))
        );
    }
}

// REQUIRED BEFORE CLAIMING GENERIC CAPTURE COVERAGE (not a counterfeit shipped test):
// A owns creation of an explicit minimal RA2-rules fixture with a capturable building,
// reveal_to_all=1, sight=4 and a regular capturable=1/reveal_to_all=0 control; both have
// positive source-bounded HP. Pre-tick neutral host and two remote players; capture by a
// real adjacent Engineer must consume it and reveal edge but not outside to remote P1.
// Capture again by a second pre-tick P1 Engineer initially far away, moving via commands,
// and observe second genuine capture broadcast; no world Owner rewrites. Repeat with
// reveal_to_all=0 and with current fog on/off. Generic capture is PENDING that fixture.

#[test]
fn ra9_a_dolphin_shot_keeps_its_type_and_launch_owner_after_native_source_death() {
    let mut app = world();
    app.world_mut().insert_resource(
        Grid::from_cells(
            48,
            48,
            vec![
                Cell {
                    passable: true,
                    water: true,
                    buildable: false,
                    ore: 0,
                };
                48 * 48
            ],
        )
        .expect("water"),
    );
    let source = put(&mut app, P0, "dolphin", 8, 8);
    let target = put(&mut app, P0, "dolphin", 13, 8);
    let killer = put(&mut app, P1, "dolphin", 8, 10);
    // Scenario-only valid native HP; real enemy damage, not a post-tick deletion.
    let e = entity(&mut app, source).expect("source");
    app.world_mut()
        .get_mut::<sim::Health>(e)
        .expect("health")
        .hit_points = 1;
    let e = entity(&mut app, target).expect("target");
    app.world_mut().entity_mut(e).remove::<sim::Weapon>();
    tick(&mut app, vec![]);
    let before = hp(&mut app, target);
    let events = tick(
        &mut app,
        vec![
            issue(
                P0,
                Command::ForceFire {
                    units: vec![source],
                    target: ForceTarget::Entity(target),
                },
            ),
            issue(
                P1,
                Command::ForceFire {
                    units: vec![killer],
                    target: ForceTarget::Entity(source),
                },
            ),
        ],
    );
    no_rejections(&events);
    assert!(
        events
            .iter()
            .any(|e| matches!(e,SimEvent::WeaponFired {attacker,target:t,..}
        if *attacker==source&&*t==target))
    );
    let mut dead_while_in_flight = false;
    for _ in 0..12 {
        tick(&mut app, vec![]);
        let dead = entity(&mut app, source).is_none();
        let flight = app
            .world()
            .resource::<sim::Projectiles>()
            .iter()
            .any(|p| p.attacker == source && p.target == target && p.remaining > 0);
        dead_while_in_flight |= dead && flight;
    }
    assert!(
        dead_while_in_flight,
        "fixture must witness source Cleanup before its impact"
    );
    assert_eq!(
        hp(&mut app, target),
        before,
        "do not look up only a still-live shooter"
    );
}

#[test]
fn ra9_stale_automatic_defuse_cannot_capture_a_host_after_another_real_engineer_defuses() {
    let mut app = world();
    let auto = put(&mut app, P0, "engineer", 12, 16);
    let host = put(&mut app, P1, "power_plant", 16, 16);
    let ivan = put(&mut app, P1, "crazy_ivan", 15, 16);
    let explicit = put(&mut app, P1, "engineer", 15, 17);
    tick(&mut app, vec![]);
    no_rejections(&tick(
        &mut app,
        vec![issue(
            P1,
            Command::Guard {
                units: vec![explicit],
            },
        )],
    ));
    no_rejections(&tick(
        &mut app,
        vec![issue(
            P1,
            Command::Interact {
                unit: ivan,
                target: host,
            },
        )],
    ));
    assert!(bomb(&mut app, host));
    assert_eq!(
        app.world()
            .resource::<SpecialState>()
            .auto_defuses
            .get(&auto),
        Some(&(host, P0))
    );
    no_rejections(&tick(
        &mut app,
        vec![issue(
            P1,
            Command::Interact {
                unit: explicit,
                target: host,
            },
        )],
    ));
    assert!(!bomb(&mut app, host));
    run(&mut app, 220);
    assert_eq!(owner(&mut app, host), Some(P1));
    assert!(entity(&mut app, auto).is_some());
    assert!(entity(&mut app, explicit).is_some());
    assert!(
        !app.world()
            .resource::<SpecialState>()
            .auto_defuses
            .contains_key(&auto)
    );
}

#[test]
fn ra9_guard_and_replacement_move_keep_command_priority_over_automatic_defuse() {
    let mut app = world();
    let host = put(&mut app, P0, "power_plant", 16, 16);
    let ivan = put(&mut app, P0, "crazy_ivan", 15, 16);
    let engineer = put(&mut app, P0, "engineer", 12, 16);
    tick(&mut app, vec![]);
    no_rejections(&tick(
        &mut app,
        vec![issue(
            P0,
            Command::Guard {
                units: vec![engineer],
            },
        )],
    ));
    no_rejections(&tick(
        &mut app,
        vec![issue(
            P0,
            Command::Interact {
                unit: ivan,
                target: host,
            },
        )],
    ));
    assert!(bomb(&mut app, host));
    assert!(
        !app.world()
            .resource::<SpecialState>()
            .auto_defuses
            .contains_key(&engineer)
    );
    // Stop releases Guard; same-tick idle discovery is allowed when no prior auto job exists.
    no_rejections(&tick(
        &mut app,
        vec![issue(
            P0,
            Command::Stop {
                units: vec![engineer],
            },
        )],
    ));
    assert!(
        app.world()
            .resource::<SpecialState>()
            .auto_defuses
            .contains_key(&engineer)
    );
    no_rejections(&tick(
        &mut app,
        vec![issue(
            P0,
            Command::Move {
                units: vec![engineer],
                target: CellPos::new(4, 16),
            },
        )],
    ));
    assert!(
        !app.world()
            .resource::<SpecialState>()
            .auto_defuses
            .contains_key(&engineer)
    );
    let speed = defs(&app)
        .unit(defs(&app).unit_kind("engineer").expect("kind"))
        .expect("unit")
        .speed;
    assert_eq!(
        speed, 11,
        "shipped movement speed, not an arbitrary arrival deadline"
    );
    // Replacement orders preserve a step already committed by automatic approach.
    // Read the real post-command state: finish that eastward step, then traverse all
    // nine westward cells. Infantry standing points use the same scalar step length.
    let e = entity(&mut app, engineer).expect("engineer");
    let mobile = *app.world().get::<sim::Mobile>(e).expect("mobile");
    assert_eq!(mobile.cell, CellPos::new(12, 16));
    let committed = mobile
        .step
        .expect("real eastward auto-approach step remains committed");
    assert_eq!(committed.to, CellPos::new(13, 16));
    assert_eq!(committed.length, sim::ORTHOGONAL_STEP);
    assert!(committed.progress < committed.length);
    let target = CellPos::new(4, 16);
    assert_eq!(committed.to.y, target.y);
    let west_cells = committed.to.x.abs_diff(target.x);
    assert_eq!(west_cells, 9);
    let remaining = u32::from(committed.length - committed.progress)
        + west_cells * u32::from(sim::ORTHOGONAL_STEP);
    let budget = remaining.div_ceil(u32::from(speed)) + 4;
    assert!(
        budget < 600,
        "complete actual route must precede the real fuse"
    );
    for _ in 0..budget {
        tick(&mut app, vec![]);
        assert!(bomb(&mut app, host));
        assert!(
            !app.world()
                .resource::<SpecialState>()
                .auto_defuses
                .contains_key(&engineer)
        );
    }
    let e = entity(&mut app, engineer).expect("engineer");
    assert_eq!(
        app.world().get::<sim::Mobile>(e).expect("mobile").cell,
        CellPos::new(4, 16)
    );
}

#[test]
fn ra9_public_reveal_is_once_per_surviving_owner_and_gap_still_suppresses_sight() {
    for fog in [false, true] {
        let mut app = world();
        let mut rules = app.world().resource::<sim::MatchState>().rules();
        rules.fog_of_war = fog;
        app.world_mut()
            .resource_mut::<sim::MatchState>()
            .set_rules(rules);
        let host = put(&mut app, P0, "chronosphere", 16, 16);
        // No nearby P1 local sight source: reconnaissance restores local sight after Gap.
        // P0 owns the powered hostile Gap; the P1 observer receives no local counter-reveal.
        put(&mut app, P0, "gap_generator", 19, 20);
        put(&mut app, P0, "power_plant", 30, 32);
        put(&mut app, P0, "power_plant", 32, 32);
        let events = tick(&mut app, vec![]);
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e,SimEvent::BuildingRevealed {
            building,..} if *building==host))
                .count(),
            1
        );
        assert!(
            !app.world()
                .resource::<Players>()
                .get(P1)
                .expect("observer")
                .vision()
                .is_visible(CellPos::new(16, 16)),
            "hostile powered Gap applies after public reveal"
        );
        let events = tick(&mut app, vec![]);
        assert!(
            !events
                .iter()
                .any(|e| matches!(e,SimEvent::BuildingRevealed {
            building,..} if *building==host)),
            "no permanent reveal source"
        );
    }
}

// Independent synthetic RA4-fixture variants: never change stock SW capturability.
mod ra4_common;
fn generic_reveal_definitions(flag: bool) -> sim::Definitions {
    let path = ra4_common::fixture("arsenal.ron");
    let text = std::fs::read_to_string(path).expect("migrated fixture");
    let mut found = 0;
    let variant = text
        .lines()
        .map(|line| {
            if line.contains(r#"(id: "airfield","#) {
                found += 1;
                assert!(
                    line.contains("capturable: 1"),
                    "A's named fixture migration is required"
                );
                assert!(line.contains("reveal_to_all: 0"), "schema10 explicit field");
                line.replace(
                    "reveal_to_all: 0",
                    if flag {
                        "reveal_to_all: 1"
                    } else {
                        "reveal_to_all: 0"
                    },
                )
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(found, 1);
    let tmp = write_key_fixture("generic-reveal", &variant);
    let result = ra4_common::definitions().with_arsenal(&tmp);
    std::fs::remove_file(tmp).expect("remove before asserting load result");
    result.unwrap_or_else(|e| panic!("{e}"))
}
#[test]
fn ra9_a_genuinely_capturable_fixture_broadcasts_each_real_capture_only_when_flagged() {
    for flag in [false, true] {
        for fog in [false, true] {
            let mut app = ra4_common::world_of(generic_reveal_definitions(flag), 7);
            let mut rules = app.world().resource::<sim::MatchState>().rules();
            rules.fog_of_war = fog;
            app.world_mut()
                .resource_mut::<sim::MatchState>()
                .set_rules(rules);
            let host = ra4_common::place(&mut app, sim::NEUTRAL, "airfield", 16, 16);
            let first = ra4_common::spawn(&mut app, P0, "engineer", 15, 16);
            let second = ra4_common::spawn(&mut app, P1, "engineer", 35, 35);
            let kind = defs(&app).building_kind("airfield").expect("kind");
            let b = defs(&app).building(kind).expect("building").clone();
            let sight = defs(&app).building_combat(kind).expect("combat").sight as i32;
            let edge = CellPos::new(16 + i32::from(b.width) - 1 + sight, 16);
            let outside = CellPos::new(edge.x + 1, edge.y);
            tick(&mut app, vec![]);
            let events = tick(
                &mut app,
                vec![issue(
                    P0,
                    Command::Interact {
                        unit: first,
                        target: host,
                    },
                )],
            );
            no_rejections(&events);
            assert_eq!(owner(&mut app, host), Some(P0));
            assert!(entity(&mut app, first).is_none());
            let v = app
                .world()
                .resource::<Players>()
                .get(P1)
                .expect("observer")
                .vision();
            assert_eq!(v.is_explored(edge), flag);
            assert!(!v.is_explored(outside));
            assert_eq!(
                events
                    .iter()
                    .any(|e| matches!(e,SimEvent::BuildingRevealed {
            building,player,..} if *building==host&&*player==P0)),
                flag
            );
            // Second actor exists before tick0 and travels by genuine movement.
            no_rejections(&tick(
                &mut app,
                vec![issue(
                    P1,
                    Command::Move {
                        units: vec![second],
                        target: CellPos::new(15, 16),
                    },
                )],
            ));
            run(&mut app, 1200);
            let events = tick(
                &mut app,
                vec![issue(
                    P1,
                    Command::Interact {
                        unit: second,
                        target: host,
                    },
                )],
            );
            no_rejections(&events);
            assert_eq!(owner(&mut app, host), Some(P1));
            assert!(entity(&mut app, second).is_none());
            assert_eq!(
                events
                    .iter()
                    .any(|e| matches!(e,SimEvent::BuildingRevealed {
            building,player,..} if *building==host&&*player==P1)),
                flag
            );
        }
    }
}

#[test]
fn ra9_new_state_hashes_are_independent_helper_witnesses_not_runtime_proof() {
    let mut app = world();
    let a = put(&mut app, P0, "engineer", 8, 8);
    let b = put(&mut app, P1, "power_plant", 12, 8);
    let base = sim::state_hash(app.world_mut());
    app.world_mut()
        .resource_mut::<SpecialState>()
        .auto_defuses
        .insert(a, (b, P0));
    let job = sim::state_hash(app.world_mut());
    assert_ne!(base, job);
    app.world_mut()
        .resource_mut::<SpecialState>()
        .auto_defuses
        .insert(a, (b, P1));
    assert_ne!(job, sim::state_hash(app.world_mut()));
    app.world_mut()
        .resource_mut::<SpecialState>()
        .auto_defuses
        .clear();
    assert_eq!(base, sim::state_hash(app.world_mut()));
    app.world_mut()
        .resource_mut::<SpecialState>()
        .auto_defuse_cancelled
        .insert(a, 0);
    assert_ne!(base, sim::state_hash(app.world_mut()));
    app.world_mut()
        .resource_mut::<SpecialState>()
        .auto_defuse_cancelled
        .clear();
    assert_eq!(base, sim::state_hash(app.world_mut()));
    app.world_mut()
        .resource_mut::<SpecialState>()
        .revealed_owners
        .insert(b, P0);
    let reveal = sim::state_hash(app.world_mut());
    assert_ne!(base, reveal);
    app.world_mut()
        .resource_mut::<SpecialState>()
        .revealed_owners
        .insert(b, P1);
    assert_ne!(reveal, sim::state_hash(app.world_mut()));
}

// Controlled parser fixture: stock Yuri's NavalTargeting=0 excludes an underwater
// Dolphin. Permit this exact target layer only to exercise real MindControl ownership
// between a real stock SonicZap launch and impact; this is not stock Yuri/naval fidelity.
fn takeover_fixture_definitions() -> sim::Definitions {
    let source = std::fs::read_to_string(data_dir().join("combat.ron")).expect("committed combat");
    let mut found = 0;
    let text = source
        .lines()
        .map(|line| {
            if line.contains(r#"(unit: "yuri","#) {
                found += 1;
                assert_eq!(line.matches("naval_targeting: 0").count(), 1);
                line.replace("naval_targeting: 0", "naval_targeting: 5")
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(found, 1);
    let definitions = shipped();
    let kind = definitions.unit_kind("yuri").expect("Yuri");
    assert_eq!(
        definitions
            .unit_combat(kind)
            .expect("terms")
            .naval_targeting,
        0
    );
    let path = write_key_fixture("takeover", &text);
    let parsed = definitions.with_combat(&path);
    std::fs::remove_file(path).expect("remove only this invocation's parser fixture");
    let parsed = parsed.expect("controlled target-layer fixture");
    assert_eq!(parsed.unit_combat(kind).expect("terms").naval_targeting, 5);
    parsed
}
fn write_key_fixture(label: &str, text: &str) -> std::path::PathBuf {
    use std::io::Write;
    let path = unique_key_fixture(label);
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .expect("exclusive per-invocation parser fixture");
    file.write_all(text.as_bytes()).expect("fixture bytes");
    path
}
fn unique_key_fixture(label: &str) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    std::path::PathBuf::from(std::env::var_os("TMPDIR").expect("configured TMPDIR")).join(format!(
        "ra9-key-{label}-{}-{}.ron",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

#[test]
fn ra9_a_real_midflight_takeover_preserves_dolphin_launch_owner() {
    for (victim_owner, immune) in [(P0, true), (P1, false)] {
        let mut app = world();
        app.world_mut()
            .insert_resource(takeover_fixture_definitions());
        app.world_mut().insert_resource(
            Grid::from_cells(
                48,
                48,
                vec![
                    Cell {
                        passable: true,
                        water: true,
                        buildable: false,
                        ore: 0,
                    };
                    48 * 48
                ],
            )
            .expect("sea"),
        );
        app.world_mut().resource_mut::<Grid>().set_terrain(
            CellPos::new(8, 7),
            Cell {
                passable: true,
                water: false,
                buildable: true,
                ore: 0,
            },
        );
        let source = put(&mut app, P0, "dolphin", 8, 8);
        let target = put(&mut app, victim_owner, "dolphin", 12, 8);
        let controller = put(&mut app, P1, "yuri", 8, 7);
        let sensor = put(&mut app, P1, "dolphin", 8, 10);
        // Pre-first-tick clock setup prevents any implicit first-tick control before
        // the measured simultaneous launch/takeover. The next native tick makes it due.
        let caster_entity = entity(&mut app, controller).expect("controller");
        app.world_mut()
            .get_mut::<sim::Weapon>(caster_entity)
            .expect("MindControl weapon")
            .cooldown = 2;
        for id in [target, sensor] {
            let e = entity(&mut app, id).expect("actor");
            app.world_mut().entity_mut(e).remove::<sim::Weapon>();
        }
        tick(&mut app, vec![]);
        assert_eq!(
            owner(&mut app, source),
            Some(P0),
            "not controlled before measured launch"
        );
        assert_eq!(
            app.world()
                .get::<sim::Weapon>(caster_entity)
                .expect("controller")
                .cooldown,
            1
        );
        let before = hp(&mut app, target);
        let events = tick(
            &mut app,
            vec![
                issue(
                    P0,
                    Command::ForceFire {
                        units: vec![source],
                        target: ForceTarget::Entity(target),
                    },
                ),
                issue(
                    P1,
                    Command::Attack {
                        units: vec![controller],
                        target: source,
                    },
                ),
            ],
        );
        no_rejections(&events);
        assert!(events.iter().any(|e| matches!(e,SimEvent::WeaponFired {
            attacker,target:t,..} if *attacker==source&&*t==target)));
        assert!(events.iter().any(|e|matches!(e,SimEvent::WeaponFired { attacker,target:t,weapon }
            if *attacker==controller&&*t==source&&*weapon==defs(&app).weapon_kind("mind_control").expect("weapon"))),
            "actual selected MindControl shot must cause ownership transition: {events:?}");
        assert_eq!(
            owner(&mut app, source),
            Some(P1),
            "real MindControl takeover before impact"
        );
        assert!(
            app.world()
                .resource::<sim::Projectiles>()
                .iter()
                .any(|p| p.attacker == source
                    && p.target == target
                    && p.owner == P0
                    && p.remaining > 0)
        );
        run(&mut app, 20);
        if immune {
            assert_eq!(hp(&mut app, target), before);
        } else {
            assert!(hp(&mut app, target) < before);
        }
    }
}

// UNVALIDATED: merge into the native test file after schema10 fixture migration.
// Parser adapters and independent field/hash witnesses; no gameplay closure is claimed.
fn key_field_variant(field: &str, value: Option<&str>) -> Result<sim::Definitions, sim::DataError> {
    let text = std::fs::read_to_string(ra4_common::fixture("arsenal.ron")).expect("fixture");
    let needle = format!("{field}: 0");
    let mut count = 0;
    let text = text
        .lines()
        .map(|line| {
            if line.contains(r#"(id: "airfield","#) {
                count += 1;
                assert_eq!(line.matches(&needle).count(), 1);
                match value {
                    Some(v) => line.replace(&needle, &format!("{field}: {v}")),
                    None => line.replace(&format!("{needle}, "), ""),
                }
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(count, 1);
    let path = write_key_fixture(field, &text);
    let result = ra4_common::definitions().with_arsenal(&path);
    std::fs::remove_file(path).expect("remove before result assertions");
    result
}
#[test]
fn ra9_key_fields_are_required_bounded_and_independently_hashed() {
    for field in ["type_immune", "bomb_sight", "reveal_to_all"] {
        let max = if field == "bomb_sight" {
            sim::MAX_SIGHT
        } else {
            1
        };
        for bad in [None, Some("-1"), Some("\"wrong\"")] {
            assert!(key_field_variant(field, bad).is_err(), "{field}:{bad:?}");
        }
        assert!(key_field_variant(field, Some(&max.saturating_add(1).to_string())).is_err());
        assert!(key_field_variant(field, Some(&max.to_string())).is_ok());
        let definitions = key_field_variant(field, Some("0")).expect("base");
        let mut a = ra4_common::world_of(definitions, 7);
        let base = sim::state_hash(a.world_mut());
        let definitions = key_field_variant(field, Some("1")).expect("single field variant");
        let item =
            sim::ProductionItem::Building(definitions.building_kind("airfield").expect("kind"));
        let m = definitions.metadata(item).expect("metadata");
        match field {
            "type_immune" => assert!(m.type_immune),
            "bomb_sight" => assert_eq!(m.bomb_sight, 1),
            "reveal_to_all" => assert!(m.reveal_to_all),
            _ => unreachable!("fixed list"),
        }
        let mut b = ra4_common::world_of(definitions, 7);
        assert_ne!(
            base,
            sim::state_hash(b.world_mut()),
            "{field} must reach Definitions hash"
        );
    }
}

// UNVALIDATED: append to native-final test file; requires S scenery integration.
#[test]
fn ra9_automatic_bombsight_defuses_real_occupied_actorless_mapped_props() {
    for (feature, name) in [
        (5, "civilian_house"),
        (6, "civilian_city"),
        (7, "civilian_tower"),
    ] {
        let mut app = world();
        let cell = CellPos::new(20, 20);
        app.world_mut().resource_mut::<Grid>().set_terrain(
            cell,
            Cell {
                passable: false,
                buildable: false,
                water: false,
                ore: 0,
            },
        );
        let mut looks = vec![0; 48 * 48];
        looks[20 * 48 + 20] = feature << 4;
        let d = defs(&app);
        let kind = d.building_kind(name).expect("mapped building");
        let props = sim::Props::from_looks(
            app.world().resource::<Grid>(),
            &looks,
            d.building_combat(kind).expect("combat").armour,
        );
        let host = props.id_at(cell).expect("prop");
        app.world_mut().insert_resource(props);
        let occupant = put(&mut app, P0, "conscript", 19, 21);
        let ivan = put(&mut app, P0, "crazy_ivan", 19, 20);
        let engineer = put(&mut app, P0, "engineer", 15, 20);
        tick(&mut app, vec![]);
        no_rejections(&tick(
            &mut app,
            vec![issue(
                P0,
                Command::Interact {
                    unit: occupant,
                    target: host,
                },
            )],
        ));
        assert!(entity(&mut app, occupant).is_none());
        assert_eq!(
            app.world()
                .resource::<SpecialState>()
                .garrisons
                .get(&host)
                .expect("real entry")
                .owner,
            P0
        );
        assert!(
            entity(&mut app, host).is_none(),
            "actual actorless target, no fabricated Actor"
        );
        no_rejections(&tick(
            &mut app,
            vec![issue(
                P0,
                Command::Interact {
                    unit: ivan,
                    target: host,
                },
            )],
        ));
        assert!(bomb(&mut app, host));
        let before = app
            .world()
            .resource::<sim::Props>()
            .get(host)
            .expect("host")
            .1
            .hit_points;
        run(&mut app, 80);
        assert!(bomb(&mut app, host), "outside detection radius");
        no_rejections(&tick(
            &mut app,
            vec![issue(
                P0,
                Command::Move {
                    units: vec![engineer],
                    target: CellPos::new(16, 20),
                },
            )],
        ));
        let mut defused = false;
        for _ in 0..240 {
            tick(&mut app, vec![]);
            if !bomb(&mut app, host) {
                defused = true;
                break;
            }
        }
        assert!(defused);
        assert!(entity(&mut app, engineer).is_some());
        assert_eq!(
            app.world()
                .resource::<sim::Props>()
                .get(host)
                .expect("host survives")
                .1
                .hit_points,
            before
        );
        assert_eq!(
            app.world()
                .resource::<SpecialState>()
                .garrisons
                .get(&host)
                .expect("occupation preserved")
                .owner,
            P0
        );
        assert!(sim::cell_invariant_violations(app.world_mut()).is_empty());
    }
}

#[test]
fn ra9_bombsight_known_non_c4_charges_ignore_same_allied_or_hostile_ivan_ownership() {
    for (label, planter, allied) in [
        ("same", P0, false),
        ("allied", P1, true),
        ("hostile", P1, false),
    ] {
        let mut app = world();
        if allied {
            assert!(app.world_mut().resource_mut::<Players>().set_team(P0, 1));
            assert!(app.world_mut().resource_mut::<Players>().set_team(P1, 1));
        }
        assert_eq!(
            app.world().resource::<Players>().hostile(P0, planter),
            label == "hostile"
        );
        let host = put(&mut app, planter, "construction_yard", 16, 16);
        let ivan = put(&mut app, planter, "crazy_ivan", 15, 16);
        let engineer = put(&mut app, P0, "engineer", 11, 16);
        // An unarmed actual MCV supplies sight; the Engineer remains outside BombSight4.
        let scout = put(&mut app, P0, "mcv", 11, 13);
        let d = defs(&app);
        let scout_kind = d.unit_kind("mcv").expect("loaded MCV");
        assert!(
            d.unit_combat(scout_kind)
                .expect("combat")
                .weapons(false)
                .into_iter()
                .all(|w| w.is_none())
        );
        tick(&mut app, vec![]);
        no_rejections(&tick(
            &mut app,
            vec![issue(
                planter,
                Command::Interact {
                    unit: ivan,
                    target: host,
                },
            )],
        ));
        let charge = app
            .world()
            .resource::<SpecialState>()
            .charges
            .get(&host)
            .expect("actual native plant");
        assert!(!charge.c4);
        assert_eq!(charge.source, ivan);
        let due = charge.due;
        assert_eq!(owner(&mut app, ivan), Some(planter));
        let before = hp(&mut app, host);
        no_rejections(&tick(
            &mut app,
            vec![issue(
                planter,
                Command::Move {
                    units: vec![ivan],
                    target: CellPos::new(5, 16),
                },
            )],
        ));
        assert!(
            bomb(&mut app, host),
            "outside detection must preserve real charge: {label}"
        );
        no_rejections(&tick(
            &mut app,
            vec![issue(
                P0,
                Command::Move {
                    units: vec![engineer],
                    target: CellPos::new(12, 16),
                },
            )],
        ));
        let mut defused = false;
        for _ in 0..240 {
            assert!(
                app.world().resource::<sim::SimTick>().0 < due,
                "defuse must precede natural expiry"
            );
            tick(&mut app, vec![]);
            if !bomb(&mut app, host) {
                defused = true;
                break;
            }
        }
        assert!(
            defused,
            "known non-C4 charge owner must not prevent automatic defuse: {label}"
        );
        assert_eq!(hp(&mut app, host), before);
        assert_eq!(owner(&mut app, host), Some(planter));
        assert!(entity(&mut app, engineer).is_some());
        assert!(entity(&mut app, scout).is_some());
        assert!(sim::cell_invariant_violations(app.world_mut()).is_empty());
    }
}
