//! RA2: production by rate, the lanes of the Construction Yard, queue lengths and build limits
//! (docs/RA2_PARITY_PLAN.md sections 4.2 and 5, RA2 items 2 to 4; docs/DECISIONS.md R22, R23).
//!
//! Driven through `CommandQueue` and `SimSchedule` on the RA2 fixture (`tests/fixtures/data/ra2`;
//! invariant I10). The fixture's rules are those of Red Alert 2 for the rates (every further
//! factory divides the build time by 0.8; low power slows production to between 0.8 and 0.5)
//! and hold 3 items behind the one in production of a unit queue.

mod ra2_common;

use bevy_app::App;
use ra2_common::*;
use sim::{
    Command, Lane, MAX_COUNTED_FACTORIES, PlayerId, Players, ProductionItem, RejectReason,
    SimEvent, factory_rate_pm, power_rate_pm, production_rate_pm, state_hash,
};

/// An Allied base for P0 with `barracks` barracks and `power` power buildings (100 each; a
/// barracks consumes 10).
fn base(credits: i64, barracks: usize, power: usize) -> App {
    let mut app = world(credits);
    place(&mut app, P0, "allied_yard", 2, 2);
    for index in 0..power {
        place(&mut app, P0, "allied_power", 8 + 3 * index as i32, 2);
    }
    for index in 0..barracks {
        place(
            &mut app,
            P0,
            "allied_barracks",
            2 + 3 * (index % 12) as i32,
            8 + 3 * (index / 12) as i32,
        );
    }
    app
}

/// Queues `item` for P0 and returns the number of ticks, the first being the one that applies
/// the command, until `done` holds for the events of a tick; panics after `limit` ticks.
fn ticks_until(
    app: &mut App,
    item: ProductionItem,
    limit: u32,
    done: impl Fn(&[SimEvent]) -> bool,
) -> u32 {
    let mut events = tick(app, vec![queue(P0, item)]);
    assert_eq!(rejections(&events), NONE);
    for count in 1..=limit {
        if done(&events) {
            return count;
        }
        events = tick(app, Vec::new());
        assert_eq!(rejections(&events), NONE);
    }
    panic!("not done after {limit} ticks");
}

fn spawned(events: &[SimEvent]) -> bool {
    events
        .iter()
        .any(|event| matches!(event, SimEvent::UnitSpawned { .. }))
}

/// Ticks a rifleman (cost 100, build time 10) takes for P0 in `app`, and what it was charged.
fn rifleman(app: &mut App) -> (u32, i64) {
    let item = unit(app, "rifleman");
    let before = credits(app, P0);
    let count = ticks_until(app, item, 100, spawned);
    (count, before - credits(app, P0))
}

fn rules(app: &App) -> sim::ProductionRules {
    defs(app).rules().expect("rules").production
}

// --- Positive -----------------------------------------------------------------------------------

#[test]
fn one_factory_produces_one_unit_of_progress_per_tick_and_charges_the_cost() {
    // The rate of one factory with enough power is 1000 permille: the rule of M3, exactly.
    let mut app = base(1_000, 1, 1);
    assert_eq!(rifleman(&mut app), (10, 100));
    // A tank (cost 400, build time 20) at the one factory.
    place(&mut app, P0, "allied_factory", 20, 20);
    let tank = unit(&app, "tank");
    let before = credits(&app, P0);
    let mut charges = Vec::new();
    let mut last = before;
    let count = ticks_until(&mut app, tank, 1, |_| true);
    assert_eq!(count, 1);
    charges.push(last - credits(&app, P0));
    last = credits(&app, P0);
    for _ in 1..20 {
        tick(&mut app, Vec::new());
        charges.push(last - credits(&app, P0));
        last = credits(&app, P0);
    }
    assert_eq!(charges, [20; 20], "equation 3: cost / build time per tick");
    assert_eq!(before - credits(&app, P0), 400);
    assert_eq!(units_of(&mut app, P0, "tank"), 1);
}

