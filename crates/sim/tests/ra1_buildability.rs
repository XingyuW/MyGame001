//! RA1: who may build what (docs/RA2_PARITY_PLAN.md section 3.3; docs/DECISIONS.md R10).
//!
//! The buildability rule of Red Alert 2, driven through `CommandQueue` and `SimSchedule` on the
//! RA1 fixture (`tests/fixtures/data/ra1`, a small world with two sides and three countries;
//! invariant I10): prerequisites and prerequisite groups, factory groups, the side of the
//! factory, the player's country, the tech level, the superweapon option, and the queue that
//! the factories of one group share.

use std::path::PathBuf;

use bevy_app::App;
use bevy_ecs::message::Messages;
use sim::{
    Building, BuildingKind, Cell, CellPos, Command, CommandQueue, Definitions, Grid, IssuedCommand,
    MatchRules, MatchState, Owner, PlayerId, Players, ProductionItem, RejectReason, SimEvent,
    SimId, SimPlugin, SimSchedule, Unit, check_buildable, check_placement, country_tree,
    load_match_definitions, missing_prerequisite, produces, spawn_building, spawn_unit, state_hash,
};

const P0: PlayerId = PlayerId(0);
const P1: PlayerId = PlayerId(1);

/// No rejection.
const NONE: [RejectReason; 0] = [];

fn fixture(file: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/data/ra1")
        .join(file)
}

fn definitions() -> Definitions {
    load_match_definitions(
        fixture("units.ron"),
        fixture("economy.ron"),
        fixture("combat.ron"),
    )
    .and_then(|definitions| definitions.with_rules(fixture("rules.ron")))
    .and_then(|definitions| definitions.with_arsenal(fixture("arsenal.ron")))
    .unwrap_or_else(|error| panic!("{error}"))
}

/// A world of 40 x 40 open cells with two players of the countries `countries` (ids of the
/// rules file), each with plenty of credits.
fn world(countries: [&str; 2]) -> App {
    let definitions = definitions();
    let mut app = App::new();
    app.add_plugins(SimPlugin { seed: 3 });
    let cell = Cell {
        passable: true,
        water: false,
        buildable: true,
        ore: 0,
    };
    app.world_mut()
        .insert_resource(Grid::from_cells(40, 40, vec![cell; 1600]).expect("a grid"));
    for (player, country) in [P0, P1].into_iter().zip(countries) {
        let id = definitions
            .rules()
            .and_then(|rules| rules.country_id(country))
            .unwrap_or_else(|| panic!("country {country}"));
        let mut players = app.world_mut().resource_mut::<Players>();
        players.add(player, 100_000).expect("a new player");
        assert!(players.set_country(player, Some(id)));
    }
    app.world_mut().insert_resource(definitions);
    app
}

fn defs(app: &App) -> Definitions {
    app.world().resource::<Definitions>().clone()
}

fn building(app: &App, id: &str) -> ProductionItem {
    ProductionItem::Building(kind(app, id))
}

fn kind(app: &App, id: &str) -> BuildingKind {
    defs(app)
        .building_kind(id)
        .unwrap_or_else(|| panic!("building {id}"))
}

fn unit(app: &App, id: &str) -> ProductionItem {
    ProductionItem::Unit(
        defs(app)
            .unit_kind(id)
            .unwrap_or_else(|| panic!("unit {id}")),
    )
}

fn place(app: &mut App, owner: PlayerId, id: &str, x: i32, y: i32) -> SimId {
    spawn_building(app.world_mut(), owner, id, CellPos::new(x, y))
        .unwrap_or_else(|error| panic!("{id}: {error}"))
}

fn tick(app: &mut App, commands: Vec<IssuedCommand>) -> Vec<SimEvent> {
    {
        let mut queue = app.world_mut().resource_mut::<CommandQueue>();
        for command in commands {
            queue.push(command);
        }
    }
    app.world_mut().run_schedule(SimSchedule);
    app.world()
        .resource::<Messages<SimEvent>>()
        .iter_current_update_messages()
        .cloned()
        .collect()
}

