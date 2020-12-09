use crate::{
    chunk_manager::{ChunkLoader, ChunkManager, ChunkScheduler},
    entity::chunk::*,
};

use legion::{
    systems::{Executor, ParallelRunnable, Resources, Schedule, Step},
    World,
};
use std::sync::Arc;

fn chunks_schedule() -> Schedule {
    Schedule::builder()
        .add_system(chunk_locations_update_system())
        .flush()
        .add_system(chunk_loaders_updates_system())
        .add_system(chunk_observer_chunk_loadings_system())
        .build()
}

pub struct McSchedule {
    pub chunk_manager: ChunkManager,
    pub resources: Resources,
    schedule: Schedule,
}

impl McSchedule {
    fn create_schedule() -> Schedule {
        let schedules = vec![chunks_schedule()];
        let mut final_schedule_steps = vec![];

        for schedule in schedules {
            final_schedule_steps.append(&mut schedule.into_vec());
            final_schedule_steps.push(Step::FlushCmdBuffers);
        }

        Schedule::from(final_schedule_steps)
    }

    pub fn new(chunk_loader: Arc<impl ChunkLoader + 'static>) -> Self {
        let mut resources = Resources::default();

        let chunk_manager = ChunkManager::new(chunk_loader);
        resources.insert(chunk_manager.scheduler());

        Self {
            chunk_manager,
            schedule: Self::create_schedule(),
            resources,
        }
    }

    /// Execute "execute" on the created schedule
    pub fn tick(&mut self, world: &mut World) {
        self.schedule.execute(world, &mut self.resources)
    }
}