#[test]
fn two_and_three_factories_divide_the_build_time_by_the_multiple_factory_factor() {
    // 1000, 1250 and 1562 permille per tick: 10 units of progress take 10, 8 and 7 ticks
    // (floor(1.25 x 8) = 10, floor(1.5625 x 7) = 10, and one tick less is not enough).
    for (barracks, expected) in [(1, 10), (2, 8), (3, 7)] {
        let mut app = base(1_000, barracks, 1);
        assert_eq!(
            rifleman(&mut app),
            (expected, 100),
            "{barracks} barracks: ticks, and the exact cost (I7)"
        );
    }
    // The factories of the other side count: they are of the same group. A factory of another
    // group does not.
    let mut mixed = base(1_000, 1, 1);
    place(&mut mixed, P0, "soviet_barracks", 20, 20);
    place(&mut mixed, P0, "allied_factory", 24, 20);
    assert_eq!(rifleman(&mut mixed), (8, 100));
}

#[test]
fn the_rates_are_those_of_the_plan() {
    let app = base(0, 0, 0);
    let rules = rules(&app);
    // 1000 x 5^(n-1) div 4^(n-1).
    let rates: Vec<u64> = (0..=4)
        .map(|factories| factory_rate_pm(&rules, factories))
        .collect();
    assert_eq!(rates, [1000, 1000, 1250, 1562, 1953]);
    // Low power: 1000 - 1000 x (1 - produced / consumed), within 500 and 800.
    assert_eq!(power_rate_pm(&rules, 100, 100), 1000, "exactly enough");
    assert_eq!(power_rate_pm(&rules, 100, 0), 1000);
    assert_eq!(power_rate_pm(&rules, 0, 0), 1000, "nothing consumes");
    assert_eq!(power_rate_pm(&rules, 100, 101), 800, "any shortage");
    assert_eq!(power_rate_pm(&rules, 100, 125), 800, "a fifth short");
    assert_eq!(power_rate_pm(&rules, 100, 150), 666);
    assert_eq!(power_rate_pm(&rules, 100, 200), 500, "half short");
    assert_eq!(power_rate_pm(&rules, 0, 10), 500, "no power at all");
    // The product, rounded down; with Super Me the factory rate times the Super Me factor,
    // whatever the power.
    assert_eq!(production_rate_pm(&rules, 2, 100, 150, false), 832);
    assert_eq!(production_rate_pm(&rules, 2, 100, 150, true), 1250 * 30);
    assert_eq!(production_rate_pm(&rules, 1, 0, 10, true), 1000 * 30);
}

#[test]
fn a_building_and_a_defence_are_built_at_once_in_the_two_lanes_of_the_yard() {
    let mut app = base(10_000, 0, 1);
    let (power, bunker) = (building(&app, "allied_power"), building(&app, "bunker"));
    let events = tick(&mut app, vec![queue(P0, power), queue(P0, bunker)]);
    assert_eq!(rejections(&events), NONE);
    // Both are at the head of a lane of their own and progress in the same tick.
    assert_eq!(defs(&app).lane(power), Lane::Main);
    assert_eq!(defs(&app).lane(bunker), Lane::Defences);
    let state = |app: &App| {
        let players = app.world().resource::<Players>();
        let state = players.get(P0).expect("player");
        (
            state.lane(yard_lane(app, Lane::Main)).to_vec(),
            state.lane(yard_lane(app, Lane::Defences)).to_vec(),
        )
    };
    let (main, defences) = state(&app);
    assert_eq!((main.len(), defences.len()), (1, 1));
    assert_eq!((main[0].item, main[0].progress), (power, 1));
    assert_eq!((defences[0].item, defences[0].progress), (bunker, 1));
    // The unit queues of the factories are lanes besides them.
    place(&mut app, P0, "allied_barracks", 20, 20);
    let rifleman = unit(&app, "rifleman");
    assert_eq!(rejections(&tick(&mut app, vec![queue(P0, rifleman)])), NONE);
    let events = ticks(&mut app, 18);
    let complete: Vec<ProductionItem> = events
        .iter()
        .filter_map(|event| match event {
            SimEvent::ProductionComplete { item, .. } => Some(*item),
            _ => None,
        })
        .collect();
    assert_eq!(complete.len(), 3, "{complete:?}");
    assert!(complete.contains(&power) && complete.contains(&bunker));
    assert_eq!(
        credits(&app, P0),
        10_000 - 300 - 250 - 100,
        "each charged its cost"
    );
    // Both buildings wait for their placement, each in its lane, and both can be placed.
    let (main, defences) = state(&app);
    assert_eq!((main[0].progress, defences[0].progress), (20, 20));
    let place_at = |kind, x, y| {
        issue(
            P0,
            Command::PlaceBuilding {
                building: kind,
                origin: sim::CellPos::new(x, y),
            },
        )
    };
    let commands = vec![
        place_at(kind(&app, "bunker"), 6, 6),
        place_at(kind(&app, "allied_power"), 2, 6),
    ];
    let events = tick(&mut app, commands);
    assert_eq!(rejections(&events), NONE);
    let (main, defences) = state(&app);
    assert!(main.is_empty() && defences.is_empty());
}