fn queue(player: PlayerId, item: ProductionItem) -> IssuedCommand {
    IssuedCommand {
        player,
        command: Command::QueueProduction { item },
    }
}

fn rejections(events: &[SimEvent]) -> Vec<RejectReason> {
    events
        .iter()
        .filter_map(|event| match event {
            SimEvent::CommandRejected { reason, .. } => Some(reason.clone()),
            _ => None,
        })
        .collect()
}

/// The queue of `player` kept under the factory group of `producer`, as item list.
fn queued(app: &App, player: PlayerId, producer: &str) -> Vec<ProductionItem> {
    let key = defs(app).queue_key(kind(app, producer));
    app.world()
        .resource::<Players>()
        .get(player)
        .expect("player")
        .queue(key)
        .iter()
        .map(|queued| queued.item)
        .collect()
}

fn owned_kinds(app: &mut App, player: PlayerId) -> Vec<BuildingKind> {
    let mut query = app.world_mut().query::<(&SimId, &Owner, &Building)>();
    let mut found: Vec<(SimId, BuildingKind)> = query
        .iter(app.world())
        .filter(|(_, owner, _)| owner.0 == player)
        .map(|(id, _, building)| (*id, building.kind))
        .collect();
    found.sort_unstable();
    found.into_iter().map(|(_, kind)| kind).collect()
}

fn units_of(app: &mut App, player: PlayerId, id: &str) -> usize {
    let kind = defs(app).unit_kind(id).expect("unit");
    let mut query = app.world_mut().query::<(&Owner, &Unit)>();
    query
        .iter(app.world())
        .filter(|(owner, unit)| owner.0 == player && unit.kind == kind)
        .count()
}

/// Queues the building `id` for `player`, lets it complete, and places it on the first cell
/// the command's own validation accepts. Panics on any rejection.
fn build(app: &mut App, player: PlayerId, id: &str) {
    let item = building(app, id);
    let kind = kind(app, id);
    let time = defs(app).production(item).expect("terms").build_time;
    let events = tick(app, vec![queue(player, item)]);
    assert_eq!(rejections(&events), NONE, "queue {id}");
    for _ in 0..=time {
        assert_eq!(rejections(&tick(app, Vec::new())), NONE, "build {id}");
    }
    let footprints: Vec<_> = {
        let mut query = app.world_mut().query::<(&Owner, &Building)>();
        query
            .iter(app.world())
            .filter(|(owner, _)| owner.0 == player)
            .map(|(_, building)| building.footprint)
            .collect()
    };
    let origin = {
        let world = app.world();
        let (grid, definitions) = (world.resource::<Grid>(), world.resource::<Definitions>());
        let state = world.resource::<Players>().get(player).expect("player");
        (0..40)
            .flat_map(|y| (0..40).map(move |x| CellPos::new(x, y)))
            .find(|&origin| {
                check_placement(
                    grid,
                    definitions,
                    state,
                    footprints.iter().copied(),
                    kind,
                    origin,
                )
                .is_ok()
            })
            .unwrap_or_else(|| panic!("no place for {id}"))
    };
    let events = tick(
        app,
        vec![IssuedCommand {
            player,
            command: Command::PlaceBuilding {
                building: kind,
                origin,
            },
        }],
    );
    assert_eq!(rejections(&events), NONE, "place {id}");
    assert!(owned_kinds(app, player).contains(&kind), "{id} stands");
}

/// Queues the unit `id` for `player` and waits until it has left its factory.
fn train(app: &mut App, player: PlayerId, id: &str) {
    let item = unit(app, id);
    let before = units_of(app, player, id);
    let time = defs(app).production(item).expect("terms").build_time;
    assert_eq!(
        rejections(&tick(app, vec![queue(player, item)])),
        NONE,
        "{id}"
    );
    for _ in 0..time + 2 {
        assert_eq!(rejections(&tick(app, Vec::new())), NONE, "{id}");
    }
    assert_eq!(
        units_of(app, player, id),
        before + 1,
        "{id} left its factory"
    );
}

// --- Positive ---------------------------------------------------------------------------------

