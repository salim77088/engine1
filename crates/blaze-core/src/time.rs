//! Timekeeping utilities — frame delta, elapsed time and fixed-step accumulator.

use instant::Instant;
use std::time::Duration;

/// Tracks elapsed time, delta time and a fixed-step accumulator used by
/// physics integration.
#[derive(Debug)]
pub struct Time {
    #[allow(dead_code)]
    start: Instant,
    last: Instant,
    delta: Duration,
    elapsed: Duration,
    fixed_step: Duration,
    accumulator: Duration,
    frame_count: u64,
}

impl Default for Time {
    fn default() -> Self {
        Self::new()
    }
}

impl Time {
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            start: now,
            last: now,
            delta: Duration::ZERO,
            elapsed: Duration::ZERO,
            fixed_step: Duration::from_secs_f64(1.0 / 60.0),
            accumulator: Duration::ZERO,
            frame_count: 0,
        }
    }

    /// Set the desired fixed-step duration (e.g. `1/60` for 60Hz physics).
    pub fn with_fixed_step(mut self, step: Duration) -> Self {
        self.fixed_step = step;
        self
    }

    /// Called at the start of every frame.
    pub fn tick(&mut self) {
        let now = Instant::now();
        self.delta = now - self.last;
        self.last = now;
        self.elapsed += self.delta;
        self.accumulator += self.delta;
        self.frame_count += 1;
    }

    /// Drain fixed-step ticks that should run this frame. Returns how many
    /// fixed updates the physics system should perform.
    pub fn drain_fixed_steps(&mut self) -> u32 {
        let mut steps = 0u32;
        while self.accumulator >= self.fixed_step && steps < 5 {
            self.accumulator -= self.fixed_step;
            steps += 1;
        }
        // Avoid spiral-of-death: clamp accumulator if it grew too large.
        if self.accumulator > self.fixed_step * 5 {
            self.accumulator = self.fixed_step * 5;
        }
        steps
    }

    #[inline]
    pub fn delta(&self) -> Duration { self.delta }

    #[inline]
    pub fn delta_secs(&self) -> f32 { self.delta.as_secs_f32() }

    #[inline]
    pub fn delta_secs_f64(&self) -> f64 { self.delta.as_secs_f64() }

    #[inline]
    pub fn elapsed(&self) -> Duration { self.elapsed }

    #[inline]
    pub fn elapsed_secs(&self) -> f32 { self.elapsed.as_secs_f32() }

    #[inline]
    pub fn frame_count(&self) -> u64 { self.frame_count }

    #[inline]
    pub fn fps(&self) -> f32 {
        if self.delta.is_zero() { 0.0 } else { 1.0 / self.delta_secs() }
    }

    #[inline]
    pub fn fixed_step_secs(&self) -> f32 { self.fixed_step.as_secs_f32() }
}
