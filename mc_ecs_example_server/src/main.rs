mod chunk_loader;

use mc_ecs_server_lib::{
    entity::chunk::{ChunkLoaderComponent, ChunkLocationComponent},
    mc_schedule::McSchedule,
};

use chunk_loader::StoneChunkLoader;
use legion::World;

use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    thread::sleep,
    time::{Duration, Instant},
};

fn ticker(tps_counter: Arc<AtomicUsize>, mut schedule: McSchedule, mut world: World) {
    let to_remove = world.push((
        ChunkLoaderComponent {
            radius: 10,
            loaded_chunks: Default::default(),
        },
        ChunkLocationComponent::new(0, 0),
    ));
    let mut counter = 0;
    loop {
        counter += 1;
        schedule.tick(&mut world);
        tps_counter.fetch_add(1, Ordering::Relaxed);
        if counter == 50000 {
            println!("Removed");
            world.remove(to_remove);
        }
        std::thread::yield_now();
    }
}

fn main() {
    let tps_counter = Arc::new(AtomicUsize::new(0));

    std::thread::spawn({
        let counter = tps_counter.clone();
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
        let ticks = tps_counter.load(Ordering::SeqCst);
        tps_counter.store(0, Ordering::SeqCst);
        let tps = ticks as f64 / delay.as_secs_f64();
        println!("TPS: {:.0}", tps);
        println!("MSPT: {:?}", Duration::from_secs_f64(1.0 / (tps.max(0.01))));

        let sleep_to = start + delay.checked_mul(i).unwrap();
        sleep(sleep_to.saturating_duration_since(Instant::now()));
    }
}