#[test]
fn each_side_builds_its_own_tree_from_its_construction_yard() {
    // An Allied player builds the Allied tree up to the superweapon and trains its units; a
    // Soviet player the Soviet tree. Everything goes through QueueProduction and PlaceBuilding.
    let mut app = world(["alpha", "gamma"]);
    place(&mut app, P0, "allied_yard", 4, 4);
    place(&mut app, P1, "soviet_yard", 30, 30);
    tick(&mut app, Vec::new());
    for id in [
        "allied_power",
        "allied_refinery",
        "allied_barracks",
        "allied_factory",
        "allied_power",
        "allied_radar",
        "turret",
        "storm_device",
    ] {
        build(&mut app, P0, id);
    }
    for id in ["rifleman", "engineer", "sniper", "tank", "allied_mcv"] {
        train(&mut app, P0, id);
    }
    for id in [
        "soviet_power",
        "soviet_refinery",
        "soviet_barracks",
        "soviet_factory",
        "soviet_radar",
        "turret",
    ] {
        build(&mut app, P1, id);
    }
    for id in ["conscript", "militia", "engineer", "heavy", "soviet_mcv"] {
        train(&mut app, P1, id);
    }
    // The other side's items stay closed: its buildings need its yard, its units its factories.
    let closed = vec![
        queue(P0, building(&app, "soviet_power")),
        queue(P0, unit(&app, "conscript")),
        queue(P0, unit(&app, "militia")),
        queue(P1, building(&app, "allied_power")),
        queue(P1, unit(&app, "tank")),
    ];
    let events = tick(&mut app, closed);
    assert_eq!(
        rejections(&events),
        [
            RejectReason::MissingPrerequisite {
                item: building(&app, "soviet_power"),
                prerequisite: kind(&app, "soviet_yard"),
            },
            RejectReason::MissingPrerequisite {
                item: unit(&app, "conscript"),
                prerequisite: kind(&app, "soviet_barracks"),
            },
            RejectReason::NoFactoryOfSide(unit(&app, "militia")),
            RejectReason::MissingPrerequisite {
                item: building(&app, "allied_power"),
                prerequisite: kind(&app, "allied_yard"),
            },
            RejectReason::MissingPrerequisite {
                item: unit(&app, "tank"),
                prerequisite: kind(&app, "allied_factory"),
            },
        ]
    );
}

#[test]
fn a_captured_barracks_of_the_other_side_trains_its_infantry() {
    // Tests 1 to 3 of the rule look at buildings, not at the player: an Allied player who
    // captures a Soviet Barracks with an engineer (the `Interact` command) trains Conscripts
    // and Militia there, and loses them again with the building.
    let mut app = world(["alpha", "gamma"]);
    place(&mut app, P0, "allied_yard", 4, 4);
    place(&mut app, P0, "allied_power", 10, 4);
    let enemy = place(&mut app, P1, "soviet_barracks", 20, 20);
    place(&mut app, P1, "soviet_yard", 30, 30);
    let engineer =
        spawn_unit(app.world_mut(), P0, "engineer", CellPos::new(19, 20)).expect("an engineer");
    tick(&mut app, Vec::new());
    let conscript = unit(&app, "conscript");
    let before = tick(&mut app, vec![queue(P0, conscript)]);
    assert_eq!(
        rejections(&before),
        [RejectReason::MissingPrerequisite {
            item: conscript,
            prerequisite: kind(&app, "soviet_barracks"),
        }]
    );
    let events = tick(
        &mut app,
        vec![IssuedCommand {
            player: P0,
            command: Command::Interact {
                unit: engineer,
                target: enemy,
            },
        }],
    );
    assert_eq!(rejections(&events), NONE);
    assert!(owned_kinds(&mut app, P0).contains(&kind(&app, "soviet_barracks")));
    train(&mut app, P0, "conscript");
    train(&mut app, P0, "militia");
    // The engineer needs any barracks, and the captured one is one.
    train(&mut app, P0, "engineer");
    // Its own side's infantry still needs its own barracks.
    let rifleman = unit(&app, "rifleman");
    assert_eq!(
        rejections(&tick(&mut app, vec![queue(P0, rifleman)])),
        [RejectReason::MissingPrerequisite {
            item: rifleman,
            prerequisite: kind(&app, "allied_barracks"),
        }]
    );
    // Selling the captured barracks closes the Soviet infantry again, and a Conscript in
    // production waits for a barracks that trains it.
    assert_eq!(
        rejections(&tick(&mut app, vec![queue(P0, conscript)])),
        NONE
    );
    let events = tick(
        &mut app,
        vec![IssuedCommand {
            player: P0,
            command: Command::SellBuilding { building: enemy },
        }],
    );
    assert_eq!(rejections(&events), NONE);
    let credits = |app: &App| {
        app.world()
            .resource::<Players>()
            .get(P0)
            .expect("P0")
            .credits()
    };
    let waiting = credits(&app);
    for _ in 0..30 {
        tick(&mut app, Vec::new());
    }
    assert_eq!(queued(&app, P0, "soviet_barracks"), [conscript]);
    assert_eq!(
        credits(&app),
        waiting,
        "no progress without a factory of the side"
    );
    assert_eq!(units_of(&mut app, P0, "conscript"), 1);
}

