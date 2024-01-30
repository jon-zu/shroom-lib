use std::{fmt::Debug, time::Duration};

use chrono::{DateTime, Utc};
use derive_more::{From, Into};
use nt_time::FileTime;

use crate::{packet_wrap, DecodePacket, EncodePacket};

/// Represents ticks from the win32 API `GetTickCount`
#[derive(Debug, From, Into, Clone, Copy)]
pub struct Ticks(pub u32);

packet_wrap!(Ticks<>, u32, u32);

/// Represents time from the win32 API `timeGetTime`
/// time since system start in seconds
#[derive(Debug, Default, Clone, From, Into)]
pub struct ClientTime(pub u32);

impl ClientTime {
    pub fn as_duration(&self) -> Duration {
        Duration::from_millis(self.0.into())
    }
}

packet_wrap!(ClientTime<>, u32, u32);

/// Represents an offset in terms
/// of ms relative to the client time
/// For example -1000 results on the client side to:
/// timeGetTime() - 1000
#[derive(Debug, Clone)]
pub struct ClientTimeOffset(pub DurationMs<i32>);

impl From<(bool, u32)> for ClientTimeOffset {
    fn from(v: (bool, u32)) -> Self {
        if v.0 {
            Self(DurationMs(-(v.1 as i32)))
        } else {
            Self(DurationMs(v.1 as i32))
        }
    }
}

impl From<ClientTimeOffset> for (bool, u32) {
    fn from(v: ClientTimeOffset) -> Self {
        let v = v.0 .0;
        (v >= 0, v.unsigned_abs())
    }
}

packet_wrap!(ClientTimeOffset<>, (bool, u32), (bool, u32));

/// Timestamps in the protocol
#[derive(Debug, PartialEq, PartialOrd, Copy, Clone)]
pub struct ShroomTime(FileTime);

impl From<ShroomTime> for DateTime<Utc> {
    fn from(value: ShroomTime) -> Self {
        value.0.into()
    }
}

impl TryFrom<DateTime<Utc>> for ShroomTime {
    type Error = crate::Error;

    fn try_from(value: DateTime<Utc>) -> Result<Self, Self::Error> {
        Ok(Self(value.try_into()?))
    }
}

/// Valid range for the time
pub const SHROOM_TIME_MIN: ShroomTime = ShroomTime::new(94354848000000000); // 1/1/1900
pub const SHROOM_TIME_MAX: ShroomTime = ShroomTime::new(150842304000000000); // 1/1/2079

impl ShroomTime {
    pub const fn new(v: u64) -> Self {
        Self(FileTime::new(v))
    }

    pub fn now() -> Self {
        Self(FileTime::now())
    }

    pub const fn is_min(&self) -> bool {
        self.raw() == SHROOM_TIME_MIN.raw()
    }

    pub fn is_max(&self) -> bool {
        self.raw() == SHROOM_TIME_MAX.raw()
    }

    pub const fn raw(&self) -> u64 {
        self.0.to_raw()
    }

    pub const fn max() -> Self {
        SHROOM_TIME_MAX
    }

    pub const fn min() -> Self {
        SHROOM_TIME_MIN
    }
}

// Encode/Decode helper
impl From<u64> for ShroomTime {
    fn from(v: u64) -> Self {
        Self::new(v)
    }
}
impl From<ShroomTime> for u64 {
    fn from(v: ShroomTime) -> Self {
        v.raw()
    }
}
packet_wrap!(ShroomTime<>, u64, u64);

/// Expiration time, can be either None or a time
#[derive(Debug, PartialEq, PartialOrd, Copy, Clone)]
pub struct ShroomExpirationTime(pub Option<ShroomTime>);

impl TryFrom<DateTime<Utc>> for ShroomExpirationTime {
    type Error = crate::Error;

    fn try_from(value: DateTime<Utc>) -> Result<Self, Self::Error> {
        Ok(Self(Some(value.try_into()?)))
    }
}

impl TryFrom<Option<DateTime<Utc>>> for ShroomExpirationTime {
    type Error = crate::Error;
    fn try_from(value: Option<DateTime<Utc>>) -> Result<Self, Self::Error> {
        Ok(Self(match value {
            Some(v) => Some(v.try_into()?),
            None => None,
        }))
    }
}

