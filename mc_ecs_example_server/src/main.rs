mod chunk_loader;

use mc_ecs_server_lib::{mc_schedule::McSchedule};

use chunk_loader::StoneChunkProvider;
use legion::{Schedule, World};

use crate::chunk_loader::*;
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    thread::sleep,
    time::{Duration, Instant},
};

fn ticker(tps_counter: Arc<AtomicUsize>) {
    let mut world: World = World::default();
    let mut schedule = McSchedule::new();

    let chunk_provider = Arc::new(StoneChunkProvider::new());

    schedule.set_custom_schedule(
        Schedule::builder()
            .add_system(stone_chunk_provider_system(Arc::clone(&chunk_provider)))
            .build(),
    );

    loop {
        schedule.tick(&mut world);
        tps_counter.fetch_add(1, Ordering::Relaxed);
        std::thread::yield_now();
    }
}

fn main() {
    let tps_counter = Arc::new(AtomicUsize::new(0));

    std::thread::spawn({
        let counter = tps_counter.clone();
        move || {
            ticker(counter);
        }
    });

    let start = Instant::now();
    let delay = Duration::from_secs(2);
    let mut i = 0;
    loop {
        i += 1;
        let ticks = tps_counter.load(Ordering::SeqCst);
        tps_counter.store(0, Ordering::SeqCst);
        let tps = ticks as f64 / delay.as_secs_f64();
        println!("TPS: {:.0}", tps);
        println!("MSPT: {:?}", Duration::from_secs_f64(1.0 / tps.max(0.01)));

        let sleep_to = start + delay.checked_mul(i).unwrap();
        sleep(sleep_to.saturating_duration_since(Instant::now()));
    }
}
