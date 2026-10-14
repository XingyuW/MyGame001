//! Regression tests of advisory review 1 (docs/reviews/RA2-parity-advisory-1-sim.md P0-1, P1-1 and
//! its first suspected item; docs/DECISIONS.md R56), on the RA4 fixture (`tests/fixtures/data/ra4`;
//! invariant I10), through `CommandQueue` and `SimSchedule`:
//!
//! * a unit or building whose hit points a command of the tick took (a nuclear strike lands in
//!   the `Commands` stage) is gone for every command after it and for every order waiting to be
//!   carried out: it is not deployed, undeployed, sold, repaired, moved, embarked, captured or
//!   fired, it is no prerequisite, factory or anchor, and `Cleanup` removes it;
//! * a command that names a unit or building of another player that the issuing player does not
//!   see, or that is hidden from it, is rejected exactly as one that names nothing (decisions
//!   D30 and D33), whatever the command.
//!
//! In the fixture the nuclear strike is 300 through `megaton` (4 cells, 10 percent at the edge;
//! flesh 100, plate and hull 40, concrete 10 percent) and a silo charges in 20 ticks. The low
//! hit points of the victims stand for earlier damage (scenario setup); the lethal damage and
//! every command go through the production path.

mod ra4_common;

use bevy_app::App;
use ra4_common::*;
use sim::{
    Building, CellPos, Command, IssuedCommand, Owner, PlayerId, Players, ProductionItem,
    RejectReason, SimEvent, SimId, SpecialState, Unit,
};

/// The point every strike of these tests lands on.
const GROUND_ZERO: (i32, i32) = (20, 20);

/// Whether `id` is a unit, and whether it is a building.
fn exists_as(app: &mut App, id: SimId) -> (bool, bool) {
    let world = app.world_mut();
    let unit = world
        .query::<(&SimId, &Unit)>()
        .iter(world)
        .any(|(found, _)| *found == id);
    let building = world
        .query::<(&SimId, &Building)>()
        .iter(world)
        .any(|(found, _)| *found == id);
    (unit, building)
}

fn owner_of(app: &mut App, id: SimId) -> Option<PlayerId> {
    let world = app.world_mut();
    world
        .query::<(&SimId, &Owner)>()
        .iter(world)
        .find(|(found, _)| **found == id)
        .map(|(_, owner)| owner.0)
}

fn credits(app: &App, player: PlayerId) -> i64 {
    app.world()
        .resource::<Players>()
        .get(player)
        .expect("player")
        .credits()
}

/// A charged silo of P0 in the corner and an engineer of P0 who sees ground zero from five
/// cells, outside the blast.
fn charged_silo(app: &mut App) -> SimId {
    let silo = place(app, P0, "silo", 2, 2);
    spawn(app, P0, "engineer", GROUND_ZERO.0 - 5, GROUND_ZERO.1);
    ticks(app, 25);
    silo
}

fn fire(player: PlayerId, silo: SimId, (x, y): (i32, i32)) -> IssuedCommand {
    issue(
        player,
        Command::Superweapon {
            building: silo,
            target: CellPos::new(x, y),
            units: Vec::new(),
        },
    )
}

fn strike(silo: SimId) -> IssuedCommand {
    fire(P0, silo, GROUND_ZERO)
}

fn destroyed(events: &[SimEvent], id: SimId) -> bool {
    events.iter().any(|event| match event {
        SimEvent::UnitDestroyed { unit, .. } => *unit == id,
        SimEvent::BuildingDestroyed { building, .. } => *building == id,
        _ => false,
    })
}

fn building_kind(app: &App, id: &str) -> sim::BuildingKind {
    defs(app)
        .building_kind(id)
        .unwrap_or_else(|| panic!("{id}"))
}

fn unit_item(app: &App, id: &str) -> ProductionItem {
    ProductionItem::Unit(defs(app).unit_kind(id).unwrap_or_else(|| panic!("{id}")))
}

// --- P0-1: what a command of the tick destroyed is gone for the commands after it -----------------

#[test]
fn a_weak_yard_dies_in_the_strike() {
    // The control of the tests below: the strike alone destroys the yard in its tick.
    let mut app = world();
    let yard = place(&mut app, P1, "soviet_yard", 19, 19);
    let silo = charged_silo(&mut app);
    set_hp(&mut app, yard, 20);
    let events = tick(&mut app, vec![strike(silo)]);
    assert!(rejections(&events).is_empty(), "{events:?}");
    assert!(destroyed(&events, yard));
    assert_eq!(exists_as(&mut app, yard), (false, false));
}