#[test]
fn the_factories_of_one_group_share_a_queue_and_work_in_turn() {
    // With an Allied and a (captured) Soviet barracks the player has one infantry queue: the
    // items of both sides wait in it in the order they were queued, each produced by a
    // barracks of a side that builds it.
    let mut app = world(["alpha", "gamma"]);
    place(&mut app, P0, "allied_yard", 4, 4);
    place(&mut app, P0, "allied_power", 10, 4);
    place(&mut app, P0, "allied_barracks", 14, 4);
    place(&mut app, P0, "soviet_barracks", 18, 4);
    tick(&mut app, Vec::new());
    let (rifleman, conscript) = (unit(&app, "rifleman"), unit(&app, "conscript"));
    let definitions = defs(&app);
    let allied = kind(&app, "allied_barracks");
    let soviet = kind(&app, "soviet_barracks");
    assert_eq!(definitions.queue_key(allied), definitions.queue_key(soviet));
    assert!(definitions.same_factory(allied, soviet));
    assert!(produces(&definitions, allied, rifleman) && !produces(&definitions, soviet, rifleman));
    assert!(
        produces(&definitions, soviet, conscript) && !produces(&definitions, allied, conscript)
    );
    let events = tick(
        &mut app,
        vec![
            queue(P0, rifleman),
            queue(P0, conscript),
            queue(P0, rifleman),
        ],
    );
    assert_eq!(rejections(&events), NONE);
    assert_eq!(
        queued(&app, P0, "allied_barracks"),
        [rifleman, conscript, rifleman]
    );
    assert_eq!(
        queued(&app, P0, "soviet_barracks"),
        queued(&app, P0, "allied_barracks")
    );
    // One item at a time: after the first build time only the first rifleman has left.
    let mut left = Vec::new();
    for _ in 0..40 {
        for event in tick(&mut app, Vec::new()) {
            if let SimEvent::UnitSpawned { building, .. } = event {
                left.push(building);
            }
        }
    }
    let ids: Vec<(SimId, BuildingKind)> = {
        let mut query = app.world_mut().query::<(&SimId, &Building)>();
        query
            .iter(app.world())
            .map(|(id, b)| (*id, b.kind))
            .collect()
    };
    let kind_of = |id: SimId| ids.iter().find(|(found, _)| *found == id).expect("id").1;
    assert_eq!(
        left.iter().map(|&id| kind_of(id)).collect::<Vec<_>>(),
        [allied, soviet, allied],
        "each unit leaves a barracks of a side that trains it"
    );
    assert!(queued(&app, P0, "allied_barracks").is_empty());
}

