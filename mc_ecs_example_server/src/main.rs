mod chunk_loader;

use crate::chunk_loader::*;
use chunk_loader::StoneChunkProvider;
use mc_ecs_server_lib::mc_schedule::McSchedule;
use mc_utils::tick_scheduler::{TickProfiler, TickScheduler};

use legion::{Schedule, World};
use std::{sync::Arc, time::Duration};

fn main() {
    let mut world: World = World::default();
    let mut schedule = McSchedule::new();

    let chunk_provider = Arc::new(StoneChunkProvider::new());

    schedule.set_custom_schedule(
        Schedule::builder()
            .add_system(stone_chunk_provider_system(Arc::clone(&chunk_provider)))
            .build(),
    );

    TickScheduler::builder()
        .profiling_interval(Duration::from_secs(3))
        .build()
        .start(
            move || {
                schedule.tick(&mut world);
            },
            Some(|profiler: &TickProfiler| {
                println!("TPS: {:.0}", profiler.tick_per_seconds());
                println!("DPT: {:?}", profiler.duration_per_tick());
            }),
        );
}