#[test]
fn a_yard_destroyed_by_a_strike_is_not_undeployed_in_the_same_tick() {
    // Regression (advisory review 1, sim P0-1): the yard became a base unit with 1 hit point.
    let mut app = world();
    let yard = place(&mut app, P1, "soviet_yard", 19, 19);
    let silo = charged_silo(&mut app);
    set_hp(&mut app, yard, 20);
    let events = tick(
        &mut app,
        vec![
            strike(silo),
            issue(P1, Command::Undeploy { building: yard }),
        ],
    );
    assert_eq!(rejections(&events), [RejectReason::UnknownBuilding(yard)]);
    assert!(destroyed(&events, yard), "{events:?}");
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, SimEvent::BuildingUndeployed { .. })),
        "{events:?}"
    );
    assert_eq!(exists_as(&mut app, yard), (false, false));
    ticks(&mut app, 3);
    assert_eq!(exists_as(&mut app, yard), (false, false));
    assert_cells(&mut app, "after the strike");
    // Boundary: with one hit point more than the strike takes, the yard undeploys and the unit
    // has its share of hit points (decision R12): the rule is about what was destroyed.
    let mut app = world();
    let yard = place(&mut app, P1, "soviet_yard", 19, 19);
    let silo = charged_silo(&mut app);
    let mut probe = world();
    let twin = place(&mut probe, P1, "soviet_yard", 19, 19);
    let probe_silo = charged_silo(&mut probe);
    tick(&mut probe, vec![strike(probe_silo)]);
    let taken = 1000 - hp(&mut probe, twin).expect("the whole yard survives");
    assert!(taken > 20, "the strike takes {taken} from a yard");
    set_hp(&mut app, yard, taken + 1);
    let events = tick(
        &mut app,
        vec![
            strike(silo),
            issue(P1, Command::Undeploy { building: yard }),
        ],
    );
    assert!(rejections(&events).is_empty(), "{events:?}");
    assert_eq!(exists_as(&mut app, yard), (true, false));
    assert_eq!(hp(&mut app, yard), Some(1), "1 of 1000 as a share of 600");
}

#[test]
fn a_base_unit_destroyed_by_a_strike_is_not_deployed_in_the_same_tick() {
    // Regression (sim P0-1): the base unit became a yard with 1 hit point.
    let mut app = world();
    let mcv = spawn(&mut app, P1, "soviet_mcv", 20, 20);
    let silo = charged_silo(&mut app);
    set_hp(&mut app, mcv, 20);
    let events = tick(
        &mut app,
        vec![strike(silo), issue(P1, Command::Deploy { unit: mcv })],
    );
    assert_eq!(rejections(&events), [RejectReason::UnknownUnit(mcv)]);
    assert!(destroyed(&events, mcv), "{events:?}");
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, SimEvent::BuildingPlaced { .. })),
        "{events:?}"
    );
    ticks(&mut app, 3);
    assert_eq!(exists_as(&mut app, mcv), (false, false));
    assert_cells(&mut app, "after the strike");
}

#[test]
fn a_building_destroyed_by_a_strike_is_not_sold_repaired_or_given_a_rally_point() {
    // Regression (sim P0-1): the sale paid 1000 credits for a destroyed yard, and nothing
    // reported it destroyed.
    let mut app = world();
    let yard = place(&mut app, P1, "soviet_yard", 19, 19);
    let silo = charged_silo(&mut app);
    set_hp(&mut app, yard, 20);
    let before = credits(&app, P1);
    let events = tick(
        &mut app,
        vec![
            strike(silo),
            issue(P1, Command::RepairBuilding { building: yard }),
            issue(
                P1,
                Command::SetRallyPoint {
                    building: yard,
                    cell: CellPos::new(25, 25),
                },
            ),
            issue(P1, Command::SellBuilding { building: yard }),
        ],
    );
    assert_eq!(
        rejections(&events),
        vec![RejectReason::UnknownBuilding(yard); 3]
    );
    assert_eq!(credits(&app, P1), before);
    assert!(destroyed(&events, yard), "{events:?}");
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, SimEvent::BuildingSold { .. })),
        "{events:?}"
    );
    let repairing = app
        .world()
        .resource::<Players>()
        .get(P1)
        .expect("player")
        .repairing()
        .len();
    assert_eq!(repairing, 0);
}

#[test]
fn a_destroyed_building_is_no_prerequisite_and_no_anchor_in_the_same_tick() {
    // The player's only barracks falls to the strike; a yard far away has a power plant ready.
    let mut app = world();
    place(&mut app, P1, "soviet_yard", 34, 2);
    let barracks = place(&mut app, P1, "soviet_barracks", 19, 19);
    let power = building_kind(&app, "soviet_power");
    tick(
        &mut app,
        vec![issue(
            P1,
            Command::QueueProduction {
                item: ProductionItem::Building(power),
            },
        )],
    );
    let silo = charged_silo(&mut app);
    // The barracks drains power and nothing gives any: the plant takes 40 ticks, not 20.
    ticks(&mut app, 20);
    let conscript = unit_item(&app, "conscript");
    let beside = Command::PlaceBuilding {
        building: power,
        origin: CellPos::new(21, 19),
    };
    // Control: while the barracks stands, both commands are valid (a twin world).
    let mut control = world();
    place(&mut control, P1, "soviet_yard", 34, 2);
    place(&mut control, P1, "soviet_barracks", 19, 19);
    tick(
        &mut control,
        vec![issue(
            P1,
            Command::QueueProduction {
                item: ProductionItem::Building(power),
            },
        )],
    );
    charged_silo(&mut control);
    ticks(&mut control, 20);
    let events = tick(
        &mut control,
        vec![
            issue(P1, Command::QueueProduction { item: conscript }),
            issue(P1, beside.clone()),
        ],
    );
    assert!(rejections(&events).is_empty(), "{events:?}");
    // After the strike of the same tick neither is.
    set_hp(&mut app, barracks, 20);
    let events = tick(
        &mut app,
        vec![
            strike(silo),
            issue(P1, Command::QueueProduction { item: conscript }),
            issue(P1, beside),
        ],
    );
    let told = rejections(&events);
    assert_eq!(
        told[0],
        RejectReason::MissingPrerequisite {
            item: conscript,
            prerequisite: building_kind(&app, "soviet_barracks"),
        }
    );
    assert!(
        matches!(
            told[1],
            RejectReason::Placement(sim::PlacementError::TooFar { .. })
        ) && told.len() == 2,
        "{told:?}"
    );
    assert!(destroyed(&events, barracks));
}