#[test]
fn low_power_slows_production_by_the_graded_factor() {
    // One power building (100) and one barracks (10), with factories (20 each) that leave the
    // player 10, 50 and 100 short: 110, 150 and 200 consumed. The rifleman's 10 units of
    // progress take 13 ticks at 800 permille, 16 at 666 and 20 at 500.
    for (factories, expected) in [(0, 10), (5, 13), (7, 16), (10, 20)] {
        let mut app = base(1_000, 1, 1);
        for index in 0..factories {
            place(
                &mut app,
                P0,
                "allied_factory",
                2 + 4 * (index % 8),
                30 + 3 * (index / 8),
            );
        }
        assert_eq!(
            rifleman(&mut app),
            (expected, 100),
            "{factories} factories: ticks, and the exact cost (I7)"
        );
        let state = app.world().resource::<Players>().get(P0).expect("player");
        assert_eq!(state.low_power(), factories > 0);
    }
}

#[test]
fn a_cancelled_item_refunds_what_it_was_charged_at_any_rate() {
    // Two barracks: 1.25 units of progress per tick. After 3 ticks the rifleman of 100 credits
    // and 10 ticks has made 3 units (and carries 750 permille) and paid 30.
    let mut app = base(1_000, 2, 1);
    let item = unit(&app, "rifleman");
    assert_eq!(rejections(&tick(&mut app, vec![queue(P0, item)])), NONE);
    ticks(&mut app, 2);
    let head = lane_of(&app, P0, item)[0];
    assert_eq!((head.progress, head.charged, head.fraction), (3, 30, 750));
    assert_eq!(credits(&app, P0), 970);
    let events = tick(
        &mut app,
        vec![issue(P0, Command::CancelProduction { item })],
    );
    assert!(events.contains(&SimEvent::ProductionCancelled {
        player: P0,
        item,
        refund: 30
    }));
    assert_eq!(credits(&app, P0), 1_000);
    assert!(lane_of(&app, P0, item).is_empty());
}

// --- Negative -----------------------------------------------------------------------------------

/// Asserts that `commands` of P0 in the world `make` builds are rejected with `reasons`, in
/// order, and change nothing: the same tick without them gives the same state hash.
fn assert_rejected_without_change(
    make: impl Fn() -> App,
    commands: impl Fn(&App) -> Vec<Command>,
    reasons: impl Fn(&App) -> Vec<RejectReason>,
) {
    let mut app = make();
    let issued = commands(&app)
        .into_iter()
        .map(|command| issue(P0, command))
        .collect();
    let events = tick(&mut app, issued);
    assert_eq!(rejections(&events), reasons(&app));
    let mut quiet = make();
    tick(&mut quiet, Vec::new());
    assert_eq!(state_hash(app.world_mut()), state_hash(quiet.world_mut()));
}

