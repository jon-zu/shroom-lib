use super::clock::GameTime;

#[derive(Debug)]
pub struct GameInterval {
    last_update: GameTime,
    ticks: u64,
}

impl GameInterval {
    pub fn new(ticks: u64) -> Self {
        Self {
            last_update: GameTime::default(),
            ticks,
        }
    }

    pub fn reset(&mut self, t: GameTime) {
        self.last_update = t;
    }

    pub fn update(&mut self, t: GameTime) -> bool {
        if self.last_update.delta(t).ticks() >= self.ticks {
            self.last_update = t;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::util::clock::GameClock;

    use super::*;

    #[tokio::test]
    async fn interval() {
        let mut clock = GameClock::default();
        let mut iv = GameInterval::new(1);

        // Wait an initial tick
        clock.tick().await;

        assert!(iv.update(clock.time()));
        assert!(!iv.update(clock.time()));

        // Wait one tick
        clock.tick().await;

        assert!(iv.update(clock.time()));
        assert!(!iv.update(clock.time()));
    }
}
