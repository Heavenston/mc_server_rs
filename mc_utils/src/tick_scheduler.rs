use std::{
    sync::{Arc, RwLock},
    thread::{sleep, spawn},
    time::{Duration, Instant},
};

fn interval(delay: Duration, mut callback: impl FnMut() -> ()) {
    let start = Instant::now();
    let mut i = 0;
    loop {
        i += 1;
        callback();
        let sleep_to = start + delay.checked_mul(i).unwrap();
        sleep(sleep_to.saturating_duration_since(Instant::now()));
    }
}

/// Struct containing info to profile ticks from the [TickScheduler]
pub struct TickProfiler {
    minimum_duration_per_ticks: Duration,
    ticks_since_last_check: u32,
    tick_duration_sum: Duration,
    profiling_interval: Duration,
}
impl TickProfiler {
    fn reset(&mut self) {
        self.ticks_since_last_check = 0;
        self.tick_duration_sum = Duration::from_nanos(0);
    }

    pub fn tick_per_seconds(&self) -> f64 {
        (self.ticks_since_last_check as f64) / (self.profiling_interval.as_secs_f64())
    }
    pub fn duration_per_tick(&self) -> Option<Duration> {
        if self.ticks_since_last_check == 0 {
            None
        } else {
            Some(
                self.tick_duration_sum
                    .div_f64(self.ticks_since_last_check as f64),
            )
        }
    }
}

/// Schedule ticks that happen every X times
/// Create it with the [TickSchedulerBuilder]
pub struct TickScheduler {
    profiler: Arc<RwLock<TickProfiler>>,
}

impl TickScheduler {
    /// Created a [TickSchedulerBuilder]
    pub fn builder() -> TickSchedulerBuilder {
        TickSchedulerBuilder::new()
    }

    /// Creates a [TickScheduler], but you probably want to use [TickScheduler::builder] instead
    pub fn new(minimum_duration_per_ticks: Duration, profiling_interval: Duration) -> Self {
        Self {
            profiler: Arc::new(RwLock::new(TickProfiler {
                minimum_duration_per_ticks: minimum_duration_per_ticks.clone(),
                ticks_since_last_check: 0,
                tick_duration_sum: Duration::from_nanos(0),
                profiling_interval,
            })),
        }
    }

    /// Starts the [TickScheduler] from the provided callbacks
    /// This will create a new thread if a profiler_callback is given
    pub fn start(
        self,
        mut tick_callback: impl FnMut() -> (),
        profiler_callback: Option<impl 'static + FnMut(&TickProfiler) -> () + Send + Sync>,
    ) {
        if let Some(mut profiler_callback) = profiler_callback {
            let profiling_interval = self.profiler.read().unwrap().profiling_interval.clone();
            let profiler = self.profiler.clone();
            spawn(move || {
                interval(profiling_interval, move || {
                    let mut profiler = profiler.write().unwrap();
                    profiler_callback(&*profiler);
                    profiler.reset();
                });
            });
        }

        let delay = self
            .profiler
            .read()
            .unwrap()
            .minimum_duration_per_ticks
            .clone();
        interval(delay, move || {
            let start = Instant::now();
            tick_callback();
            let duration = start.elapsed();
            let mut profiler = self.profiler.write().unwrap();
            profiler.ticks_since_last_check += 1;
            profiler.tick_duration_sum += duration;
        });
    }
}

/// Builder for the [TickScheduler]
pub struct TickSchedulerBuilder {
    minimum_duration_per_ticks: Duration,
    profiling_interval: Duration,
}
impl TickSchedulerBuilder {
    /// Creates a new [TickSchedulerBuilder] with default config
    pub fn new() -> Self {
        Self {
            minimum_duration_per_ticks: Duration::from_millis(50),
            profiling_interval: Duration::from_secs(2),
        }
    }

    /// Sets the minimum_duration_per_ticks which dictate the minimum delay between ticks
    pub fn minimum_duration_per_ticks(mut self, minimum_duration_per_ticks: Duration) -> Self {
        self.minimum_duration_per_ticks = minimum_duration_per_ticks;
        self
    }
    /// Sets the interval at which the interval callback will be called
    pub fn profiling_interval(mut self, profiling_interval: Duration) -> Self {
        self.profiling_interval = profiling_interval;
        self
    }

    /// Consumes the builder and create a [TickScheduler] based on the config
    pub fn build(self) -> TickScheduler {
        TickScheduler::new(self.minimum_duration_per_ticks, self.profiling_interval)
    }
}