#[test]
fn a_unit_destroyed_by_a_strike_takes_no_order_and_is_no_target_in_the_same_tick() {
    let mut app = world();
    let tank = spawn(&mut app, P1, "tank", 20, 20);
    let watcher = spawn(&mut app, P0, "tank", 14, 21);
    let silo = charged_silo(&mut app);
    let engineer = units_of(&mut app, P0, "engineer")[0];
    set_hp(&mut app, tank, 20);
    let events = tick(
        &mut app,
        vec![
            strike(silo),
            move_to(P1, &[tank], 25, 25),
            issue(P1, Command::Stop { units: vec![tank] }),
            issue(
                P1,
                Command::AttackMove {
                    units: vec![tank],
                    target: CellPos::new(25, 25),
                },
            ),
            attack(P1, tank, engineer),
            issue(
                P1,
                Command::Harvest {
                    units: vec![tank],
                    target: CellPos::new(25, 25),
                },
            ),
            attack(P0, watcher, tank),
        ],
    );
    let mut expected = vec![RejectReason::UnknownUnit(tank); 5];
    expected.push(RejectReason::UnknownTarget(tank));
    assert_eq!(rejections(&events), expected);
    assert!(destroyed(&events, tank));
    assert!(shots(&events, watcher).is_empty());
    // Control: a tank the strike leaves alive takes the same orders.
    let mut app = world();
    let tank = spawn(&mut app, P1, "tank", 20, 20);
    let silo = charged_silo(&mut app);
    let events = tick(&mut app, vec![strike(silo), move_to(P1, &[tank], 25, 25)]);
    assert!(rejections(&events).is_empty(), "{events:?}");
    assert_eq!(hp(&mut app, tank), Some(300 - 120));
}

#[test]
fn a_unit_destroyed_by_a_strike_does_not_embark_and_a_destroyed_transport_takes_nobody() {
    // The first suspected item of the review: a soldier without hit points went aboard, was
    // kept as a passenger and came out dead later.
    let mut app = world();
    let soldier = spawn(&mut app, P1, "conscript", 20, 20);
    let craft = spawn(&mut app, P1, "hovercraft", 21, 20);
    let silo = charged_silo(&mut app);
    set_hp(&mut app, soldier, 20);
    let events = tick(
        &mut app,
        vec![
            strike(silo),
            issue(
                P1,
                Command::Interact {
                    unit: soldier,
                    target: craft,
                },
            ),
        ],
    );
    assert_eq!(rejections(&events), [RejectReason::UnknownUnit(soldier)]);
    assert!(destroyed(&events, soldier));
    assert!(hp(&mut app, craft).is_some_and(|left| left > 0));
    assert!(
        app.world()
            .resource::<SpecialState>()
            .cargo
            .get(&craft)
            .is_none_or(Vec::is_empty)
    );
    // The other way round: the transport falls, the tank beside it lives and stays outside.
    let mut app = world();
    let craft = spawn(&mut app, P1, "hovercraft", 20, 20);
    let tank = spawn(&mut app, P1, "tank", 21, 20);
    let silo = charged_silo(&mut app);
    set_hp(&mut app, craft, 20);
    let events = tick(
        &mut app,
        vec![
            strike(silo),
            issue(
                P1,
                Command::Interact {
                    unit: tank,
                    target: craft,
                },
            ),
        ],
    );
    assert_eq!(rejections(&events), [RejectReason::UnknownTarget(craft)]);
    assert!(destroyed(&events, craft));
    assert_eq!(exists_as(&mut app, tank), (true, false));
    assert!(app.world().resource::<SpecialState>().cargo.is_empty());
}