#[test]
fn a_prerequisite_group_is_satisfied_by_any_member_also_its_last() {
    // Boundary: the refinery needs "any power". Without one it is rejected naming the group's
    // first member; the last member of the group alone (a captured Soviet plant) satisfies it.
    let mut app = world(["alpha", "gamma"]);
    place(&mut app, P0, "allied_yard", 4, 4);
    tick(&mut app, Vec::new());
    let refinery = building(&app, "allied_refinery");
    assert_eq!(
        rejections(&tick(&mut app, vec![queue(P0, refinery)])),
        [RejectReason::MissingPrerequisite {
            item: refinery,
            prerequisite: kind(&app, "allied_power"),
        }]
    );
    let definitions = defs(&app);
    let yard = kind(&app, "allied_yard");
    assert_eq!(
        missing_prerequisite(&definitions, &[yard], refinery),
        Some(kind(&app, "allied_power"))
    );
    assert_eq!(
        missing_prerequisite(&definitions, &[yard, kind(&app, "soviet_power")], refinery),
        None
    );
    place(&mut app, P0, "soviet_power", 10, 4);
    tick(&mut app, Vec::new());
    assert_eq!(rejections(&tick(&mut app, vec![queue(P0, refinery)])), NONE);
    assert_eq!(queued(&app, P0, "allied_yard"), [refinery]);
}

#[test]
fn a_country_tree_holds_what_the_country_builds_on_its_own() {
    let app = world(["alpha", "gamma"]);
    let definitions = defs(&app);
    let rules = definitions.rules().expect("rules");
    let tree = |country: &str| country_tree(&definitions, rules.country_id(country).expect("id"));
    let (alpha, beta, gamma) = (tree("alpha"), tree("beta"), tree("gamma"));
    for id in ["allied_power", "allied_factory", "storm_device", "turret"] {
        assert!(alpha.contains(&building(&app, id)) && beta.contains(&building(&app, id)));
        assert_eq!(gamma.contains(&building(&app, id)), id == "turret", "{id}");
    }
    for id in ["soviet_power", "soviet_factory"] {
        assert!(gamma.contains(&building(&app, id)) && !alpha.contains(&building(&app, id)));
    }
    // The yards themselves cannot be queued (tech level -1): a yard comes from its base unit.
    assert!(!alpha.contains(&building(&app, "allied_yard")));
    // National specials: the sniper is alpha's, the raider everyone's but alpha's.
    assert!(alpha.contains(&unit(&app, "sniper")) && !beta.contains(&unit(&app, "sniper")));
    assert!(!alpha.contains(&unit(&app, "raider")) && beta.contains(&unit(&app, "raider")));
    assert!(!gamma.contains(&unit(&app, "sniper")) && !gamma.contains(&unit(&app, "raider")));
    // Items outside the tech level of the rules, and the other side's units.
    for id in ["prototype", "advanced", "conscript", "militia", "heavy"] {
        assert!(!alpha.contains(&unit(&app, id)), "{id}");
    }
    assert!(gamma.contains(&unit(&app, "militia")) && gamma.contains(&unit(&app, "engineer")));
    assert!(alpha.contains(&unit(&app, "engineer")));
}

// --- Negative ---------------------------------------------------------------------------------

/// A base of P0 with every Allied building, built identically for a world and its quiet twin.
fn allied_base(countries: [&str; 2], superweapons: bool) -> App {
    let mut app = world(countries);
    place(&mut app, P0, "allied_yard", 4, 4);
    place(&mut app, P0, "allied_power", 10, 4);
    place(&mut app, P0, "allied_barracks", 14, 4);
    place(&mut app, P0, "allied_radar", 18, 4);
    place(&mut app, P1, "soviet_yard", 30, 30);
    app.world_mut()
        .resource_mut::<MatchState>()
        .set_rules(MatchRules {
            superweapons,
            ..MatchRules::default()
        });
    tick(&mut app, Vec::new());
    app
}

/// Asserts that `commands` of P0 are rejected with `reasons`, in order, and change nothing but
/// the tick counter and the event buffer: the same tick without them gives the same hash.
fn assert_rejected_without_change(
    make: impl Fn() -> App,
    commands: impl Fn(&App) -> Vec<ProductionItem>,
    reasons: impl Fn(&App) -> Vec<RejectReason>,
) {
    let mut app = make();
    let issued = commands(&app)
        .into_iter()
        .map(|item| queue(P0, item))
        .collect();
    let events = tick(&mut app, issued);
    assert_eq!(rejections(&events), reasons(&app));
    let mut quiet = make();
    tick(&mut quiet, Vec::new());
    assert_eq!(state_hash(app.world_mut()), state_hash(quiet.world_mut()));
    assert!(queued(&app, P0, "allied_barracks").is_empty());
    assert!(queued(&app, P0, "allied_yard").is_empty());
}

