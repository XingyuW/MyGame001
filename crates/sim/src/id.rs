use bevy_ecs::component::Component;
use bevy_ecs::resource::Resource;

/// Stable simulation identifier, assigned monotonically by [`SimIdAllocator`].
///
/// All order sensitive iteration sorts by this id, never by `Entity` or query order.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SimId(pub u32);

/// Hands out [`SimId`] values in increasing order, never reusing one.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SimIdAllocator {
    next: u32,
}

impl SimIdAllocator {
    /// Returns the next id, or `None` once the id space is exhausted (`u32::MAX` is never
    /// handed out, so the allocator itself cannot overflow).
    pub fn allocate(&mut self) -> Option<SimId> {
        let id = SimId(self.next);
        self.next = self.next.checked_add(1)?;
        Some(id)
    }

    /// The id that the next call to [`allocate`](Self::allocate) will return.
    pub fn peek_next(&self) -> u32 {
        self.next
    }
}

// Append to id.rs; the production allocator API/state remain unchanged.
#[cfg(test)]
mod ra9_boundary_tests {
    use super::*;
    use crate::{
        Cell, CellPos, Command, CommandQueue, Grid, IssuedCommand, PlayerId, Players,
        ProductionItem, RejectReason, SimEvent, SimPlugin, SimSchedule, state_hash,
    };
    use bevy_app::App;
    use bevy_ecs::message::Messages;
    fn ready_world() -> (App, crate::BuildingKind) {
        let path =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/data/ra2");
        let defs = crate::load_match_definitions(
            path.join("units.ron"),
            path.join("economy.ron"),
            path.join("combat.ron"),
        )
        .and_then(|d| d.with_rules(path.join("rules.ron")))
        .and_then(|d| d.with_arsenal(path.join("arsenal.ron")))
        .expect("RA2 fixture");
        let country = defs
            .rules()
            .expect("rules")
            .country_id("alpha")
            .expect("country");
        let power = defs.building_kind("allied_power").expect("power");
        let build = defs
            .production(ProductionItem::Building(power))
            .expect("terms")
            .build_time;
        let mut app = App::new();
        app.add_plugins(SimPlugin { seed: 7 });
        app.world_mut().insert_resource(
            Grid::from_cells(
                32,
                32,
                vec![
                    Cell {
                        passable: true,
                        water: false,
                        buildable: true,
                        ore: 0
                    };
                    1024
                ],
            )
            .expect("grid"),
        );
        app.world_mut().insert_resource(defs);
        app.world_mut()
            .resource_mut::<Players>()
            .add(PlayerId(0), 10_000)
            .expect("player");
        assert!(
            app.world_mut()
                .resource_mut::<Players>()
                .set_country(PlayerId(0), Some(country))
        );
        crate::spawn_building(
            app.world_mut(),
            PlayerId(0),
            "allied_yard",
            CellPos::new(2, 2),
        )
        .expect("yard");
        app.world_mut()
            .resource_mut::<CommandQueue>()
            .push(IssuedCommand {
                player: PlayerId(0),
                command: Command::QueueProduction {
                    item: ProductionItem::Building(power),
                },
            });
        for _ in 0..build * 4 {
            app.world_mut().run_schedule(SimSchedule);
        }
        let state = app
            .world()
            .resource::<Players>()
            .get(PlayerId(0))
            .expect("player");
        let head = state
            .lane(
                app.world()
                    .resource::<crate::Definitions>()
                    .queue_of(ProductionItem::Building(power))
                    .expect("queue"),
            )
            .first()
            .expect("ready queue");
        assert_eq!(head.item, ProductionItem::Building(power));
        assert!(head.progress >= build);
        let mut q = app.world_mut().query::<(&crate::Owner, &crate::Building)>();
        let owned = q
            .iter(app.world())
            .filter(|(o, _)| o.0 == PlayerId(0))
            .map(|(_, b)| (b.kind, b.footprint))
            .collect::<Vec<_>>();
        let defs = app.world().resource::<crate::Definitions>();
        let state = app
            .world()
            .resource::<Players>()
            .get(PlayerId(0))
            .expect("player");
        assert!(
            crate::check_placement(
                app.world().resource::<Grid>(),
                defs,
                state,
                owned.into_iter(),
                power,
                CellPos::new(2, 6)
            )
            .is_ok(),
            "actual placement guard premise"
        );
        (app, power)
    }
    #[test]
    fn ra9_last_native_allocation_and_exhausted_command_preserve_ready_state() {
        let (mut attempted, power) = ready_world();
        let (mut control, _) = ready_world();
        for app in [&mut attempted, &mut control] {
            let before = state_hash(app.world_mut());
            *app.world_mut().resource_mut::<SimIdAllocator>() =
                SimIdAllocator { next: u32::MAX - 1 };
            assert_ne!(state_hash(app.world_mut()), before, "allocator is hashed");
            assert_eq!(
                crate::spawn_unit(
                    app.world_mut(),
                    PlayerId(0),
                    "rifleman",
                    CellPos::new(10, 10)
                )
                .expect("last id"),
                SimId(u32::MAX - 1)
            );
            assert_eq!(
                app.world().resource::<SimIdAllocator>().peek_next(),
                u32::MAX
            );
            assert_eq!(
                app.world_mut().resource_mut::<SimIdAllocator>().allocate(),
                None
            );
            assert_eq!(
                app.world_mut().resource_mut::<SimIdAllocator>().allocate(),
                None
            );
        }
        attempted
            .world_mut()
            .resource_mut::<CommandQueue>()
            .push(IssuedCommand {
                player: PlayerId(0),
                command: Command::PlaceBuilding {
                    building: power,
                    origin: CellPos::new(2, 6),
                },
            });
        attempted.world_mut().run_schedule(SimSchedule);
        control.world_mut().run_schedule(SimSchedule);
        let reasons = attempted
            .world()
            .resource::<Messages<SimEvent>>()
            .iter_current_update_messages()
            .filter_map(|e| match e {
                SimEvent::CommandRejected { reason, .. } => Some(reason.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(reasons, [RejectReason::IdsExhausted]);
        assert_eq!(
            state_hash(attempted.world_mut()),
            state_hash(control.world_mut()),
            "consumed rejected input changes no queue/credits/grid/allocator state"
        );
        attempted.world_mut().run_schedule(SimSchedule);
        control.world_mut().run_schedule(SimSchedule);
        assert_eq!(
            state_hash(attempted.world_mut()),
            state_hash(control.world_mut())
        );
        let before = state_hash(attempted.world_mut());
        assert_eq!(
            crate::spawn_unit(
                attempted.world_mut(),
                PlayerId(0),
                "rifleman",
                CellPos::new(12, 12)
            ),
            Err(crate::SpawnError::IdsExhausted)
        );
        assert_eq!(
            state_hash(attempted.world_mut()),
            before,
            "no claimed cell on failed native spawn"
        );
    }
}
