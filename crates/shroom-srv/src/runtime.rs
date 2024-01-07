use std::{marker::PhantomData, net::IpAddr, ops::RangeInclusive, time::Duration};

use shroom_net::codec::ShroomCodec;
use tokio::net::TcpStream;

use crate::{
    rpc::{RpcListener, RpcService},
    srv::server_system::{ServerAcceptor, ServerSystem, SystemHandler},
    util::supervised_task::{SupervisedTask, SupervisedTaskHandle},
};

pub struct RuntimeConfig {
    pub bind_addr: IpAddr,
    pub login_port: u16,
    pub game_ports: RangeInclusive<u16>,
}

pub trait RuntimeHandler {
    type Ctx: Send + Sync + 'static;
    type Codec: ShroomCodec<Transport = TcpStream> + Clone + Send + Sync + 'static;
    type LoginService: RpcService<Ctx = Self::Ctx, Codec = Self::Codec> + Send + 'static;
    type System: SystemHandler + Send + 'static;
}

pub struct LoginTask<H: RuntimeHandler> {
    login: RpcListener<H::LoginService>,
    bind_addr: IpAddr,
    port: u16,
}
impl<H: RuntimeHandler> SupervisedTask for LoginTask<H> {
    type Context = ();

    async fn run(&mut self, _ctx: &mut Self::Context) -> anyhow::Result<()> {
        self.login.run_tcp((self.bind_addr, self.port)).await?;
        Ok(())
    }
}

pub struct ChannelTask<H: RuntimeHandler> {
    acceptor: ServerAcceptor<H::System, H::Codec>,
    bind_addr: IpAddr,
    port: u16,
}
impl<H: RuntimeHandler> SupervisedTask for ChannelTask<H>
where
    <H::System as SystemHandler>::Ctx: Sync + Send + 'static,
{
    type Context = ();

    async fn run(&mut self, _ctx: &mut Self::Context) -> anyhow::Result<()> {
        self.acceptor.run_tcp((self.bind_addr, self.port)).await?;
        Ok(())
    }
}

pub struct ServerRuntime<H: RuntimeHandler> {
    _handler: PhantomData<H>,
    system: ServerSystem<H::System>,
    login_task: LoginTask<H>,
    channel_task: ChannelTask<H>,
}

impl<H: RuntimeHandler + 'static> ServerRuntime<H>
where
    <H::System as SystemHandler>::Ctx: Sync + Send + 'static,
{
    pub fn new(
        cfg: RuntimeConfig,
        mut sys: ServerSystem<H::System>,
        cdc: H::Codec,
        ctx: H::Ctx,
    ) -> Self {
        let acceptor = sys.create_acceptor(cdc.clone()).unwrap();
        Self {
            _handler: PhantomData,
            login_task: LoginTask {
                login: RpcListener::new(cdc.clone(), ctx),
                bind_addr: cfg.bind_addr,
                port: cfg.login_port,
            },
            channel_task: ChannelTask {
                acceptor,
                bind_addr: cfg.bind_addr,
                port: *cfg.game_ports.start(),
            },
            system: sys,
        }
    }

    pub async fn run(mut self) -> anyhow::Result<()> {
        let _login = SupervisedTaskHandle::spawn(self.login_task, (), Duration::from_secs(1));
        let _channel = SupervisedTaskHandle::spawn(self.channel_task, (), Duration::from_secs(1));
        self.system.run().await?;
        Ok(())
    }
}
