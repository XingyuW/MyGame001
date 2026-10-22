//! Historical preparation note: UNCOMPILED / UNRUN. Fully provided test setup/receipt helpers; no simulation implementation.
//! Temporary RON variants are short-lived parser inputs; all edits are declared fixture values.
#![allow(dead_code)]
use bevy_app::App;
use bevy_ecs::{
    message::Messages, resource::Resource, schedule::IntoScheduleConfigs, world::World,
};
pub use sim::{
    Cell, CellPos, Command, Definitions, Grid, Health, IssuedCommand, PlayerId, Players, Props,
    RejectReason, SimCoreSet, SimEvent, SimId, SimPlugin, SimSchedule, SimSet, SimTick,
    SpecialState,
};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
pub const P0: PlayerId = PlayerId(0);
pub const P1: PlayerId = PlayerId(1);
pub const HOST: CellPos = CellPos::new(20, 20);
pub const KINDS: [(u8, &str); 3] = [
    (5, "civilian_house"),
    (6, "civilian_city"),
    (7, "civilian_tower"),
];
pub fn data() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/data")
}
pub fn shipped() -> Definitions {
    sim::load_ruleset(data()).unwrap_or_else(|e| panic!("{e}"))
}
pub fn world_of(definitions: Definitions) -> App {
    let mut app = App::new();
    app.add_plugins(SimPlugin { seed: 11 });
    app.world_mut().insert_resource(
        Grid::from_cells(
            48,
            48,
            vec![
                Cell {
                    passable: true,
                    buildable: true,
                    water: false,
                    ore: 0,
                };
                48 * 48
            ],
        )
        .expect("grid"),
    );
    for (p, country) in [(P0, "great_britain"), (P1, "russia")] {
        let id = definitions
            .rules()
            .and_then(|r| r.country_id(country))
            .expect("country");
        let mut players = app.world_mut().resource_mut::<Players>();
        players.add(p, 10_000).expect("player");
        assert!(players.set_country(p, Some(id)));
    }
    app.world_mut().insert_resource(definitions);
    app
}
pub fn world() -> App {
    world_of(shipped())
}
pub fn defs(app: &App) -> Definitions {
    app.world().resource::<Definitions>().clone()
}
pub fn put(app: &mut App, p: PlayerId, name: &str, x: i32, y: i32) -> SimId {
    if defs(app).unit_kind(name).is_some() {
        sim::spawn_unit(app.world_mut(), p, name, CellPos::new(x, y))
            .unwrap_or_else(|e| panic!("{name}: {e}"))
    } else {
        sim::spawn_building(app.world_mut(), p, name, CellPos::new(x, y))
            .unwrap_or_else(|e| panic!("{name}: {e}"))
    }
}
pub fn entity(app: &mut App, id: SimId) -> Option<bevy_ecs::entity::Entity> {
    let world = app.world_mut();
    world
        .query::<(bevy_ecs::entity::Entity, &SimId)>()
        .iter(world)
        .find(|(_, found)| **found == id)
        .map(|(e, _)| e)
}
pub fn hp(app: &mut App, id: SimId) -> Option<i32> {
    let e = entity(app, id)?;
    app.world().get::<Health>(e).map(|h| h.hit_points)
}
pub fn cell(app: &mut App, id: SimId) -> CellPos {
    let e = entity(app, id).expect("living mobile");
    app.world().get::<sim::Mobile>(e).expect("mobile").cell
}
pub fn passive_before_tick(app: &mut App, id: SimId) {
    assert_eq!(app.world().resource::<SimTick>().0, 0);
    let e = entity(app, id).expect("source");
    app.world_mut().entity_mut(e).remove::<sim::Weapon>();
}
pub fn mapped(app: &mut App, feature: u8, name: &str) -> SimId {
    assert_eq!(app.world().resource::<SimTick>().0, 0);
    assert!(app.world_mut().resource_mut::<Grid>().set_terrain(
        HOST,
        Cell {
            passable: false,
            buildable: false,
            water: false,
            ore: 0,
        }
    ));
    let mut looks = vec![0; 48 * 48];
    looks[20 * 48 + 20] = feature << 4;
    let d = defs(app);
    let kind = d.building_kind(name).expect("mapped kind");
    let props = Props::from_looks(
        app.world().resource::<Grid>(),
        &looks,
        d.building_combat(kind).expect("mapped terms").armour,
    );
    let host = props.id_at(HOST).expect("real mapped Prop");
    app.world_mut().insert_resource(props);
    assert!(entity(app, host).is_none());
    host
}
pub fn prop_hp(app: &App, id: SimId) -> Option<i32> {
    app.world()
        .resource::<Props>()
        .get(id)
        .map(|(_, p)| p.hit_points)
}
pub fn issue(p: PlayerId, command: Command) -> IssuedCommand {
    IssuedCommand { player: p, command }
}
pub fn interact(unit: SimId, target: SimId) -> IssuedCommand {
    issue(P0, Command::Interact { unit, target })
}
pub fn guard(ids: &[SimId]) -> IssuedCommand {
    issue(
        P0,
        Command::Guard {
            units: ids.to_vec(),
        },
    )
}
pub fn tick(app: &mut App, commands: Vec<IssuedCommand>) -> Vec<SimEvent> {
    for c in commands {
        app.world_mut().resource_mut::<sim::CommandQueue>().push(c);
    }
    app.world_mut().run_schedule(SimSchedule);
    app.world()
        .resource::<Messages<SimEvent>>()
        .iter_current_update_messages()
        .cloned()
        .collect()
}
pub fn run(app: &mut App, n: usize) -> Vec<SimEvent> {
    let mut out = vec![];
    for _ in 0..n {
        out.extend(tick(app, vec![]));
    }
    out
}
pub fn reasons(events: &[SimEvent]) -> Vec<RejectReason> {
    events
        .iter()
        .filter_map(|e| match e {
            SimEvent::CommandRejected { reason, .. } => Some(reason.clone()),
            _ => None,
        })
        .collect()
}
pub fn ok(events: &[SimEvent]) {
    assert!(reasons(events).is_empty(), "{events:?}");
}
pub fn rejected(events: &[SimEvent], reason: RejectReason) {
    assert_eq!(reasons(events), vec![reason], "{events:?}");
}
pub fn fired(events: &[SimEvent], source: SimId, target: SimId) -> bool {
    events.iter().any(|e| {
        matches!(e,SimEvent::WeaponFired {attacker,target:t,..}
        if *attacker==source && *t==target)
    })
}
pub fn prop_destroyed(events: &[SimEvent], target: SimId) -> bool {
    events
        .iter()
        .any(|e| matches!(e,SimEvent::PropDestroyed {prop,..} if *prop==target))
}
pub fn unit_destroyed(events: &[SimEvent], target: SimId) -> bool {
    events
        .iter()
        .any(|e| matches!(e,SimEvent::UnitDestroyed {unit,..} if *unit==target))
}
pub fn cells(app: &mut App) {
    assert!(sim::cell_invariant_violations(app.world_mut()).is_empty());
    assert!(sim::stray_claims(app.world_mut()).is_empty());
    for (_, p) in app.world().resource::<Players>().iter() {
        assert!(p.credits() >= 0);
    }
}
#[derive(Clone, Debug)]
pub struct Sample {
    pub source_hp: Option<i32>,
    pub target_hp: Option<i32>,
    pub hidden: bool,
    pub guard: bool,
    pub pending: bool,
    pub charged: bool,
    pub erasing: bool,
}
#[derive(Resource)]
pub struct Probe {
    pub source: SimId,
    pub target: SimId,
    pub commands: Option<Sample>,
    pub economy: Option<Sample>,
}
fn sample(world: &mut World) -> Sample {
    let (source, target) = {
        let p = world.resource::<Probe>();
        (p.source, p.target)
    };
    let source_hp = world
        .query::<(&SimId, &Health)>()
        .iter(world)
        .find(|(id, _)| **id == source)
        .map(|(_, h)| h.hit_points);
    let target_hp = world
        .resource::<Props>()
        .get(target)
        .map(|(_, p)| p.hit_points);
    let s = world.resource::<SpecialState>();
    let (hidden, pending, charged, erasing) = (
        s.hidden.contains(&(P0, target)),
        s.interactions.contains_key(&source),
        s.charges.contains_key(&target),
        s.erasures.contains_key(&target),
    );
    let guard = matches!(
        world.resource::<sim::TacticalState>().orders.get(&source),
        Some(sim::TacticalOrder::Guard { .. })
    );
    Sample {
        source_hp,
        target_hp,
        hidden,
        guard,
        pending,
        charged,
        erasing,
    }
}
fn after_commands(world: &mut World) {
    let s = sample(world);
    world.resource_mut::<Probe>().commands = Some(s);
}
fn before_economy(world: &mut World) {
    let s = sample(world);
    world.resource_mut::<Probe>().economy = Some(s);
}
pub fn probe(app: &mut App, source: SimId, target: SimId) {
    app.world_mut().insert_resource(Probe {
        source,
        target,
        commands: None,
        economy: None,
    });
    app.add_systems(
        SimSchedule,
        (
            after_commands
                .in_set(SimSet::Commands)
                .after(SimCoreSet(SimSet::Commands)),
            before_economy
                .in_set(SimSet::Economy)
                .before(SimCoreSet(SimSet::Economy)),
        ),
    );
}
// The following four edits form an INDEPENDENT fixture, not stock Nuke semantics:
// silo strategic=0, cooldown=3; Nuke effect damage=100000; Nuke warhead cell_spread=256.
// Every other loaded field is retained, including all five schema10 fields after A migration.
fn edit_row(text: &str, row: &str, field: &str, value: &str) -> String {
    let mut found = 0;
    let out = text
        .lines()
        .map(|line| {
            if !line.contains(row) {
                return line.to_owned();
            }
            found += 1;
            let key = format!("{field}:");
            assert_eq!(line.matches(&key).count(), 1);
            let start = line.find(&key).unwrap() + key.len();
            let end = start + line[start..].find(',').expect("scalar followed by comma");
            format!("{} {}{}", &line[..start], value, &line[end..])
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(found, 1, "fixture row {row}");
    out + "\n"
}
pub fn immediate_definitions() -> Definitions {
    let base = shipped();
    assert_eq!(
        base.effect(sim::Effect::Nuke).expect("stock effect").damage,
        600
    );
    let arsenal = std::fs::read_to_string(data().join("arsenal.ron")).expect("arsenal");
    let arsenal = edit_row(&arsenal, r#"(id: "nuclear_silo","#, "strategic", "0");
    let arsenal = edit_row(&arsenal, r#"(id: "nuclear_silo","#, "cooldown", "3");
    let combat = std::fs::read_to_string(data().join("combat.ron")).expect("combat");
    let combat = edit_row(
        &combat,
        r#"(id: "nuke", cell_spread:"#,
        "cell_spread",
        "256",
    );
    let combat = edit_row(&combat, r#"(id: "nuke", damage:"#, "damage", "100000");
    static SERIAL: AtomicUsize = AtomicUsize::new(0);
    let n = SERIAL.fetch_add(1, Ordering::Relaxed);
    let temp = PathBuf::from(std::env::var_os("TMPDIR").expect("configured TMPDIR"));
    let a = temp.join(format!(
        "ra9-prop-immediate-{}-{n}-arsenal.ron",
        std::process::id()
    ));
    let b = temp.join(format!(
        "ra9-prop-immediate-{}-{n}-combat.ron",
        std::process::id()
    ));
    std::fs::write(&a, arsenal).expect("short-lived fixture");
    std::fs::write(&b, combat).expect("short-lived fixture");
    let result = base.with_combat(&b).and_then(|d| d.with_arsenal(&a));
    let ra = std::fs::remove_file(a);
    let rb = std::fs::remove_file(b);
    ra.expect("remove before load assertions");
    rb.expect("remove before load assertions");
    let d = result.unwrap_or_else(|e| panic!("{e}"));
    let m = d
        .metadata(sim::ProductionItem::Building(
            d.building_kind("nuclear_silo").expect("silo"),
        ))
        .expect("metadata");
    assert_eq!(m.cooldown, 3);
    assert!(!m.strategic);
    assert_eq!(d.effect(sim::Effect::Nuke).expect("effect").damage, 100000);
    assert_eq!(
        d.warhead(d.warhead_kind("nuke").expect("warhead"))
            .expect("warhead")
            .cell_spread,
        256
    );
    assert_eq!(
        d.weapon(d.weapon_kind("nuke_payload").expect("stock weapon"))
            .expect("weapon")
            .damage,
        600
    );
    d
}
pub fn silo(app: &mut App) -> SimId {
    put(app, P0, "nuclear_reactor", 40, 2);
    put(app, P0, "nuclear_silo", 40, 40)
}
pub fn warm(app: &mut App, id: SimId) {
    for _ in 0..8 {
        if app
            .world()
            .resource::<SpecialState>()
            .charge
            .get(&id)
            .copied()
            .unwrap_or(0)
            >= 3
        {
            break;
        }
        ok(&tick(app, vec![]));
    }
    let d = defs(app);
    let kind = d.building_kind("nuclear_silo").expect("silo");
    let p = app.world().resource::<Players>().get(P0).expect("owner");
    assert!(!p.low_power());
    assert!(p.powers(&d, kind));
    assert_eq!(
        app.world().resource::<SpecialState>().charge.get(&id),
        Some(&3)
    );
}
pub fn strike(id: SimId, target: CellPos) -> IssuedCommand {
    issue(
        P0,
        Command::Superweapon {
            building: id,
            target,
            units: vec![],
        },
    )
}
pub fn strike_receipt(app: &App, events: &[SimEvent], id: SimId, target: CellPos) {
    let d = defs(app);
    let warhead = d.warhead_kind("nuke").expect("nuke");
    assert!(
        events.iter().any(
            |e| matches!(e,SimEvent::StrategicActivated {player,building,target:t,..}
        if *player==P0 && *building==id && *t==target)
        ),
        "{events:?}"
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e,SimEvent::Impact {warhead:h,..} if *h==warhead)),
        "{events:?}"
    );
}
