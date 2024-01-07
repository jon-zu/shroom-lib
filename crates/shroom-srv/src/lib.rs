use futures::Future;

use util::clock::{GameClockRef, GameTime};

pub const MS_PER_TICK: u64 = 50;
pub const MSG_LIMIT_PER_TICK: usize = 100;

pub trait ServerId: Eq + Ord +  std::hash::Hash + Copy + Send + 'static {}
impl<T: Eq + Ord +  std::hash::Hash + Copy + Send + 'static> ServerId for T {}

pub mod util {
    pub mod clock;
    pub mod delay_queue;
    pub mod interval;
    pub mod poll_state;
    pub mod supervised_task;
    pub mod encode_buffer;
    #[cfg(test)]
    pub mod test_util;
}

pub mod srv {
    pub mod room_set;
    pub mod server_room;
    //pub mod server_socket2;
    //pub mod server_room2;
    pub mod server_session;
    pub mod server_socket;
    pub mod server_system;
}

pub mod rpc;
pub mod actor;
pub mod session;
pub mod runtime;

pub trait Context {
    fn create(clock_ref: GameClockRef) -> Self;

    fn time(&self) -> GameTime;
    fn wait_tick(&mut self) -> impl Future<Output = ()> + Send;
}