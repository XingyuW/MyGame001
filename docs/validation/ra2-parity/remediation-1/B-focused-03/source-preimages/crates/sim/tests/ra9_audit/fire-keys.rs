//! Historical preparation note: UNVALIDATED/UNRUN native observations for three shipped false-only fire-key rows.
//! Test instrumentation observes after Combat and before Visibility; it changes no simulation
//! component, history, RNG, command or damage. It is never installed in production.
use super::AuditResult;
use bevy_app::App;
use bevy_ecs::{
    entity::Entity, message::Messages, resource::Resource, schedule::IntoScheduleConfigs,
    world::World,
};
use sim::{
    AiPlayers, AttackOrder, Cell, CellPos, Command, CommandQueue, Definitions, Grid, Health,
    IssuedCommand, Mobile, Orders, PlayerId, Players, SimCoreSet, SimEvent, SimId, SimPlugin,
    SimSchedule, SimSet, SimTick, SpecialState, Weapon,
};
use std::collections::BTreeMap;
const P0: PlayerId = PlayerId(0);
const P1: PlayerId = PlayerId(1);
const OPEN: Cell = Cell {
    passable: true,
    water: false,
    buildable: true,
    ore: 0,
};
#[derive(Clone, Debug)]
pub(super) struct Shot {
    pub(super) tick: u64,
    pub(super) attacker: SimId,
    pub(super) target: SimId,
    pub(super) weapon: sim::WeaponKind,
    pub(super) primary: sim::WeaponKind,
    pub(super) secondary: Option<sim::WeaponKind>,
    pub(super) tree: bool,
    pub(super) hidden_to_target_owner: bool,
    pub(super) stepping: bool,
    pub(super) destination: Option<CellPos>,
    pub(super) order: Option<AttackOrder>,
}
#[derive(Resource, Default)]
struct Trace {
    shots: Vec<Shot>,
}
fn observe(world: &mut World) {
    let states: BTreeMap<_, _> = world
        .query::<(&SimId, &Mobile, &Orders, &Weapon)>()
        .iter(world)
        .map(|(id, m, o, w)| {
            (
                *id,
                (
                    m.step.is_some(),
                    o.current.as_ref().map(|o| o.destination),
                    w.order,
                    w.kind,
                    w.secondary,
                ),
            )
        })
        .collect();
    let owners = world
        .query::<(&SimId, &sim::Owner)>()
        .iter(world)
        .map(|(id, o)| (*id, o.0))
        .collect::<BTreeMap<_, _>>();
    let tick = world.resource::<SimTick>().0;
    let events = world
        .resource::<Messages<SimEvent>>()
        .iter_current_update_messages()
        .cloned()
        .collect::<Vec<_>>();
    let mut captured = Vec::new();
    for event in events {
        if let SimEvent::WeaponFired {
            attacker,
            target,
            weapon,
        } = event
            && let Some((stepping, destination, order, primary, secondary)) = states.get(&attacker)
        {
            captured.push(Shot {
                tick,
                attacker,
                target,
                weapon,
                primary: *primary,
                secondary: *secondary,
                tree: world
                    .resource::<SpecialState>()
                    .mirage_trees
                    .contains_key(&attacker),
                hidden_to_target_owner: owners.get(&target).is_some_and(|owner| {
                    world
                        .resource::<SpecialState>()
                        .hidden
                        .contains(&(*owner, attacker))
                }),
                stepping: *stepping,
                destination: *destination,
                order: *order,
            });
        }
    }
    world.resource_mut::<Trace>().shots.extend(captured);
}
fn need(value: bool, reason: &str) -> AuditResult<()> {
    if value { Ok(()) } else { Err(reason.into()) }
}
fn world(defs: &Definitions) -> AuditResult<App> {
    let mut app = App::new();
    app.add_plugins(SimPlugin { seed: 11 });
    app.world_mut()
        .insert_resource(Grid::from_cells(48, 48, vec![OPEN; 48 * 48]).ok_or("grid setup failed")?);
    for (player, country) in [(P0, "great_britain"), (P1, "russia")] {
        let id = defs
            .rules()
            .and_then(|r| r.country_id(country))
            .ok_or("actual country setup")?;
        let mut players = app.world_mut().resource_mut::<Players>();
        players.add(player, 10_000).map_err(|e| e.to_string())?;
        need(
            players.set_country(player, Some(id)),
            "country setup failed",
        )?;
    }
    app.world_mut().insert_resource(defs.clone());
    app.world_mut().insert_resource(Trace::default());
    app.add_systems(
        SimSchedule,
        observe
            .after(SimCoreSet(SimSet::Combat))
            .in_set(SimSet::Combat),
    );
    need(
        app.world().resource::<AiPlayers>().controller(P0).is_none()
            && app.world().resource::<AiPlayers>().controller(P1).is_none(),
        "manual player probe must have no AI controllers",
    )?;
    Ok(app)
}
fn put(app: &mut App, p: PlayerId, id: &str, cell: CellPos) -> AuditResult<SimId> {
    sim::spawn_unit(app.world_mut(), p, id, cell)
        .map_err(|e| format!("native actor setup {id}: {e}"))
}
fn entity(app: &mut App, id: SimId) -> AuditResult<Entity> {
    let world = app.world_mut();
    world
        .query::<(Entity, &SimId)>()
        .iter(world)
        .find(|(_, found)| **found == id)
        .map(|(e, _)| e)
        .ok_or_else(|| format!("native actor gone {id:?}"))
}
fn hp(app: &mut App, id: SimId) -> AuditResult<i32> {
    let e = entity(app, id)?;
    app.world()
        .get::<Health>(e)
        .map(|h| h.hit_points)
        .ok_or("health missing".into())
}
fn issued(player: PlayerId, command: Command) -> IssuedCommand {
    IssuedCommand { player, command }
}
fn tick(app: &mut App, commands: Vec<IssuedCommand>) -> AuditResult<Vec<SimEvent>> {
    for c in commands {
        app.world_mut().resource_mut::<CommandQueue>().push(c);
    }
    app.world_mut().run_schedule(SimSchedule);
    let events = app
        .world()
        .resource::<Messages<SimEvent>>()
        .iter_current_update_messages()
        .cloned()
        .collect::<Vec<_>>();
    if events
        .iter()
        .any(|e| matches!(e, SimEvent::CommandRejected { .. }))
    {
        return Err(format!("native PREMISE command rejected: {events:?}"));
    }
    Ok(events)
}
fn disable_enemy_weapon_before_first_tick(app: &mut App, id: SimId) -> AuditResult<()> {
    let e = entity(app, id)?;
    app.world_mut().entity_mut(e).remove::<Weapon>();
    Ok(())
}
fn fired(events: &[SimEvent], a: SimId, b: SimId) -> bool {
    events
        .iter()
        .any(|e| matches!(e,SimEvent::WeaponFired{attacker,target,..}if *attacker==a&&*target==b))
}
#[derive(Clone, Debug)]
pub(super) struct ReturnFireObservation {
    pub(super) hostile_fire: bool,
    pub(super) actual_hit: bool,
    pub(super) hit_before_explicit_attack: bool,
    pub(super) move_preserved_after_hit: bool,
    pub(super) no_unsolicited_shot: bool,
    pub(super) explicit_attack_fired: bool,
    pub(super) hp_before: i32,
    pub(super) hp_after: i32,
    pub(super) destination: CellPos,
}
pub(super) fn observe_move_under_real_hit(
    defs: &Definitions,
) -> AuditResult<ReturnFireObservation> {
    let mut app = world(defs)?;
    let player = put(&mut app, P0, "grizzly", CellPos::new(8, 8))?;
    let attacker = put(&mut app, P1, "conscript", CellPos::new(11, 8))?;
    let destination = CellPos::new(8, 25);
    // Both ordinary public Moves suppress initial passive fire while native Visibility warms.
    // No HP/weapon/reload/history is forged; the hostile weapon stays completely intact.
    let warm = tick(
        &mut app,
        vec![
            issued(
                P0,
                Command::Move {
                    units: vec![player],
                    target: destination,
                },
            ),
            issued(
                P1,
                Command::Move {
                    units: vec![attacker],
                    target: CellPos::new(12, 8),
                },
            ),
        ],
    )?;
    need(
        !fired(&warm, attacker, player) && !fired(&warm, player, attacker),
        "warmup unexpectedly fired; real-hit premise must be restaged",
    )?;
    let before = hp(&mut app, player)?;
    let mut hostile_fire = false;
    let mut actual_hit = false;
    let mut preserved = true;
    let mut no_shot = true;
    let mut after = before;
    for n in 0..24 {
        let commands = if n == 0 {
            vec![issued(
                P1,
                Command::Attack {
                    units: vec![attacker],
                    target: player,
                },
            )]
        } else {
            Vec::new()
        };
        let events = tick(&mut app, commands)?;
        hostile_fire |= fired(&events, attacker, player);
        no_shot &= !fired(&events, player, attacker);
        after = hp(&mut app, player)?;
        let e = entity(&mut app, player)?;
        let w = app
            .world()
            .get::<Weapon>(e)
            .ok_or("player native weapon missing")?;
        let o = app
            .world()
            .get::<Orders>(e)
            .ok_or("player native orders missing")?;
        preserved &= w.order.is_none()
            && w.target.is_none()
            && o.current
                .as_ref()
                .is_some_and(|o| o.destination == destination);
        if after < before {
            actual_hit = true;
            break;
        }
    }
    need(
        hostile_fire && actual_hit,
        "native PREMISE requires actual hostile WeaponFired and actual HP loss after manual Move, not merely nearby enemy or scripted HP",
    )?;
    need(after > 0, "native victim must survive the controlled hit")?;
    // Preserve the public move across the next two ticks too; no attacker-id order override.
    for _ in 0..2 {
        let events = tick(&mut app, Vec::new())?;
        no_shot &= !fired(&events, player, attacker);
        let e = entity(&mut app, player)?;
        let w = app.world().get::<Weapon>(e).ok_or("weapon")?;
        let o = app.world().get::<Orders>(e).ok_or("orders")?;
        preserved &= w.order.is_none()
            && w.target.is_none()
            && o.current
                .as_ref()
                .is_some_and(|o| o.destination == destination);
    }
    let hit_before_explicit_attack = actual_hit;
    let mut explicit = false;
    for n in 0..12 {
        let events = tick(
            &mut app,
            if n == 0 {
                vec![issued(
                    P0,
                    Command::Attack {
                        units: vec![player],
                        target: attacker,
                    },
                )]
            } else {
                Vec::new()
            },
        )?;
        explicit |= fired(&events, player, attacker);
        if explicit {
            break;
        }
    }
    Ok(ReturnFireObservation {
        hostile_fire,
        actual_hit,
        hit_before_explicit_attack,
        move_preserved_after_hit: preserved,
        no_unsolicited_shot: no_shot,
        explicit_attack_fired: explicit,
        hp_before: before,
        hp_after: after,
        destination,
    })
}
#[derive(Clone, Debug)]
pub(super) struct AcquisitionObservation {
    pub(super) idle_without_hit: bool,
    pub(super) attack_move_without_hit: bool,
}
pub(super) fn observe_acquisition_controls(
    defs: &Definitions,
) -> AuditResult<AcquisitionObservation> {
    let mut idle = world(defs)?;
    let a = put(&mut idle, P0, "grizzly", CellPos::new(8, 8))?;
    let b = put(&mut idle, P1, "conscript", CellPos::new(11, 8))?;
    disable_enemy_weapon_before_first_tick(&mut idle, b)?;
    let before = hp(&mut idle, a)?;
    let mut idle_fired = false;
    for _ in 0..4 {
        let events = tick(&mut idle, Vec::new())?;
        need(!fired(&events, b, a), "idle control received hostile fire")?;
        idle_fired |= fired(&events, a, b);
        need(hp(&mut idle, a)? == before, "idle control was damaged")?;
        if idle_fired {
            break;
        }
    }
    let mut moving = world(defs)?;
    let a = put(&mut moving, P0, "grizzly", CellPos::new(8, 8))?;
    let b = put(&mut moving, P1, "conscript", CellPos::new(11, 8))?;
    disable_enemy_weapon_before_first_tick(&mut moving, b)?;
    let before = hp(&mut moving, a)?;
    tick(
        &mut moving,
        vec![issued(
            P0,
            Command::Move {
                units: vec![a],
                target: CellPos::new(8, 25),
            },
        )],
    )?;
    let mut attack_move = false;
    for n in 0..6 {
        let events = tick(
            &mut moving,
            if n == 0 {
                vec![issued(
                    P0,
                    Command::AttackMove {
                        units: vec![a],
                        target: CellPos::new(8, 25),
                    },
                )]
            } else {
                Vec::new()
            },
        )?;
        need(
            !fired(&events, b, a) && hp(&mut moving, a)? == before,
            "AttackMove control received damage",
        )?;
        attack_move |= fired(&events, a, b);
        if attack_move {
            break;
        }
    }
    Ok(AcquisitionObservation {
        idle_without_hit: idle_fired,
        attack_move_without_hit: attack_move,
    })
}
#[derive(Clone, Debug)]
pub(super) struct MirageObservation {
    pub(super) elite: bool,
    pub(super) initial_tree: bool,
    pub(super) moved_out_of_tree: bool,
    pub(super) shot: Shot,
    pub(super) event_weapon: String,
    pub(super) actual_primary: String,
    pub(super) actual_secondary: Option<String>,
}
pub(super) fn observe_mirage_undisguised(
    defs: &Definitions,
    elite: bool,
) -> AuditResult<MirageObservation> {
    let mut app = world(defs)?;
    let a = put(&mut app, P0, "mirage_tank", CellPos::new(8, 8))?;
    let b = put(&mut app, P1, "rhino", CellPos::new(16, 8))?;
    disable_enemy_weapon_before_first_tick(&mut app, b)?;
    if elite {
        need(
            sim::promote(app.world_mut(), a, sim::ELITE),
            "elite Mirage scenario promotion failed",
        )?;
    }
    let kind = defs.unit_kind("mirage_tank").ok_or("actual Mirage kind")?;
    let terms = defs.unit_combat(kind).ok_or("actual Mirage terms")?;
    let slots = terms.weapons(elite);
    let e = entity(&mut app, a)?;
    let armed = app
        .world()
        .get::<Weapon>(e)
        .ok_or("actual runtime Mirage weapon")?;
    need(
        Some(armed.kind) == slots[0] && armed.secondary == slots[1],
        "actual promoted weapon component differs from selected Definitions slots",
    )?;
    // Eight cells is outside actual primary seven-cell range but within Mirage sight.
    tick(&mut app, Vec::new())?;
    let initial_tree = app
        .world()
        .resource::<SpecialState>()
        .mirage_trees
        .contains_key(&a);
    need(
        initial_tree,
        "native PREMISE needs initial real idle Mirage tree",
    )?;
    need(
        !app.world()
            .resource::<Trace>()
            .shots
            .iter()
            .any(|s| s.attacker == a),
        "native initial Mirage must not have fired before movement",
    )?;
    tick(
        &mut app,
        vec![
            issued(
                P0,
                Command::Move {
                    units: vec![a],
                    target: CellPos::new(12, 8),
                },
            ),
            issued(
                P1,
                Command::Move {
                    units: vec![b],
                    target: CellPos::new(13, 8),
                },
            ),
        ],
    )?;
    let e = entity(&mut app, a)?;
    let moved = !app
        .world()
        .resource::<SpecialState>()
        .mirage_trees
        .contains_key(&a)
        && app
            .world()
            .get::<Mobile>(e)
            .is_some_and(|m| m.step.is_some());
    need(
        moved,
        "native PREMISE needs actual Move, step and removed Mirage tree",
    )?;
    for n in 0..80 {
        tick(
            &mut app,
            if n == 0 {
                vec![issued(
                    P0,
                    Command::Attack {
                        units: vec![a],
                        target: b,
                    },
                )]
            } else {
                Vec::new()
            },
        )?;
        if let Some(shot) = app
            .world()
            .resource::<Trace>()
            .shots
            .iter()
            .find(|s| s.attacker == a && s.target == b)
            .cloned()
        {
            let id = defs
                .weapon(shot.weapon)
                .ok_or("event weapon exists")?
                .id
                .clone();
            let primary = defs
                .weapon(shot.primary)
                .ok_or("observed actual runtime primary")?
                .id
                .clone();
            let secondary = shot
                .secondary
                .and_then(|k| defs.weapon(k))
                .map(|w| w.id.clone());
            need(
                !shot.tree,
                "native Mirage fired only after tree disguise returned; this is not false-flag evidence",
            )?;
            need(
                !shot.hidden_to_target_owner,
                "native shot remained logically concealed from the actual target owner",
            )?;
            need(
                matches!(shot.order,Some(AttackOrder::Attack{target,..})if target==b),
                "native fire must follow the actual public Attack target",
            )?;
            return Ok(MirageObservation {
                elite,
                initial_tree,
                moved_out_of_tree: moved,
                shot,
                event_weapon: id,
                actual_primary: primary,
                actual_secondary: secondary,
            });
        }
    }
    Err("native Mirage did not fire while undisguised within bounded ordinary chase".into())
}
#[derive(Clone, Debug)]
pub(super) struct Evidence {
    pub(super) returned: ReturnFireObservation,
    pub(super) acquisition: AcquisitionObservation,
    pub(super) mirage_base: MirageObservation,
    pub(super) mirage_elite: MirageObservation,
}
pub(super) fn collect(defs: &Definitions) -> AuditResult<Evidence> {
    Ok(Evidence {
        returned: observe_move_under_real_hit(defs)?,
        acquisition: observe_acquisition_controls(defs)?,
        mirage_base: observe_mirage_undisguised(defs, false)?,
        mirage_elite: observe_mirage_undisguised(defs, true)?,
    })
}
fn shipped() -> AuditResult<Definitions> {
    sim::load_ruleset(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/data"),
    )
    .map_err(|e| e.to_string())
}
#[test]
fn ra9_player_return_false_controlled_move_survives_actual_hostile_hit_without_order_override()
-> AuditResult<()> {
    let e = observe_move_under_real_hit(&shipped()?)?;
    need(
        e.hostile_fire && e.actual_hit && e.move_preserved_after_hit && e.no_unsolicited_shot,
        "manual Move real-hit contract failed",
    )
}
#[test]
fn ra9_player_return_false_preserves_idle_acquisition_without_prior_damage() -> AuditResult<()> {
    need(
        observe_acquisition_controls(&shipped()?)?.idle_without_hit,
        "idle source acquisition is not a return-fire flag",
    )
}
#[test]
fn ra9_player_return_false_preserves_public_attackmove_acquisition_without_prior_damage()
-> AuditResult<()> {
    need(
        observe_acquisition_controls(&shipped()?)?.attack_move_without_hit,
        "public AttackMove acquisition must remain available",
    )
}
#[test]
fn ra9_player_return_false_preserves_explicit_attack_after_real_damage() -> AuditResult<()> {
    let e = observe_move_under_real_hit(&shipped()?)?;
    need(
        e.hit_before_explicit_attack && e.explicit_attack_fired,
        "public Attack after real hit must remain available",
    )
}
#[test]
fn ra9_mirage_base_false_flag_fires_actual_primary_while_undisguised() -> AuditResult<()> {
    let e = observe_mirage_undisguised(&shipped()?, false)?;
    need(
        e.event_weapon == "mirage_gun" && e.actual_primary == e.event_weapon && !e.shot.tree,
        "base actual source weapon/no-disguise witness",
    )
}
#[test]
fn ra9_mirage_elite_false_flag_fires_actual_primary_without_claiming_secondary_e_fire()
-> AuditResult<()> {
    let e = observe_mirage_undisguised(&shipped()?, true)?;
    need(
        e.event_weapon == "mirage_gun"
            && e.actual_primary == e.event_weapon
            && e.actual_secondary.as_deref() == Some("mirage_gun_e")
            && !e.shot.tree,
        "elite actual selector/no-disguise witness; no MirageGunE firing claimed",
    )
}
