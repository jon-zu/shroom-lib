use std::{
    sync::{atomic::AtomicU64, Arc},
    time::Duration,
};

use tokio::{sync::Notify, time::Instant};

pub const MS_PER_TICK: u64 = 50;

#[derive(Debug, Clone, Copy, Default, PartialEq, PartialOrd, Ord, Eq)]
pub struct GameTime(u64);

impl GameTime {
    pub fn add_dur(&self, dur: std::time::Duration) -> Self {
        Self(self.0 + dur.as_millis() as u64)
    }

    pub fn add_ms(&self, ms: u64) -> Self {
        Self(self.0 + ms)
    }

    pub fn ticks(&self) -> u64 {
        self.0 / MS_PER_TICK
    }

    pub fn add_ticks(&self, ticks: u64) -> Self {
        self.add_ms(ticks * MS_PER_TICK)
    }

    pub fn expired(&self, other: Self) -> bool {
        self.0 >= other.0
    }

    pub fn delta(&self, t: Self) -> GameTime {
        GameTime(t.0 - self.0)
    }
}

#[derive(Debug)]
struct Shared {
    clock: AtomicU64,
    notify: Notify,
    start: Instant,
}

impl Default for Shared {
    fn default() -> Self {
        Self::from_start(Instant::now())
    }
}

impl Shared {
    pub fn from_start(start: Instant) -> Self {
        Self {
            clock: AtomicU64::new(0),
            notify: Notify::new(),
            start,
        }
    }

    pub fn current_time(&self) -> GameTime {
        GameTime(self.clock.load(std::sync::atomic::Ordering::SeqCst))
    }

    pub fn tick(&self) {
        self.clock
            .fetch_add(MS_PER_TICK, std::sync::atomic::Ordering::SeqCst);
        self.notify.notify_waiters();
    }
}

#[derive(Debug, Default)]
pub struct GameClock(Arc<Shared>);

impl GameClock {
    /// Get the wait duration until the next tick
    pub fn wait_duration(&self) -> Duration {
        let elapsed = Instant::now().duration_since(self.0.start);
        let next = self.0.current_time().0 + MS_PER_TICK;
        if next <= elapsed.as_millis() as u64 {
            return Duration::from_millis(0);
        }
        Duration::from_millis(next - elapsed.as_millis() as u64)
    }

    pub async fn tick(&mut self) {
        tokio::time::sleep(self.wait_duration()).await;
        self.0.tick();
    }

    pub fn handle(&mut self) -> GameClockRef {
        GameClockRef {
            shared: self.0.clone(),
            time: self.0.current_time(),
        }
    }

    pub fn time(&self) -> GameTime {
        self.0.current_time()
    }
}

#[derive(Debug)]
pub struct GameClockRef {
    shared: Arc<Shared>,
    time: GameTime,
}

impl Clone for GameClockRef {
    fn clone(&self) -> Self {
        Self {
            shared: self.shared.clone(),
            time: self.time,
        }
    }
}

impl GameClockRef {
    pub fn time(&self) -> GameTime {
        self.time
    }

    fn add_tick(t: &mut GameTime) {
        *t = t.add_ms(MS_PER_TICK);
    }

    fn try_tick_inner(clock: &AtomicU64, time: &mut GameTime) -> bool {
        let cur = clock.load(std::sync::atomic::Ordering::SeqCst);

        // If the clock is ahead we can just increment it
        if cur > time.0 {
            Self::add_tick(time);
            true
        } else {
            false
        }
    }

    fn try_tick(&mut self) -> bool {
        Self::try_tick_inner(&self.shared.clock, &mut self.time)
    }

    pub async fn wait_tick(&mut self) {
        // Test if we lag behind
        if self.try_tick() {
            return;
        }

        // Otherwise we wait for the clock to tick
        let notify = self.shared.notify.notified();

        // Handle a tick happening while creating the notifier
        if Self::try_tick_inner(&self.shared.clock, &mut self.time) {
            return;
        }

        // Else we tick
        notify.await;
        Self::add_tick(&mut self.time);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn clock() {
        let mut clock = GameClock::default();
        let mut clock_ref = clock.handle();

        assert_eq!(clock_ref.time.ticks(), 0);
        clock.tick().await;

        assert_eq!(clock_ref.time.ticks(), 0);
        clock_ref.wait_tick().await;
        assert_eq!(clock_ref.time.ticks(), 1);

        tokio::spawn(async move {
            clock.tick().await;
        });

        clock_ref.wait_tick().await;
        assert_eq!(clock_ref.time.ticks(), 2);
    }

    #[tokio::test]
    async fn keep_up() {
        let mut clock = GameClock::default();
        let mut clock_ref = clock.handle();

        // Unable to tick per default
        assert!(!clock_ref.try_tick());

        clock.tick().await;
        clock.tick().await;

        // Try to tick twice, then we would have to wait again
        assert!(clock_ref.try_tick());
        assert!(clock_ref.try_tick());
        assert!(!clock_ref.try_tick());
    }
}
