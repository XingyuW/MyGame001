//! `--smoke-test <frames>`: run the real application for a fixed number of frames, then exit.
//!
//! Exits with code 0 only if no asset failed to load and the spike map was spawned.

use bevy::asset::UntypedAssetLoadFailedEvent;
use bevy::prelude::*;
use sim::SimTick;

use crate::spike::SpikeStatus;

pub struct SmokeTestPlugin {
    pub frames: u32,
}

impl Plugin for SmokeTestPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(SmokeTest {
            frames: self.frames,
            rendered: 0,
        })
        .add_systems(Last, (fail_on_asset_load_error, count_frames).chain());
    }
}

#[derive(Resource, Debug)]
struct SmokeTest {
    frames: u32,
    rendered: u32,
}

fn fail_on_asset_load_error(
    mut failures: MessageReader<UntypedAssetLoadFailedEvent>,
    mut exit: MessageWriter<AppExit>,
) {
    for failure in failures.read() {
        error!(
            "smoke test failed: asset {} failed to load: {}",
            failure.path, failure.error
        );
        exit.write(AppExit::error());
    }
}

fn count_frames(
    mut smoke: ResMut<SmokeTest>,
    spike: Res<SpikeStatus>,
    tick: Res<SimTick>,
    mut exit: MessageWriter<AppExit>,
) {
    smoke.rendered += 1;
    if smoke.rendered < smoke.frames {
        return;
    }
    info!(
        "smoke test: {} frames, {} simulation ticks, spike map created: {}",
        smoke.rendered, tick.0, spike.map_created
    );
    if spike.map_created {
        exit.write(AppExit::Success);
    } else {
        error!(
            "smoke test failed: spike map was not created within {} frames",
            smoke.frames
        );
        exit.write(AppExit::error());
    }
}