#[test]
fn a_building_destroyed_by_a_strike_is_not_captured_by_an_engineer_who_arrives_in_that_tick() {
    // An engineer on its way has an order waiting (`SpecialState::interactions`), carried out
    // in the `Economy` stage of the tick after it arrives: a capture gives the building all
    // its hit points, so a building destroyed by a command of that tick stood again, captured.
    let mut app = world();
    let yard = place(&mut app, P1, "soviet_yard", 19, 19);
    let silo = charged_silo(&mut app);
    let sapper = spawn(&mut app, P0, "engineer", 19, 15);
    tick(
        &mut app,
        vec![issue(
            P0,
            Command::Interact {
                unit: sapper,
                target: yard,
            },
        )],
    );
    // Until it stands beside the yard with its order still waiting.
    run_until(&mut app, 400, |app, _| {
        let at = mobile(app, sapper);
        at.step.is_none() && at.cell == CellPos::new(19, 18)
    });
    assert_eq!(
        app.world()
            .resource::<SpecialState>()
            .interactions
            .get(&sapper),
        Some(&yard)
    );
    // Control (a twin world would do the same): without the strike the next tick captures.
    set_hp(&mut app, yard, 20);
    let events = tick(&mut app, vec![strike(silo)]);
    assert!(rejections(&events).is_empty(), "{events:?}");
    assert!(destroyed(&events, yard), "{events:?}");
    assert_eq!(owner_of(&mut app, yard), None, "not captured");
    assert_eq!(exists_as(&mut app, yard), (false, false));
    assert!(
        app.world()
            .resource::<SpecialState>()
            .interactions
            .is_empty()
    );
}

#[test]
fn an_engineer_who_arrives_captures_a_building_that_stands() {
    // The control of the test above: the same approach without the strike.
    let mut app = world();
    let yard = place(&mut app, P1, "soviet_yard", 19, 19);
    charged_silo(&mut app);
    let sapper = spawn(&mut app, P0, "engineer", 19, 15);
    tick(
        &mut app,
        vec![issue(
            P0,
            Command::Interact {
                unit: sapper,
                target: yard,
            },
        )],
    );
    run_until(&mut app, 400, |app, _| {
        let at = mobile(app, sapper);
        at.step.is_none() && at.cell == CellPos::new(19, 18)
    });
    set_hp(&mut app, yard, 20);
    tick(&mut app, Vec::new());
    assert_eq!(owner_of(&mut app, yard), Some(P0));
    assert_eq!(hp(&mut app, yard), Some(1000));
}

#[test]
fn a_silo_destroyed_by_a_strike_does_not_fire_in_the_same_tick() {
    let mut app = world();
    let theirs = place(&mut app, P1, "silo", 19, 19);
    let silo = charged_silo(&mut app);
    set_hp(&mut app, theirs, 20);
    // P1 aims at a cell its own silo sees.
    let events = tick(&mut app, vec![strike(silo), fire(P1, theirs, (19, 23))]);
    assert_eq!(rejections(&events), [RejectReason::UnknownUnit(theirs)]);
    let fired: Vec<PlayerId> = events
        .iter()
        .filter_map(|event| match event {
            SimEvent::StrategicActivated { player, .. } => Some(*player),
            _ => None,
        })
        .collect();
    assert_eq!(fired, [P0]);
    assert!(destroyed(&events, theirs));
    // Control: the other order of the two commands lets both fire.
    let mut app = world();
    let theirs = place(&mut app, P1, "silo", 19, 19);
    let silo = charged_silo(&mut app);
    set_hp(&mut app, theirs, 20);
    let events = tick(&mut app, vec![fire(P1, theirs, (19, 23)), strike(silo)]);
    assert!(rejections(&events).is_empty(), "{events:?}");
}

// --- P1-1: a rejection reveals nothing about what the player does not see -------------------------

/// Every command that names a unit or a building, naming `id`, as P0 would issue it with its own
/// `tank` and its own unarmed `engineer`; and what each answers for an id that names nothing P0
/// knows of.
fn naming(id: SimId, [tank, engineer]: [SimId; 2]) -> Vec<(IssuedCommand, RejectReason)> {
    let cell = CellPos::new(3, 3);
    let unit = RejectReason::UnknownUnit(id);
    let building = RejectReason::UnknownBuilding(id);
    vec![
        (move_to(P0, &[id], 3, 3), unit.clone()),
        (issue(P0, Command::Stop { units: vec![id] }), unit.clone()),
        (
            issue(
                P0,
                Command::Harvest {
                    units: vec![id],
                    target: cell,
                },
            ),
            unit.clone(),
        ),
        (
            issue(
                P0,
                Command::AttackMove {
                    units: vec![id],
                    target: cell,
                },
            ),
            unit.clone(),
        ),
        (attack(P0, id, tank), unit.clone()),
        (attack(P0, tank, id), RejectReason::UnknownTarget(id)),
        // Advisory review 2, sim P1-3: an attack on a hidden unit was answered before the
        // checks that an attack on nothing meets first, so an attack by no unit, or by an
        // unarmed one, told a hidden unit anywhere on the map from an id that names nothing.
        (
            issue(
                P0,
                Command::Attack {
                    units: Vec::new(),
                    target: id,
                },
            ),
            RejectReason::NoUnits,
        ),
        (attack(P0, engineer, id), RejectReason::NotArmed(engineer)),
        (issue(P0, Command::Deploy { unit: id }), unit.clone()),
        (
            issue(
                P0,
                Command::Interact {
                    unit: id,
                    target: tank,
                },
            ),
            unit.clone(),
        ),
        (
            issue(
                P0,
                Command::Interact {
                    unit: tank,
                    target: id,
                },
            ),
            RejectReason::UnknownTarget(id),
        ),
        (fire(P0, id, (3, 3)), unit),
        (
            issue(P0, Command::Undeploy { building: id }),
            building.clone(),
        ),
        (
            issue(P0, Command::SellBuilding { building: id }),
            building.clone(),
        ),
        (
            issue(P0, Command::RepairBuilding { building: id }),
            building.clone(),
        ),
        (
            issue(P0, Command::SetRallyPoint { building: id, cell }),
            building,
        ),
    ]
}