impl From<ShroomTime> for ShroomExpirationTime {
    fn from(value: ShroomTime) -> Self {
        Self(Some(value))
    }
}

impl From<Option<ShroomTime>> for ShroomExpirationTime {
    fn from(value: Option<ShroomTime>) -> Self {
        Self(value)
    }
}

impl ShroomExpirationTime {
    /// Create expiration from Shroom Time
    pub fn new(time: ShroomTime) -> Self {
        Self(Some(time))
    }

    /// Never expires
    pub fn never() -> Self {
        Self(None)
    }

    /// Create a delayed expiration from now + the duration
    pub fn delay(dur: chrono::Duration) -> Self {
        (Utc::now() + dur).try_into().unwrap()
    }
}

impl From<u64> for ShroomExpirationTime {
    fn from(v: u64) -> Self {
        Self((v != SHROOM_TIME_MAX.raw()).then_some(v.into()))
    }
}
impl From<ShroomExpirationTime> for u64 {
    fn from(v: ShroomExpirationTime) -> u64 {
        v.0.unwrap_or(SHROOM_TIME_MAX).raw()
    }
}
packet_wrap!(ShroomExpirationTime<>, u64, u64);

/// Represents a Duration in ms with the backed type
#[derive(Clone, Copy, PartialEq)]
pub struct DurationMs<T>(pub T);

impl<T: Debug> Debug for DurationMs<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}ms", self.0)
    }
}

impl<T: EncodePacket> EncodePacket for DurationMs<T> {
    const SIZE_HINT: crate::SizeHint = T::SIZE_HINT;
    fn encode_len(&self) -> usize {
        self.0.encode_len()
    }
    fn encode<B: bytes::BufMut>(&self, pw: &mut crate::PacketWriter<B>) -> crate::PacketResult<()> {
        self.0.encode(pw)
    }
}

impl<'de, T: DecodePacket<'de>> DecodePacket<'de> for DurationMs<T> {
    fn decode(pr: &mut crate::PacketReader<'de>) -> crate::PacketResult<Self> {
        Ok(Self(T::decode(pr)?))
    }
}

/// Convert a `Duration` into this MS duration type
impl<T> From<Duration> for DurationMs<T>
where
    T: TryFrom<u128>,
    T::Error: Debug,
{
    fn from(value: Duration) -> Self {
        Self(T::try_from(value.as_millis()).expect("Milli conversion"))
    }
}

/// Convert a DurationMS into a `Duration`
impl<T> From<DurationMs<T>> for Duration
where
    T: Into<u64>,
{
    fn from(value: DurationMs<T>) -> Self {
        Duration::from_millis(value.0.into())
    }
}

/// Duration ins ms, backed by u16
pub type ShroomDurationMs16 = DurationMs<u16>;
/// Duration in ms, backed by u32
pub type ShroomDurationMs32 = DurationMs<u32>;

#[derive(Debug, Copy, Clone, From, Into)]
pub struct ShroomTimeOffset(pub DurationMs<i32>);
packet_wrap!(ShroomTimeOffset<>, DurationMs<i32>, DurationMs<i32>);

#[cfg(test)]
mod tests {
    use proptest::prelude::*;
    use std::time::Duration;

    use crate::{
        test_util::{test_enc_dec, test_enc_dec_all},
        time::{ShroomDurationMs16, ShroomDurationMs32},
    };

    use super::{DurationMs, ShroomExpirationTime, ShroomTime};

    proptest! {
        #[test]
        fn q_dur16(dur: u16) {
            let dur = Duration::from_millis(dur.into());
            test_enc_dec::<ShroomDurationMs16>(dur.into());
        }

        #[test]
        fn q_dur32(dur: u32) {
            let dur = Duration::from_millis(dur.into());
            test_enc_dec::<ShroomDurationMs32>(dur.into());
        }
    }

    #[test]
    fn dur() {
        test_enc_dec_all([
            DurationMs::<u32>(1),
            Duration::from_millis(100 as u64).into(),
        ]);
    }

    #[test]
    fn expiration_time() {
        test_enc_dec_all([
            ShroomExpirationTime::never(),
            ShroomExpirationTime(None),
            ShroomExpirationTime::delay(chrono::Duration::seconds(1_000)),
            ShroomExpirationTime::new(ShroomTime::now()),
        ]);
    }
}
