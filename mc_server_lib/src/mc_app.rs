use crate::entity::chunk::*;

use std::any::TypeId;

use bevy_ecs::schedule::{
    Schedule, SystemStage, SystemSet,
    StageLabel, StageLabelId, IntoSystemDescriptor
};
use bevy_ecs::world::World;

fn chunks_systems() -> SystemSet {
    SystemSet::new()
        .with_system(chunk_locations_update)
        .with_system(chunk_observer_chunk_loadings)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum McAppStage {
    BeforeTick,
    Tick,
    AfterTick,
}

impl StageLabel for McAppStage {
    fn as_str(&self) -> &'static str {
        match self {
            Self::BeforeTick => "before_tick",
            Self::Tick => "tick",
            Self::AfterTick => "after_tick",
        }
    }

    fn as_label(&self) -> StageLabelId {
        self.as_str().as_label()
    }

    fn type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }
}

/// Wrapper arroun the bevy_ecs's schedule that adds required systems from the lib
/// To add custom systems use [McSchedule::set_custom_schedule]
pub struct McApp {
    schedule: Schedule,
    pub world: World,
}

impl McApp {
    /// Creates a new [McSchedule]
    pub fn new() -> Self {
        let mut schedule = Schedule::default();
        let world = World::default();

        schedule.add_stage(McAppStage::BeforeTick, SystemStage::parallel());
        schedule.add_stage(McAppStage::Tick, SystemStage::parallel());
        schedule.add_stage(McAppStage::AfterTick, SystemStage::parallel());

        schedule.add_system_set_to_stage(McAppStage::Tick, chunks_systems());

        Self {
            schedule,
            world,
        }
    }

    pub fn add_system<Params>(
        &mut self, stage: McAppStage, system: impl IntoSystemDescriptor<Params>
    ) {
        self.schedule.add_system_to_stage(stage, system);
    }
    pub fn add_system_set(&mut self, stage: McAppStage, system: SystemSet) {
        self.schedule.add_system_set_to_stage(stage, system);
    }

    /// Execute "execute" on the created schedule
    pub fn tick(&mut self) {
        self.schedule.run_once(&mut self.world)
    }
}