#[test]
fn an_item_of_another_side_is_rejected_without_a_factory_of_that_side() {
    // The militia has no prerequisites: only the side of the barracks decides.
    assert_rejected_without_change(
        || allied_base(["alpha", "gamma"], true),
        |app| vec![unit(app, "militia")],
        |app| vec![RejectReason::NoFactoryOfSide(unit(app, "militia"))],
    );
    assert_eq!(
        RejectReason::NoFactoryOfSide(ProductionItem::Unit(sim::UnitKind(7))).to_string(),
        "Unit(UnitKind(7)) is built by factories of another side, and the player owns none"
    );
}

#[test]
fn a_national_special_is_rejected_for_every_other_country() {
    // The sniper is alpha's; beta has every building it needs.
    assert_rejected_without_change(
        || allied_base(["beta", "gamma"], true),
        |app| vec![unit(app, "sniper")],
        |app| vec![RejectReason::CountryRequired(unit(app, "sniper"))],
    );
    // The raider is forbidden to alpha.
    assert_rejected_without_change(
        || allied_base(["alpha", "gamma"], true),
        |app| vec![unit(app, "raider")],
        |app| vec![RejectReason::CountryForbidden(unit(app, "raider"))],
    );
    // And each is accepted for the other country.
    let mut alpha = allied_base(["alpha", "gamma"], true);
    let sniper = unit(&alpha, "sniper");
    assert_eq!(rejections(&tick(&mut alpha, vec![queue(P0, sniper)])), NONE);
    let mut beta = allied_base(["beta", "gamma"], true);
    let raider = unit(&beta, "raider");
    assert_eq!(rejections(&tick(&mut beta, vec![queue(P0, raider)])), NONE);
    assert_eq!(
        RejectReason::CountryRequired(sniper).to_string(),
        format!("{sniper:?} is built only by other countries")
    );
    assert_eq!(
        RejectReason::CountryForbidden(raider).to_string(),
        format!("{raider:?} may not be built by the player's country")
    );
}

#[test]
fn a_national_special_stays_national_after_a_capture() {
    // The Soviet player (gamma) owns a captured Allied barracks and radar: it trains riflemen
    // there, but not the sniper, which only alpha builds; and alpha's own captured buildings do
    // not open the raider, which alpha may not build.
    let mut app = world(["gamma", "alpha"]);
    place(&mut app, P0, "soviet_yard", 4, 4);
    place(&mut app, P0, "soviet_power", 10, 4);
    place(&mut app, P0, "allied_barracks", 14, 4);
    place(&mut app, P0, "allied_radar", 18, 4);
    tick(&mut app, Vec::new());
    train(&mut app, P0, "rifleman");
    let sniper = unit(&app, "sniper");
    assert_eq!(
        rejections(&tick(&mut app, vec![queue(P0, sniper)])),
        [RejectReason::CountryRequired(sniper)]
    );
    let raider = unit(&app, "raider");
    assert_eq!(rejections(&tick(&mut app, vec![queue(P0, raider)])), NONE);
    // A player without any country is no member of a required list.
    assert!(
        app.world_mut()
            .resource_mut::<Players>()
            .set_country(P0, None)
    );
    assert_eq!(
        rejections(&tick(&mut app, vec![queue(P0, sniper)])),
        [RejectReason::CountryRequired(sniper)]
    );
}

#[test]
fn a_superweapon_building_is_rejected_while_the_option_is_off() {
    assert_rejected_without_change(
        || allied_base(["alpha", "gamma"], false),
        |app| vec![building(app, "storm_device")],
        |_| vec![RejectReason::DisabledByOptions],
    );
    assert_eq!(
        RejectReason::DisabledByOptions.to_string(),
        "the options of the match switch this off"
    );
    // Other buildings are not affected, and with the option on the device is queued.
    let mut off = allied_base(["alpha", "gamma"], false);
    let turret = building(&off, "turret");
    assert_eq!(rejections(&tick(&mut off, vec![queue(P0, turret)])), NONE);
    let mut on = allied_base(["alpha", "gamma"], true);
    let device = building(&on, "storm_device");
    assert_eq!(rejections(&tick(&mut on, vec![queue(P0, device)])), NONE);
    assert_eq!(queued(&on, P0, "allied_yard"), [device]);
}