#[test]
fn a_lane_of_buildings_holds_one_item() {
    // Red Alert 2 does not queue buildings: while one is in production, or waits for its
    // placement, the lane takes no other; the other lane is not affected.
    let busy = || {
        let mut app = base(10_000, 0, 1);
        let power = building(&app, "allied_power");
        assert_eq!(rejections(&tick(&mut app, vec![queue(P0, power)])), NONE);
        app
    };
    let yard = |app: &App| kind(app, "allied_yard");
    assert_rejected_without_change(
        busy,
        |app| {
            vec![
                Command::QueueProduction {
                    item: building(app, "allied_power"),
                },
                Command::QueueProduction {
                    item: building(app, "allied_barracks"),
                },
            ]
        },
        |app| {
            vec![
                RejectReason::QueueFull {
                    producer: yard(app),
                    max: 1
                };
                2
            ]
        },
    );
    // The defence lane is free, and full with one defence.
    let mut app = busy();
    let (bunker, wall) = (building(&app, "bunker"), building(&app, "wall"));
    let events = tick(&mut app, vec![queue(P0, bunker), queue(P0, wall)]);
    assert_eq!(
        rejections(&events),
        [RejectReason::QueueFull {
            producer: yard(&app),
            max: 1
        }]
    );
    // A complete building still holds its lane until it is placed.
    ticks(&mut app, 25);
    let power = building(&app, "allied_power");
    assert_eq!(lane_of(&app, P0, power)[0].progress, 20);
    assert_eq!(
        rejections(&tick(&mut app, vec![queue(P0, power)])),
        [RejectReason::QueueFull {
            producer: yard(&app),
            max: 1
        }]
    );
}

#[test]
fn a_build_limit_counts_the_buildings_owned_and_queued() {
    // The storm device: one per player. The vault: two.
    let ready = || {
        let mut app = base(100_000, 0, 3);
        place(&mut app, P0, "allied_radar", 30, 30);
        app
    };
    let device = |app: &App| building(app, "storm_device");
    // Queued: the second is rejected, whatever lane it would wait in.
    let queued = || {
        let mut app = ready();
        let item = device(&app);
        assert_eq!(rejections(&tick(&mut app, vec![queue(P0, item)])), NONE);
        app
    };
    assert_rejected_without_change(
        queued,
        |app| vec![Command::QueueProduction { item: device(app) }],
        |app| {
            vec![RejectReason::BuildLimitReached {
                item: device(app),
                limit: 1,
            }]
        },
    );
    // Owned: the same.
    let owned = || {
        let mut app = ready();
        place(&mut app, P0, "storm_device", 30, 20);
        app
    };
    assert_rejected_without_change(
        owned,
        |app| vec![Command::QueueProduction { item: device(app) }],
        |app| {
            vec![RejectReason::BuildLimitReached {
                item: device(app),
                limit: 1,
            }]
        },
    );
    assert_eq!(
        RejectReason::BuildLimitReached {
            item: ProductionItem::Building(sim::BuildingKind(3)),
            limit: 1
        }
        .to_string(),
        "the player may have 1 of Building(BuildingKind(3)), and has or has queued as many"
    );
    // The other player's device does not count, and a player without one may queue it.
    let mut app = ready();
    place(&mut app, P1, "storm_device", 30, 20);
    let item = device(&app);
    assert_eq!(rejections(&tick(&mut app, vec![queue(P0, item)])), NONE);
}

// --- Boundary -----------------------------------------------------------------------------------

#[test]
fn a_build_limit_of_two_is_reached_by_one_owned_and_one_queued() {
    let mut app = base(100_000, 0, 1);
    let vault = building(&app, "vault");
    // None owned: one may be queued (the lane holds no second).
    assert_eq!(rejections(&tick(&mut app, vec![queue(P0, vault)])), NONE);
    ticks(&mut app, 30);
    let place_at = |app: &App, x| {
        issue(
            P0,
            Command::PlaceBuilding {
                building: kind(app, "vault"),
                origin: sim::CellPos::new(x, 6),
            },
        )
    };
    let command = place_at(&app, 2);
    let events = tick(&mut app, vec![command]);
    assert_eq!(rejections(&events), NONE);
    // One owned: the second may be queued; with it the limit of two is reached.
    assert_eq!(rejections(&tick(&mut app, vec![queue(P0, vault)])), NONE);
    ticks(&mut app, 30);
    let limit = RejectReason::BuildLimitReached {
        item: vault,
        limit: 2,
    };
    assert_eq!(
        rejections(&tick(&mut app, vec![queue(P0, vault)])),
        std::slice::from_ref(&limit),
        "one owned and one ready"
    );
    // Cancelling the queued one frees the place again; two owned do not.
    let events = tick(
        &mut app,
        vec![
            issue(P0, Command::CancelProduction { item: vault }),
            queue(P0, vault),
        ],
    );
    assert_eq!(rejections(&events), NONE);
    ticks(&mut app, 30);
    let command = place_at(&app, 5);
    let events = tick(&mut app, vec![command]);
    assert_eq!(rejections(&events), NONE);
    assert_eq!(rejections(&tick(&mut app, vec![queue(P0, vault)])), [limit]);
}

