use std::{cmp::Reverse, collections::BinaryHeap};

use crate::GameTime;

#[derive(Debug, Clone)]
struct DelayItem<T> {
    data: T,
    timeout: GameTime,
}

impl<T> DelayItem<T> {
    fn is_before(&self, t: GameTime) -> bool {
        self.timeout <= t
    }
}

impl<T> PartialEq for DelayItem<T> {
    fn eq(&self, other: &Self) -> bool {
        self.timeout == other.timeout
    }
}

impl<T> Eq for DelayItem<T> {}

impl<T> PartialOrd for DelayItem<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for DelayItem<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.timeout.cmp(&other.timeout)
    }
}

pub struct DrainExpired<'a, T> {
    q: &'a mut DelayQueue<T>,
    t: GameTime,
}

impl<'a, T> Iterator for DrainExpired<'a, T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.q.try_pop(self.t)
    }
}

#[derive(Debug)]
pub struct DelayQueue<T>(BinaryHeap<Reverse<DelayItem<T>>>);

impl<T> Default for DelayQueue<T> {
    fn default() -> Self {
        Self(BinaryHeap::new())
    }
}

impl<T> DelayQueue<T> {
    pub fn new() -> Self {
        Self::default()
    }

    fn try_pop(&mut self, t: GameTime) -> Option<T> {
        if self.0.peek().map(|x| x.0.is_before(t)).unwrap_or(false) {
            self.0.pop().map(|x| x.0.data)
        } else {
            None
        }
    }

    pub fn push(&mut self, data: T, timeout: GameTime) {
        self.0.push(Reverse(DelayItem { data, timeout }));
    }

    pub fn pop(&mut self, t: GameTime) -> Option<T> {
        self.try_pop(t)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn drain_expired(&mut self, t: GameTime) -> DrainExpired<'_, T> {
        DrainExpired { q: self, t }
    }
}

#[cfg(test)]
mod tests {
    use crate::util::clock::GameTime;

    use super::*;
    use std::time::Duration;

    #[test]
    fn delay_queue_test() {
        let t = GameTime::default();
        let mut q = DelayQueue::new();
        q.push(1i32, t.add_dur(Duration::from_millis(10)));
        q.push(2, t.add_ms(20));

        assert_eq!(q.pop(t), None);
        let t = t.add_ms(10);
        assert_eq!(q.pop(t), Some(1));
        assert_eq!(q.pop(t), None);
        let t = t.add_ms(10);
        assert_eq!(q.pop(t), Some(2));
        assert_eq!(q.pop(t), None);

        q.push(4, t.add_ms(5));
        q.push(5, t.add_ms(10));

        let t = t.add_ms(10);
        assert_eq!(q.drain_expired(t).collect::<Vec<_>>(), vec![4, 5]);
    }
}
