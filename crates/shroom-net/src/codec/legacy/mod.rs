use futures::Future;
use shroom_crypto::{net::net_cipher::NetCipher, SharedCryptoContext};
use shroom_pkt::shroom_enum_code;
use tokio::{
    io::AsyncWriteExt,
    net::{TcpStream, ToSocketAddrs},
};
use tokio_util::codec::{FramedRead, FramedWrite};

use crate::{NetResult, ShroomStream};

use self::{
    codec::{LegacyDecoder, LegacyEncoder},
    handshake::Handshake,
    handshake_gen::{BasicHandshakeGenerator, HandshakeGenerator},
};

use super::{ShroomCodec, ShroomTransport};

pub mod codec;
pub mod handshake;
pub mod handshake_gen;

pub const MAX_HANDSHAKE_LEN: usize = 24;
pub const MAX_PACKET_LEN: usize = i16::MAX as usize;
// Locale code for handshake, T means test server
shroom_enum_code!(
    LocaleCode,
    u8,
    Korea = 1,
    KoreaT = 2,
    Japan = 3,
    China = 4,
    ChinaT = 5,
    Taiwan = 6,
    TaiwanT = 7,
    Global = 8,
    Europe = 9,
    RlsPe = 10
);

/// Legacy codec
pub struct LegacyCodec<const SHANDA: bool, T = tokio::net::TcpStream> {
    crypto_ctx: SharedCryptoContext,
    handshake_gen: BasicHandshakeGenerator,
    _marker: std::marker::PhantomData<T>,
}

pub type LegacyCodecShanda<T> = LegacyCodec<true, T>;
pub type LegacyCodecNoShanda<T> = LegacyCodec<false, T>;

impl<const S: bool, T> Clone for LegacyCodec<S, T> {
    fn clone(&self) -> Self {
        Self {
            crypto_ctx: self.crypto_ctx.clone(),
            handshake_gen: self.handshake_gen.clone(),
            _marker: std::marker::PhantomData,
        }
    }
}

impl<const S: bool, T> Default for LegacyCodec<S, T> {
    fn default() -> Self {
        Self::new(
            SharedCryptoContext::default(),
            BasicHandshakeGenerator::v95(),
        )
    }
}

impl<const S: bool, T> LegacyCodec<S, T> {
    /// Creates a new legacy codedc from the crypto context and handshake generator
    pub fn new(crypto_ctx: SharedCryptoContext, handshake_gen: BasicHandshakeGenerator) -> Self {
        Self {
            crypto_ctx,
            handshake_gen,
            _marker: std::marker::PhantomData,
        }
    }

    /// Creates a new client codec from the given handshake
    fn create_client_codec(&self, handshake: &Handshake) -> (LegacyEncoder<S>, LegacyDecoder<S>) {
        let v = handshake.version;
        (
            LegacyEncoder::new(NetCipher::new(self.crypto_ctx.clone(), handshake.iv_enc, v)),
            LegacyDecoder::new(NetCipher::new(
                self.crypto_ctx.clone(),
                handshake.iv_dec,
                v.invert(),
            )),
        )
    }

    /// Creates a new server codec from the given handshake
    fn create_server_codec(&self, handshake: &Handshake) -> (LegacyEncoder<S>, LegacyDecoder<S>) {
        let v = handshake.version;
        (
            LegacyEncoder::new(NetCipher::new(
                self.crypto_ctx.clone(),
                handshake.iv_dec,
                v.invert(),
            )),
            LegacyDecoder::new(NetCipher::new(self.crypto_ctx.clone(), handshake.iv_enc, v)),
        )
    }

    /// Creates a new client stream, which will read the handshake and then create It
    async fn create_client_inner(&self, mut trans: T) -> NetResult<ShroomStream<Self>>
    where
        T: ShroomTransport + Sync,
    {
        let hshake = Handshake::read_handshake_async(&mut trans).await?;
        let (r, w) = trans.split();
        let (enc, dec) = self.create_client_codec(&hshake);
        let r = FramedRead::new(r, dec);
        let w = FramedWrite::new(w, enc);
        Ok(ShroomStream::new(w, r))
    }

    /// Creates a new server stream, which will send out the handshake
    async fn create_server_inner(&self, mut trans: T) -> NetResult<ShroomStream<Self>>
    where
        T: ShroomTransport + Sync,
    {
        let hshake = self.handshake_gen.generate_handshake();
        trans.write_all(&hshake.to_buf()).await?;
        let (r, w) = trans.split();
        let (enc, dec) = self.create_server_codec(&hshake);
        let r = FramedRead::new(r, dec);
        let w = FramedWrite::new(w, enc);
        Ok(ShroomStream::new(w, r))
    }
}

impl<const S: bool> LegacyCodec<S, TcpStream> {
    /// Connects to a server with the given address
    pub async fn connect(&self, addr: impl ToSocketAddrs) -> NetResult<ShroomStream<Self>> {
        let stream = TcpStream::connect(addr).await?;
        self.create_client_inner(stream).await
    }

    /// Accepts a connection from a client
    pub async fn accept(&self, stream: TcpStream) -> NetResult<ShroomStream<Self>> {
        self.create_server_inner(stream).await
    }
}

impl<const S: bool, T: ShroomTransport + Sync> ShroomCodec for LegacyCodec<S, T> {
    type Sink = FramedWrite<<Self::Transport as ShroomTransport>::WriteHalf, LegacyEncoder<S>>;
    type Stream = FramedRead<<Self::Transport as ShroomTransport>::ReadHalf, LegacyDecoder<S>>;
    type Transport = T;

    fn create_client(
        &self,
        trans: Self::Transport,
    ) -> impl Future<Output = NetResult<ShroomStream<Self>>> + Send {
        self.create_client_inner(trans)
    }

    fn create_server(
        &self,
        trans: Self::Transport,
    ) -> impl Future<Output = NetResult<ShroomStream<Self>>> + Send {
        self.create_server_inner(trans)
    }
}
