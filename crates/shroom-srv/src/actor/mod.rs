use std::sync::{atomic::AtomicBool, Arc};

use tokio::sync::mpsc;

use crate::{util::interval::GameInterval, Context};

pub const MSG_PER_TICK: usize = 100;

#[derive(Debug, Default)]
struct Shared {
    cancel: AtomicBool
}


pub trait TickActor {
    type Msg: Send + 'static;
    type Ctx<'ctx>: Context + Send + 'ctx;

    fn on_msg(&mut self, ctx: &mut Self::Ctx<'_>, msg: Self::Msg) -> anyhow::Result<()>;
    fn on_update(&mut self, ctx: &mut Self::Ctx<'_>) -> anyhow::Result<()>;
}

#[derive(Debug)]
pub struct TickActorHandle<A: TickActor> {
    pub tx: mpsc::Sender<A::Msg>,
    pub task: tokio::task::JoinHandle<()>,
    shared: Arc<Shared>,
}

impl<A: TickActor> TickActorHandle<A> {
    pub fn cancel(&self) {
        self.shared.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

#[derive(Debug)]
pub struct TickActorRunner<A: TickActor> {
    actor: A,
    rx: mpsc::Receiver<A::Msg>,
    update_interval: GameInterval,
    shared: Arc<Shared>
}

impl<A: TickActor + Send + 'static> TickActorRunner<A> {
    pub fn new(actor: A, rx: mpsc::Receiver<A::Msg>) -> Self {
        Self {
            actor,
            rx,
            update_interval: GameInterval::new(1),
            shared: Arc::default()
        }
    }

    pub fn inner_mut(&mut self) -> &mut A {
        &mut self.actor
    }

    pub fn inner(&self) -> &A {
        &self.actor
    }

    pub fn spawn(actor: A, mut ctx: A::Ctx<'static>) -> TickActorHandle<A> {
        let (tx, rx) = mpsc::channel(128);
        let mut runner = Self::new(actor, rx);
        let shared = runner.shared.clone();
        let task = tokio::spawn(async move {
            if let Err(err) = runner.run(&mut ctx).await {
                log::info!("Error: {err:?}");
            }
        });

        TickActorHandle { tx, task, shared }
    }

    pub async fn run(&mut self, ctx: &mut A::Ctx<'_>) -> anyhow::Result<()> {
        while !self.shared.cancel.load(std::sync::atomic::Ordering::Relaxed) {
            self.run_once(ctx)?;
            ctx.wait_tick().await;
        }

        Ok(())
    }

    pub fn run_once(&mut self, ctx: &mut A::Ctx<'_>) -> anyhow::Result<()> {
        if self.update_interval.update(ctx.time()) {
            for _ in 0..MSG_PER_TICK {
                let Ok(msg) = self.rx.try_recv() else {
                    break;
                };
                self.actor.on_msg(ctx, msg)?;
            }

            self.actor.on_update(ctx)?;
        }

        Ok(())
    }
}

pub struct LocalTickActorRunner<A: TickActor> {
    actor: A,
    rx: mpsc::Receiver<A::Msg>,
    update_interval: GameInterval,
}

impl<A: TickActor> LocalTickActorRunner<A> {
    pub fn new(actor: A, rx: mpsc::Receiver<A::Msg>) -> Self {
        Self {
            actor,
            rx,
            update_interval: GameInterval::new(1),
        }
    }

    pub fn do_update(&mut self, ctx: &mut A::Ctx<'_>) -> anyhow::Result<()> {
        if self.update_interval.update(ctx.time()) {
            for _ in 0..MSG_PER_TICK {
                let Ok(msg) = self.rx.try_recv() else {
                    break;
                };
                self.actor.on_msg(ctx, msg)?;
            }

            self.actor.on_update(ctx)?;
        }

        Ok(())
    }
}
