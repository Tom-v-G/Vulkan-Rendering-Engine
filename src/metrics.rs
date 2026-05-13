use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};
use sysinfo::System;

pub struct FrameTimer {
    frame_times: VecDeque<f32>,
    max_samples: usize,
}

impl FrameTimer {
    pub fn new() -> Self {
        Self {
            frame_times: VecDeque::new(),
            max_samples: 100,
        }
    }

    pub fn update(&mut self, dt: f32) {
        self.frame_times.push_back(dt);

        if self.frame_times.len() > self.max_samples {
            self.frame_times.pop_front();
        }
    }

    pub fn fps(&self) -> f32 {
        let avg = self.frame_times.iter().sum::<f32>() / self.frame_times.len() as f32;

        if avg != 0.0 {
            1.0 / avg
        } else {
            0.0
        }
    }
}

pub struct EngineMetrics {
    sys: System,

    // Cached values shown in GUI
    pub frametimer: FrameTimer,
    pub rendered_chunks: u32,

    pub cpu_usage: f32,
    pub memory_mb: u64,

    // Update Timers
    last_frame: Instant,
    last_refresh: Instant,
    refresh_interval: Duration,
}

impl EngineMetrics {
    pub fn new() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();

        Self {
            sys,

            frametimer: FrameTimer::new(),
            rendered_chunks: 0,

            cpu_usage: 0.0,
            memory_mb: 0,

            last_frame: Instant::now(),
            last_refresh: Instant::now(),
            refresh_interval: Duration::from_secs(1), // refresh rate set to 1 sec.
        }
    }

    pub fn dt(&self) -> f32 {
        self.last_frame.elapsed().as_secs_f32()
    }

    pub fn update(&mut self) {
        self.frametimer.update(self.dt());

        // Only refresh some metrics once per interval
        if self.last_refresh.elapsed() >= self.refresh_interval {
            self.sys.refresh_cpu_usage();
            self.sys.refresh_memory();

            // Cache values
            self.cpu_usage = self.sys.global_cpu_usage();
            self.memory_mb = self.sys.used_memory() / 1024 / 1024;

            self.last_refresh = Instant::now();
        }

        self.last_frame = Instant::now()
    }
}