/// What P0 is told when it issues each command of [`naming`] for `id`, one a tick.
fn answers(app: &mut App, id: SimId, own: [SimId; 2]) -> Vec<Option<RejectReason>> {
    naming(id, own)
        .into_iter()
        .map(|(command, _)| {
            let events = tick(app, vec![command]);
            let mut reasons = rejections(&events);
            assert!(reasons.len() <= 1, "{events:?}");
            reasons.pop()
        })
        .collect()
}

fn as_for_nothing(id: SimId, own: [SimId; 2]) -> Vec<Option<RejectReason>> {
    naming(id, own)
        .into_iter()
        .map(|(_, reason)| Some(reason))
        .collect()
}

#[test]
fn every_command_answers_for_an_unseen_enemy_as_for_an_id_that_names_nothing() {
    // Regression (advisory review 1, sim P1-1): `Deploy`, `Interact` and `Superweapon` said
    // `NotOwned` for an enemy unit or building the player had never seen, and `UnknownUnit` for
    // an id that names nothing, so a player could ask for any id whether it is alive.
    let mut app = world();
    let own = [
        spawn(&mut app, P0, "tank", 2, 2),
        spawn(&mut app, P0, "engineer", 3, 2),
    ];
    place(&mut app, P0, "silo", 5, 5);
    let far_unit = spawn(&mut app, P1, "tank", 36, 36);
    let far_yard = place(&mut app, P1, "soviet_yard", 30, 30);
    ticks(&mut app, 2);
    let nothing = SimId(9_999);
    let before = hash(&mut app);
    for id in [nothing, far_unit, far_yard] {
        assert_eq!(
            answers(&mut app, id, own),
            as_for_nothing(id, own),
            "{id:?}"
        );
    }
    // None of them changed anything but the tick (a twin world that ran as long is equal).
    let mut twin = world();
    spawn(&mut twin, P0, "tank", 2, 2);
    spawn(&mut twin, P0, "engineer", 3, 2);
    place(&mut twin, P0, "silo", 5, 5);
    spawn(&mut twin, P1, "tank", 36, 36);
    place(&mut twin, P1, "soviet_yard", 30, 30);
    ticks(&mut twin, 2);
    assert_eq!(hash(&mut twin), before);
    ticks(&mut twin, 3 * naming(nothing, own).len() as u32);
    assert_eq!(hash(&mut twin), hash(&mut app));
}

#[test]
fn a_seen_enemy_is_reported_as_not_owned() {
    // The other side of the rule: what the player sees, it may be told is another's.
    let mut app = world();
    let own = [
        spawn(&mut app, P0, "tank", 2, 2),
        spawn(&mut app, P0, "engineer", 3, 2),
    ];
    place(&mut app, P0, "silo", 5, 5);
    let near_unit = spawn(&mut app, P1, "soviet_mcv", 6, 2);
    let near_yard = place(&mut app, P1, "soviet_yard", 2, 8);
    ticks(&mut app, 2);
    let not_owned = |id: SimId| Some(RejectReason::NotOwned(id));
    let told = answers(&mut app, near_unit, own);
    let names = [
        "move",
        "stop",
        "harvest",
        "attack move",
        "attack with",
        "attack on",
        "attack by nobody",
        "attack by the unarmed",
        "deploy",
        "interact as",
        "interact with",
        "superweapon",
        "undeploy",
        "sell",
        "repair",
        "rally",
    ];
    assert_eq!(told.len(), names.len());
    let by_name = |told: &[Option<RejectReason>], name: &str| {
        told[names.iter().position(|n| *n == name).expect("name")].clone()
    };
    for name in [
        "move",
        "stop",
        "harvest",
        "attack move",
        "attack with",
        "deploy",
        "interact as",
        "superweapon",
    ] {
        assert_eq!(by_name(&told, name), not_owned(near_unit), "{name}");
    }
    // A unit is no building: the building commands do not know it.
    for name in ["undeploy", "sell", "repair", "rally"] {
        assert_eq!(
            by_name(&told, name),
            Some(RejectReason::UnknownBuilding(near_unit)),
            "{name}"
        );
    }
    let told = answers(&mut app, near_yard, own);
    for name in [
        "deploy",
        "interact as",
        "superweapon",
        "undeploy",
        "sell",
        "repair",
        "rally",
    ] {
        assert_eq!(by_name(&told, name), not_owned(near_yard), "{name}");
    }
    // A building is no unit: the unit commands do not know it.
    for name in ["move", "stop", "harvest", "attack move", "attack with"] {
        assert_eq!(
            by_name(&told, name),
            Some(RejectReason::UnknownUnit(near_yard)),
            "{name}"
        );
    }
}