#[test]
fn items_outside_the_tech_level_cannot_be_queued() {
    // Tech level -1 (the yards, the prototype) and a level above that of the rules (the
    // advanced trooper, 7 against 5) are not producible, whatever the player owns.
    assert_rejected_without_change(
        || allied_base(["alpha", "gamma"], true),
        |app| {
            vec![
                unit(app, "prototype"),
                unit(app, "advanced"),
                building(app, "allied_yard"),
            ]
        },
        |app| {
            vec![
                RejectReason::UnknownItem(unit(app, "prototype")),
                RejectReason::UnknownItem(unit(app, "advanced")),
                RejectReason::UnknownItem(building(app, "allied_yard")),
            ]
        },
    );
}

// --- Boundary: the order of the checks ----------------------------------------------------------

#[test]
fn the_checks_run_in_their_documented_order() {
    let mut app = world(["beta", "gamma"]);
    let definitions = defs(&app);
    let state = app
        .world()
        .resource::<Players>()
        .get(P0)
        .expect("P0")
        .clone();
    let check = |owned: &[BuildingKind], superweapons: bool, item: ProductionItem| {
        check_buildable(&definitions, &state, superweapons, owned, item)
    };
    let yard = kind(&app, "allied_yard");
    let barracks = kind(&app, "allied_barracks");
    let soviet_barracks = kind(&app, "soviet_barracks");
    let radar = kind(&app, "allied_radar");
    let sniper = unit(&app, "sniper");
    // Without anything: the first prerequisite of the economy file, before the group.
    assert_eq!(
        check(&[], true, sniper),
        Err(RejectReason::MissingPrerequisite {
            item: sniper,
            prerequisite: barracks,
        })
    );
    // With the barracks: the group (named by its first member).
    assert_eq!(
        check(&[barracks], true, sniper),
        Err(RejectReason::MissingPrerequisite {
            item: sniper,
            prerequisite: radar,
        })
    );
    // With every prerequisite: the country comes last.
    assert_eq!(
        check(&[barracks, radar], true, sniper),
        Err(RejectReason::CountryRequired(sniper))
    );
    // The engineer has only a group prerequisite: no barracks, no engineer; any barracks, one.
    let engineer = unit(&app, "engineer");
    assert_eq!(
        check(&[yard], true, engineer),
        Err(RejectReason::MissingPrerequisite {
            item: engineer,
            prerequisite: barracks,
        })
    );
    assert_eq!(check(&[soviet_barracks], true, engineer), Ok(()));
    // The militia: no factory of the group at all is `NoProducer`, one of the wrong side
    // `NoFactoryOfSide`.
    let militia = unit(&app, "militia");
    assert_eq!(
        check(&[yard], true, militia),
        Err(RejectReason::NoProducer {
            item: militia,
            producer: soviet_barracks,
        })
    );
    assert_eq!(
        check(&[yard, barracks], true, militia),
        Err(RejectReason::NoFactoryOfSide(militia))
    );
    assert_eq!(check(&[yard, soviet_barracks], true, militia), Ok(()));
    // The superweapon option is checked before the prerequisites, the tech level before all.
    let device = building(&app, "storm_device");
    assert_eq!(
        check(&[], false, device),
        Err(RejectReason::DisabledByOptions)
    );
    assert_eq!(
        check(&[], false, unit(&app, "prototype")),
        Err(RejectReason::UnknownItem(unit(&app, "prototype")))
    );
    // The same through the command: the first missing prerequisite of the economy file's list.
    place(&mut app, P0, "allied_yard", 4, 4);
    tick(&mut app, Vec::new());
    assert_eq!(
        rejections(&tick(&mut app, vec![queue(P0, device)])),
        [RejectReason::MissingPrerequisite {
            item: device,
            prerequisite: radar,
        }]
    );
}
