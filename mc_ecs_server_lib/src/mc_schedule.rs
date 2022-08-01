use std::marker::PhantomData;

use crate::{entity::chunk::*, event_handler::GlobalEventHandler};

use legion::{
    systems::{Resources, Schedule, Step},
    World,
};

fn chunks_schedule() -> Schedule {
    Schedule::builder()
        .add_system(chunk_locations_update_system())
        .add_system(chunk_observer_chunk_loadings_system())
        .build()
}

/// Wrapper arroun the legion's schedule that adds required systems from the lib
/// To add custom systems use [McSchedule::set_custom_schedule]
pub struct McSchedule<E: GlobalEventHandler + 'static> {
    pub resources: Resources,
    schedule: Schedule,
    global_event_handler: PhantomData<E>,
}

impl<E: GlobalEventHandler + 'static> McSchedule<E> {
    /// Creates a schedule and adds the given schedules at the end
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

    /// Creates a new [McSchedule]
    pub fn new(global_event_handler: E) -> Self {
        let mut resources = Resources::default();

        resources.insert(global_event_handler);

        Self {
            schedule: Self::create_schedule(&mut vec![]),
            resources,
            global_event_handler: PhantomData,
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