#[test]
fn every_command_answers_for_a_submerged_enemy_as_for_an_id_that_names_nothing() {
    // RA4: a submerged unit in water the player sees is hidden from it
    // (`SpecialState::hidden`). `Attack` already said so; the commands that name it as their own
    // unit said `NotOwned`, which told the player that something is there.
    let mut app = world();
    flood(&mut app, (12, 0), (39, 39));
    let own = [
        spawn(&mut app, P0, "tank", 10, 10),
        spawn(&mut app, P0, "engineer", 10, 11),
    ];
    place(&mut app, P0, "silo", 5, 5);
    let sub = spawn(&mut app, P1, "sub", 15, 10);
    app.world_mut()
        .resource_mut::<SpecialState>()
        .disabled
        .insert(sub, u64::MAX);
    ticks(&mut app, 2);
    let state = app.world().resource::<SpecialState>();
    assert!(state.hidden.contains(&(P0, sub)));
    let sees = app
        .world()
        .resource::<Players>()
        .get(P0)
        .expect("player")
        .vision()
        .is_visible(CellPos::new(15, 10));
    assert!(sees, "the tank sees the water the sub is in");
    assert_eq!(answers(&mut app, sub, own), as_for_nothing(sub, own));
    // Detected (a boat of P0 with sensors of five cells beside it), it is another's unit.
    let mut app = world();
    flood(&mut app, (12, 0), (39, 39));
    let own = [
        spawn(&mut app, P0, "tank", 10, 10),
        spawn(&mut app, P0, "engineer", 10, 11),
    ];
    place(&mut app, P0, "silo", 5, 5);
    let sub = spawn(&mut app, P1, "sub", 15, 10);
    let boat = spawn(&mut app, P0, "boat", 13, 12);
    for id in [sub, boat] {
        app.world_mut()
            .resource_mut::<SpecialState>()
            .disabled
            .insert(id, u64::MAX);
    }
    ticks(&mut app, 2);
    assert!(
        !app.world()
            .resource::<SpecialState>()
            .hidden
            .contains(&(P0, sub))
    );
    let told = answers(&mut app, sub, own);
    assert_eq!(told[0], Some(RejectReason::NotOwned(sub)), "move");
    assert_eq!(told[8], Some(RejectReason::NotOwned(sub)), "deploy");
    assert_eq!(told[9], Some(RejectReason::NotOwned(sub)), "interact");
    assert_eq!(told[11], Some(RejectReason::NotOwned(sub)), "superweapon");
}

// --- The same two rules on the shipped data --------------------------------------------------------