#[test]
fn a_unit_queue_holds_the_item_in_production_and_the_queued_ones_of_the_rules() {
    // The fixture's rules queue 3 behind the one in production: the fourth is the last slot.
    let mut app = base(10_000, 1, 1);
    let rifleman = unit(&app, "rifleman");
    assert_eq!(defs(&app).queue_capacity(rifleman), 4);
    let events = tick(&mut app, vec![queue(P0, rifleman); 4]);
    assert_eq!(rejections(&events), NONE);
    assert_eq!(lane_of(&app, P0, rifleman).len(), 4);
    let events = tick(&mut app, vec![queue(P0, rifleman)]);
    assert_eq!(
        rejections(&events),
        [RejectReason::QueueFull {
            producer: kind(&app, "allied_barracks"),
            max: 4
        }]
    );
    assert_eq!(lane_of(&app, P0, rifleman).len(), 4);
    // Once the first has left, a slot is free again; all four leave, each for its cost.
    ticks(&mut app, 9);
    assert_eq!(lane_of(&app, P0, rifleman).len(), 3);
    assert_eq!(rejections(&tick(&mut app, vec![queue(P0, rifleman)])), NONE);
    ticks(&mut app, 60);
    assert_eq!(units_of(&mut app, P0, "rifleman"), 5);
    assert_eq!(credits(&app, P0), 10_000 - 500);
}

#[test]
fn the_factory_factor_stops_at_its_cap() {
    let app = base(0, 0, 0);
    let rules = rules(&app);
    assert_eq!(MAX_COUNTED_FACTORIES, 10);
    // 1000 x 5^9 div 4^9.
    assert_eq!(factory_rate_pm(&rules, 10), 7450);
    assert_eq!(factory_rate_pm(&rules, 11), 7450);
    assert_eq!(factory_rate_pm(&rules, usize::MAX), 7450);
    assert!(factory_rate_pm(&rules, 9) < 7450);
    // In a match: ten and twelve barracks train a sniper (cost 400, build time 10) in the same
    // two ticks, and nine need no more ticks but carry less over.
    let trained = |barracks: usize| {
        let mut app = base(1_000, barracks, 2);
        place(&mut app, P0, "allied_radar", 30, 30);
        let sniper = unit(&app, "sniper");
        assert_eq!(rejections(&tick(&mut app, vec![queue(P0, sniper)])), NONE);
        let head = lane_of(&app, P0, sniper)[0];
        let count = 1 + ticks_until_spawned(&mut app);
        (
            count,
            (head.progress, head.fraction),
            1_000 - credits(&app, P0),
        )
    };
    assert_eq!(trained(10), (2, (7, 450), 400));
    assert_eq!(trained(12), trained(10));
    assert_eq!(trained(9), (2, (5, 960), 400));
}

fn ticks_until_spawned(app: &mut App) -> u32 {
    for count in 1..=100 {
        if spawned(&tick(app, Vec::new())) {
            return count;
        }
    }
    panic!("no unit left a factory");
}

#[test]
fn power_exactly_equal_to_the_drain_is_no_shortage() {
    // 100 produced; a barracks (10), four factories (80) and a depot (10) consume exactly 100.
    let mut exact = base(1_000, 1, 1);
    for index in 0..4 {
        place(&mut exact, P0, "allied_factory", 2 + 4 * index, 30);
    }
    place(&mut exact, P0, "depot", 30, 30);
    tick(&mut exact, Vec::new());
    let state = exact.world().resource::<Players>().get(P0).expect("player");
    assert_eq!((state.power_produced(), state.power_consumed()), (100, 100));
    assert!(!state.low_power());
    assert_eq!(rifleman(&mut exact), (10, 100));
    // One point more is a shortage, and the slowest rate that any shortage gives is 800.
    place(&mut exact, P0, "turret", 36, 36);
    tick(&mut exact, Vec::new());
    let state = exact.world().resource::<Players>().get(P0).expect("player");
    assert_eq!((state.power_produced(), state.power_consumed()), (100, 105));
    assert_eq!(rifleman(&mut exact), (13, 100));
}

