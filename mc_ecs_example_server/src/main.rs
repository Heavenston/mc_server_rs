mod chunk_loader;

use mc_ecs_server_lib::{chunk_manager::ChunkManager, mc_schedule::McSchedule};

use chunk_loader::StoneChunkLoader;
use legion::{World, WorldOptions};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, RwLock,
    },
    thread::sleep,
    time::{Duration, Instant},
};

fn ticker(counter: Arc<AtomicUsize>, mut schedule: McSchedule, mut world: World) {
    loop {
        schedule.tick(&mut world);
        counter.fetch_add(1, Ordering::Relaxed);
        std::thread::yield_now();
    }
}

fn main() {
    let counter = Arc::new(AtomicUsize::new(0));

    std::thread::spawn({
        let counter = counter.clone();
        move || {
            let world: World = World::default();
            let schedule = McSchedule::new(Arc::new(StoneChunkLoader));
            ticker(counter, schedule, world);
        }
    });

    let start = Instant::now();
    let delay = Duration::from_secs(2);
    let mut i = 0;
    loop {
        i += 1;
        let ticks = counter.load(Ordering::SeqCst);
        counter.store(0, Ordering::SeqCst);
        let tps = ticks as f64 / delay.as_secs_f64();
        println!("TPS: {:.0}", tps);
        println!("MSPT: {:?}", Duration::from_secs_f64(1.0 / (tps.max(0.01))));

        let sleep_to = start + delay.checked_mul(i).unwrap();
        sleep(sleep_to.saturating_duration_since(Instant::now()));
    }
}