/// A world of 48 x 48 open cells with the shipped data (`assets/data`) and the players P0
/// (Great Britain) and P1 (Russia), who are enemies.
fn shipped_world() -> App {
    let definitions = sim::load_ruleset(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/data"),
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let mut app = App::new();
    app.add_plugins(sim::SimPlugin { seed: 11 });
    let cell = sim::Cell {
        passable: true,
        water: false,
        buildable: true,
        ore: 0,
    };
    app.world_mut()
        .insert_resource(sim::Grid::from_cells(48, 48, vec![cell; 48 * 48]).expect("a grid"));
    for (player, country) in [(P0, "great_britain"), (P1, "russia")] {
        let id = definitions
            .rules()
            .and_then(|rules| rules.country_id(country))
            .unwrap_or_else(|| panic!("country {country}"));
        let mut players = app.world_mut().resource_mut::<Players>();
        players.add(player, 10_000).expect("a new player");
        assert!(players.set_country(player, Some(id)));
    }
    app.world_mut().insert_resource(definitions);
    app
}

/// A powered building of `owner` of the superweapon type `id` at (`x`, `y`) with four Tesla
/// Reactors beside it, charged (scenario setup: the charge is written, not waited for).
fn charged(app: &mut App, owner: PlayerId, id: &str, x: i32, y: i32) -> SimId {
    for column in 0..4 {
        place(app, owner, "tesla_reactor", x + 3 * column, y);
    }
    let building = place(app, owner, id, x, y + 4);
    ticks(app, 2);
    let kind = building_kind(app, id);
    let charge = defs(app)
        .metadata(ProductionItem::Building(kind))
        .expect("an arsenal entry")
        .cooldown;
    app.world_mut()
        .resource_mut::<SpecialState>()
        .charge
        .insert(building, charge);
    building
}

/// A Grizzly of P0 with `hit_points` ten cells from ground zero, where the shipped nuclear
/// strike takes 12 hit points from it (600 x 2 percent at the edge of 10 cells, heavy armour
/// 100 percent), beside an IFV of P0 that carries an Engineer and so repairs the vehicles
/// within 2 cells by 20 hit points on every twentieth tick; a charged silo of P1, who sees
/// ground zero; the strike is issued in a tick in which the IFV repairs. Returns the Grizzly's
/// hit points after that tick.
fn strike_beside_a_repair_ifv(hit_points: i32) -> Option<i32> {
    let mut app = shipped_world();
    let silo = charged(&mut app, P1, "nuclear_silo", 1, 1);
    spawn(&mut app, P1, "engineer", 33, 30);
    let grizzly = spawn(&mut app, P0, "grizzly", 30, 20);
    let ifv = spawn(&mut app, P0, "ifv", 30, 18);
    let engineer = spawn(&mut app, P0, "engineer", 30, 17);
    let events = tick(
        &mut app,
        vec![issue(
            P0,
            Command::Interact {
                unit: engineer,
                target: ifv,
            },
        )],
    );
    assert!(rejections(&events).is_empty(), "{events:?}");
    assert_eq!(
        app.world()
            .resource::<SpecialState>()
            .cargo
            .get(&ifv)
            .map(Vec::len),
        Some(1)
    );
    // The repair of this tick, if it is one, has been made; from here on the Grizzly has the
    // hit points the scenario asks for.
    while !(app.world().resource::<sim::SimTick>().0 + 1).is_multiple_of(20) {
        tick(&mut app, Vec::new());
    }
    set_hp(&mut app, grizzly, hit_points);
    tick(&mut app, Vec::new());
    assert!((app.world().resource::<sim::SimTick>().0).is_multiple_of(20));
    set_hp(&mut app, grizzly, hit_points);
    let events = tick(&mut app, vec![fire(P1, silo, (30, 30))]);
    assert!(rejections(&events).is_empty(), "{events:?}");
    assert!(hp(&mut app, ifv).is_some(), "the IFV is outside the blast");
    hp(&mut app, grizzly)
}

#[test]
fn a_vehicle_destroyed_by_a_strike_is_not_repaired_back_to_life_in_the_same_tick() {
    // The same root cause in the `Economy` stage: the repair of an IFV that carries an Engineer
    // added its 20 hit points to a vehicle the strike of the tick's commands had left at -7.
    assert_eq!(strike_beside_a_repair_ifv(5), None, "destroyed");
    // Control and boundary: with 12 hit points it is destroyed as well, with 13 it keeps one
    // and is repaired in that tick.
    assert_eq!(strike_beside_a_repair_ifv(12), None);
    assert_eq!(strike_beside_a_repair_ifv(13), Some(13 - 12 + 20));
}

#[test]
fn the_chronosphere_answers_for_an_unseen_enemy_vehicle_as_for_an_id_that_names_nothing() {
    // The units a Chronosphere command names go through the same ownership check (sim P1-1).
    let mut app = shipped_world();
    let sphere = charged(&mut app, P0, "chronosphere", 1, 1);
    let own = spawn(&mut app, P0, "grizzly", 6, 9);
    let seen = spawn(&mut app, P1, "rhino", 8, 9);
    let unseen = spawn(&mut app, P1, "rhino", 40, 40);
    for id in [own, seen, unseen] {
        app.world_mut()
            .resource_mut::<SpecialState>()
            .disabled
            .insert(id, u64::MAX);
    }
    ticks(&mut app, 2);
    let send = |units: Vec<SimId>| {
        issue(
            P0,
            Command::Superweapon {
                building: sphere,
                target: CellPos::new(10, 12),
                units,
            },
        )
    };
    let nothing = SimId(9_999);
    let events = tick(
        &mut app,
        vec![
            send(vec![own, nothing]),
            send(vec![own, unseen]),
            send(vec![own, seen]),
        ],
    );
    assert_eq!(
        rejections(&events),
        [
            RejectReason::UnknownUnit(nothing),
            RejectReason::UnknownUnit(unseen),
            RejectReason::NotOwned(seen),
        ]
    );
    // Positive: the player's own vehicle alone goes.
    let events = tick(&mut app, vec![send(vec![own])]);
    assert!(rejections(&events).is_empty(), "{events:?}");
    assert_ne!(mobile(&mut app, own).cell, CellPos::new(6, 9));
}

#[test]
fn deploying_names_nothing_in_a_footprint_the_player_does_not_see() {
    // Advisory review 1, sim P2-4: `Deploy` looked at the footprint before it asked whether
    // the player sees it, so its rejection named a unit in a cell the player does not see.
    // (`PlaceBuilding` has asked in the right order since M4, D33.) With the shipped data and
    // the fixture a base unit sees its whole footprint, also under a Gap Generator (a unit's own
    // sight counters it), so the case needs a base unit that sees one cell around it.
    let scene = |sight: u32| {
        let from = "(unit: \"allied_mcv\", hit_points: 600, armour: \"plate\", sight: 6,";
        let to =
            format!("(unit: \"allied_mcv\", hit_points: 600, armour: \"plate\", sight: {sight},");
        let mut app = world_with(&format!("mcv_sight_{sight}"), &[(from, to.as_str())]);
        let mcv = spawn(&mut app, P0, "allied_mcv", 20, 20);
        let soldier = spawn(&mut app, P1, "conscript", 22, 22);
        app.world_mut()
            .resource_mut::<SpecialState>()
            .disabled
            .insert(soldier, u64::MAX);
        ticks(&mut app, 2);
        let sees = app
            .world()
            .resource::<Players>()
            .get(P0)
            .expect("player")
            .vision()
            .is_visible(CellPos::new(22, 22));
        let events = tick(&mut app, vec![issue(P0, Command::Deploy { unit: mcv })]);
        (sees, rejections(&events), soldier)
    };
    // Seen, the soldier in the footprint is named.
    let (sees, told, soldier) = scene(6);
    assert!(sees);
    assert_eq!(
        told,
        [RejectReason::Placement(sim::PlacementError::Occupied {
            cell: CellPos::new(22, 22),
            unit: soldier,
        })]
    );
    // Unseen, nothing is.
    let (sees, told, _) = scene(1);
    assert!(!sees);
    assert_eq!(
        told,
        [RejectReason::AbilityUnavailable(
            "Explore the deployment footprint first"
        )]
    );
}

fn ra5_house(app: &mut App, cell: CellPos) -> SimId {
    let mut looks = vec![0; 48 * 48];
    looks[cell.y as usize * 48 + cell.x as usize] = 5 << 4;
    app.world_mut().resource_mut::<sim::Grid>().set_terrain(
        cell,
        sim::Cell {
            passable: false,
            water: false,
            buildable: false,
            ore: 0,
        },
    );
    let props = sim::Props::from_looks(
        app.world().resource::<sim::Grid>(),
        &looks,
        defs(app).armour_class("wood").expect("wood"),
    );
    let id = props.id_at(cell).expect("house");
    app.world_mut().insert_resource(props);
    id
}

#[test]
fn ra5_same_tick_dead_actors_cannot_capture_repair_enter_hospital_or_occupy() {
    for (target_type, actor_type) in [
        ("oil_derrick", "engineer"),
        ("hospital", "engineer"),
        ("airport", "engineer"),
        ("tech_outpost", "engineer"),
        ("bridge_hut", "engineer"),
        ("hospital", "gi"),
        ("civilian_house", "gi"),
    ] {
        let mut app = shipped_world();
        let silo = charged(&mut app, P1, "nuclear_silo", 1, 1);
        spawn(&mut app, P1, "engineer", 24, 20);
        let target = if target_type == "civilian_house" {
            ra5_house(&mut app, CellPos::new(21, 20))
        } else {
            place(
                &mut app,
                if actor_type == "gi" { P0 } else { sim::NEUTRAL },
                target_type,
                21,
                20,
            )
        };
        let actor = spawn(&mut app, P0, actor_type, 20, 20);
        ticks(&mut app, 2);
        set_hp(&mut app, actor, 1);
        let events = tick(
            &mut app,
            vec![
                fire(P1, silo, (20, 20)),
                issue(
                    P0,
                    Command::Interact {
                        unit: actor,
                        target,
                    },
                ),
            ],
        );
        assert_eq!(
            rejections(&events),
            [RejectReason::UnknownUnit(actor)],
            "{target_type}: {events:?}"
        );
        assert_eq!(exists_as(&mut app, actor), (false, false));
        assert!(
            !app.world()
                .resource::<SpecialState>()
                .cargo
                .values()
                .flatten()
                .any(|p| p.id == actor)
        );
    }
}

#[test]
fn ra5_unseen_neutral_tech_and_enemy_garrison_ids_answer_exactly_as_missing_targets() {
    for target_type in [
        "oil_derrick",
        "hospital",
        "airport",
        "tech_outpost",
        "bridge_hut",
        "civilian_house",
    ] {
        let mut app = shipped_world();
        let engineer = spawn(&mut app, P0, "engineer", 3, 3);
        let tank = spawn(&mut app, P0, "grizzly", 4, 3);
        let target = if target_type == "civilian_house" {
            let house = ra5_house(&mut app, CellPos::new(40, 40));
            let gi = spawn(&mut app, P1, "conscript", 39, 40);
            ticks(&mut app, 2);
            let events = tick(
                &mut app,
                vec![issue(
                    P1,
                    Command::Interact {
                        unit: gi,
                        target: house,
                    },
                )],
            );
            assert!(rejections(&events).is_empty());
            house
        } else {
            place(&mut app, sim::NEUTRAL, target_type, 40, 40)
        };
        ticks(&mut app, 2);
        for id in [target, SimId(0xFFFF_FFFE)] {
            let events = tick(
                &mut app,
                vec![
                    issue(
                        P0,
                        Command::Interact {
                            unit: engineer,
                            target: id,
                        },
                    ),
                    issue(
                        P0,
                        Command::Attack {
                            units: vec![tank],
                            target: id,
                        },
                    ),
                ],
            );
            assert_eq!(
                rejections(&events),
                [
                    RejectReason::UnknownTarget(id),
                    RejectReason::UnknownTarget(id)
                ],
                "{target_type}: {events:?}"
            );
        }
    }
}

#[test]
fn ra5_garrison_destroyed_earlier_in_the_command_tick_cannot_deploy() {
    let mut app = shipped_world();
    let silo = charged(&mut app, P1, "nuclear_silo", 1, 1);
    spawn(&mut app, P1, "engineer", 24, 20);
    let house = ra5_house(&mut app, CellPos::new(20, 20));
    let gi = spawn(&mut app, P0, "gi", 19, 20);
    ticks(&mut app, 2);
    tick(
        &mut app,
        vec![issue(
            P0,
            Command::Interact {
                unit: gi,
                target: house,
            },
        )],
    );
    let events = tick(
        &mut app,
        vec![
            fire(P1, silo, (20, 20)),
            issue(P0, Command::Deploy { unit: house }),
        ],
    );
    assert_eq!(
        rejections(&events),
        [RejectReason::UnknownUnit(house)],
        "{events:?}"
    );
}
