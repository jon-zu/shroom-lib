use bytes::BytesMut;
use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use shroom_crypto::{RoundKey, ShandaCipher, ShroomVersion, net::net_cipher::NetCipher};
use shroom_net::codec::legacy::{codec::{LegacyDecoder, LegacyEncoder}, LegacyCipher};
use shroom_pkt::Packet;
use tokio_util::codec::{Decoder, Encoder};

const V83: ShroomVersion = ShroomVersion::new(83);

pub fn shanda_cipher_benchmark(c: &mut Criterion) {
    let mut bytes: [u8; 1024 * 16] = [0xFF; 1024 * 16];

    let mut group = c.benchmark_group("ShandaCipher");
    group.throughput(Throughput::Bytes(bytes.len() as u64));
    group.bench_function("decrypt", |b| {
        b.iter(|| ShandaCipher::decrypt(bytes.as_mut().into()))
    });
    group.bench_function("encrypt", |b| {
        b.iter(|| ShandaCipher::encrypt(bytes.as_mut().into()))
    });
    group.finish();
}

pub fn shroom_crypto_benchmark(c: &mut Criterion) {
    let mut bytes: [u8; 1024 * 16] = [0xFF; 1024 * 16];
    let mut shroom_crypto =
        NetCipher::<true>::new(Default::default(), RoundKey::zero(), V83);

    let mut group = c.benchmark_group("ShroomCrypto");
    group.throughput(Throughput::Bytes(bytes.len() as u64));
    group.bench_function("decrypt", |b| {
        b.iter(|| shroom_crypto.decrypt(bytes.as_mut().into()))
    });
    group.bench_function("encrypt", |b| {
        b.iter(|| shroom_crypto.encrypt(bytes.as_mut().into()))
    });
    group.finish();
}

pub fn shroom_crypto_no_shanda_benchmark(c: &mut Criterion) {
    let mut bytes: [u8; 1024 * 16] = [0xFF; 1024 * 16];
    let mut shroom_crypto =
        NetCipher::<false>::new(Default::default(), RoundKey::zero(), V83);

    let mut group = c.benchmark_group("ShroomCryptoNoShanda");
    group.throughput(Throughput::Bytes(bytes.len() as u64));
    group.bench_function("decrypt", |b| {
        b.iter(|| shroom_crypto.decrypt(bytes.as_mut().into()))
    });
    group.bench_function("encrypt", |b| {
        b.iter(|| shroom_crypto.encrypt(bytes.as_mut().into()))
    });
    group.finish();
}

pub fn shroom_framed_no_shanda_benchmark(c: &mut Criterion) {
    static BYTES: &'static [u8; 1024 * 16] = &[0xFF; 1024 * 16];
    let shroom_crypto =
        LegacyCipher::new(Default::default(), RoundKey::zero(), V83);

    let mut enc = LegacyEncoder::new(shroom_crypto.clone());
    let mut dec = LegacyDecoder::new(shroom_crypto.clone());
    let mut buf = BytesMut::new();
    let pkt = Packet::from_static(BYTES);

    let mut group = c.benchmark_group("ShroomFramedNoShanda");
    group.throughput(Throughput::Bytes(BYTES.len() as u64));
    group.bench_function("en_de_crypt", |b| {
        b.iter(|| {
            enc.encode(&pkt.clone(), &mut buf).unwrap();
            dec.decode(&mut buf).unwrap().unwrap();
        })
    });
    group.finish();
}

criterion_group!(
    benches,
    shanda_cipher_benchmark,
    shroom_crypto_benchmark,
    shroom_crypto_no_shanda_benchmark,
    shroom_framed_no_shanda_benchmark
);
criterion_main!(benches);