// --- Regression ---------------------------------------------------------------------------------

#[test]
fn an_item_is_charged_exactly_its_cost_whatever_the_rate_and_the_credits() {
    // Invariant I7 with every rate: 1 to 4 barracks, with and without a shortage of power, and
    // a player who cannot always pay. Credits never fall below zero, progress waits for
    // credits, and the complete item has cost exactly its price.
    for barracks in 1..=4 {
        for short in [false, true] {
            let mut app = base(35, barracks, 1);
            if short {
                for index in 0..6 {
                    place(&mut app, P0, "allied_factory", 2 + 4 * index, 30);
                }
            }
            // The sniper costs 400 over 10 units: 40 a unit.
            place(&mut app, P0, "allied_radar", 30, 20);
            let sniper = unit(&app, "sniper");
            assert_eq!(rejections(&tick(&mut app, vec![queue(P0, sniper)])), NONE);
            let mut paid = 35 - credits(&app, P0);
            let mut waited = false;
            for step in 0..400 {
                let before = credits(&app, P0);
                assert!(before >= 0);
                let Some(head) = lane_of(&app, P0, sniper).first().copied() else {
                    break;
                };
                assert_eq!(head.charged, paid, "{barracks} barracks, step {step}");
                assert_eq!(head.charged, 40 * i64::from(head.progress));
                assert!(head.fraction < 1000);
                // A sale now and then is the player's only income: a bunker of 250 credits,
                // of which the rules refund 40 percent.
                let income = if step % 7 == 3 {
                    let bunker = place(&mut app, P0, "bunker", 38, 38);
                    app.world_mut()
                        .resource_mut::<sim::CommandQueue>()
                        .push(issue(P0, Command::SellBuilding { building: bunker }));
                    100
                } else {
                    0
                };
                let events = tick(&mut app, Vec::new());
                assert_eq!(rejections(&events), NONE);
                let after = credits(&app, P0);
                let charged = before + income - after;
                assert!(charged >= 0 && charged % 40 == 0, "whole units only");
                waited |= charged == 0;
                paid += charged;
            }
            assert!(waited, "the player had to wait for credits");
            assert_eq!(paid, 400, "{barracks} barracks, short of power: {short}");
            assert_eq!(units_of(&mut app, P0, "sniper"), 1);
        }
    }
}

#[test]
fn equal_commands_give_equal_states_at_every_rate() {
    // Determinism (I3) with fractions, both lanes and a shortage of power.
    let run = || {
        let mut app = base(5_000, 3, 1);
        for index in 0..5 {
            place(&mut app, P0, "allied_factory", 2 + 4 * index, 30);
        }
        let commands = vec![
            queue(P0, unit(&app, "rifleman")),
            queue(P0, unit(&app, "tank")),
            queue(P0, building(&app, "allied_power")),
            queue(P0, building(&app, "bunker")),
        ];
        let mut hashes = vec![];
        tick(&mut app, commands);
        for _ in 0..40 {
            tick(&mut app, Vec::new());
            hashes.push(state_hash(app.world_mut()));
        }
        hashes
    };
    assert_eq!(run(), run());
}

#[test]
fn a_player_id_outside_the_match_changes_nothing() {
    let mut app = base(1_000, 1, 1);
    let rifleman = unit(&app, "rifleman");
    let events = tick(&mut app, vec![queue(PlayerId(9), rifleman)]);
    assert_eq!(
        rejections(&events),
        [RejectReason::UnknownPlayer(PlayerId(9))]
    );
    // Nothing changed: the state is that of the same world after a tick without the command
    // (advisory review 1, tests P2-11: the name promised it, the test asserted the reason).
    let mut quiet = base(1_000, 1, 1);
    tick(&mut quiet, Vec::new());
    assert_eq!(state_hash(app.world_mut()), state_hash(quiet.world_mut()));
}
