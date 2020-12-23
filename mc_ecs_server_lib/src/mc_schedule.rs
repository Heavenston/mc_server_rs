use crate::entity::chunk::*;

use legion::{
    systems::{Resources, Schedule, Step},
    World,
};

fn chunks_schedule() -> Schedule {
    Schedule::builder()
        .add_system(chunk_locations_update_system())
        .flush()
        .add_system(chunk_observer_chunk_loadings_system())
        .build()
}

pub struct McSchedule {
    pub resources: Resources,
    schedule: Schedule,
}

impl McSchedule {
    fn create_schedule(other_schedules: &mut Vec<Schedule>) -> Schedule {
        let mut schedules = vec![chunks_schedule()];
        schedules.append(other_schedules);
        let mut final_schedule_steps = vec![];

        for schedule in schedules {
            final_schedule_steps.append(&mut schedule.into_vec());
            final_schedule_steps.push(Step::FlushCmdBuffers);
        }

        Schedule::from(final_schedule_steps)
    }

    pub fn new() -> Self {
        let resources = Resources::default();

        Self {
            schedule: Self::create_schedule(&mut vec![]),
            resources,
        }
    }

    /// Set the custom schedule that will be run at the end of every tick
    /// This will overwrite the previous schedule provided
    pub fn set_custom_schedule(&mut self, schedule: Schedule) {
        self.schedule = Self::create_schedule(&mut vec![schedule]);
    }

    /// Execute "execute" on the created schedule
    pub fn tick(&mut self, world: &mut World) {
        self.schedule.execute(world, &mut self.resources)
    }
}
