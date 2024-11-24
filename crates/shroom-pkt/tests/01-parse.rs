use either::Either;
use shroom_pkt::{test_util::test_enc_dec, CondEither, CondOption, EncodePacket};
use shroom_pkt_derive::{ShroomPacket, ShroomPacketEnum};

#[derive(ShroomPacket)]
pub struct Packet {
    name: u8,
    bitmask: u16,
}

#[derive(ShroomPacket)]
pub struct Packet2(u8, u16);

#[derive(Debug, Clone, Copy)]
#[repr(u16)]
pub enum TestOpCode {
    Action1 = 1,
}

impl From<TestOpCode> for u16 {
    fn from(val: TestOpCode) -> Self {
        val as u16
    }
}

impl TryFrom<u16> for TestOpCode {
    type Error = String;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(TestOpCode::Action1),
            _ => Err(format!("Invalid test opcode: {value}")),
        }
    }
}

#[derive(ShroomPacket, Debug, PartialEq, Eq)]
pub struct Packet3<'a> {
    name: &'a str,
    bitmask: u16,
}

fn check_name_even(name: &str) -> bool {
    name.len() % 2 == 0
}

#[derive(ShroomPacket, Debug, PartialEq, Eq)]
pub struct Packet4<'a, T> {
    name: &'a str,
    #[pkt(check(field = "name", check = "check_name_even"))]
    bitmask: CondOption<u16>,
    val: T,
}

fn check_n_even(n: &u32) -> bool {
    n % 2 == 0
}

#[derive(ShroomPacket, Debug, PartialEq, Eq)]
pub struct Packet5 {
    n: u32,
    #[pkt(either(field = "n", check = "check_n_even"))]
    either: CondEither<String, bool>,
}


#[derive(ShroomPacket, Debug, PartialEq, Eq)]
pub struct Packet6 {
    n: u32,
    #[pkt(size = "n")]
    data: Vec<u8>,
}

#[derive(ShroomPacket, Debug, PartialEq, Eq)]
pub struct Packet7(pub u32);

#[derive(ShroomPacket, Debug, PartialEq, Eq)]
pub struct Packet8(pub u32, pub u8);

#[derive(ShroomPacket, Debug, PartialEq, Eq)]
pub struct Packet9 {
    check: bool,
    #[pkt(either(field = "check"))]
    either: CondEither<String, bool>,
}

#[derive(ShroomPacket, Debug, PartialEq, Eq)]
pub struct Packet10 {
    #[pkt(cond_option = "u8")]
    either: Option<u32>
}

#[derive(ShroomPacketEnum, PartialEq, PartialOrd, Debug, Clone, Copy)]
#[repr(u8)]
pub enum Enum1 {
    B(u8) = 2,
    C((u32, u8)) = 3,
    D(u8, f32) = 4,
}

#[derive(ShroomPacketEnum, PartialEq, PartialOrd, Debug, Clone, Copy)]
#[repr(u32)]
pub enum Enum2 {
    B(u8) = 2,
    C((u32, u8)) = 3,
    D(u8, f32) = 4,
}

#[derive(ShroomPacketEnum, PartialEq, PartialOrd, Debug, Clone, Copy)]
#[repr(u32)]
pub enum Enum3 {
    A = 1,
    B = 2
}

fn main() {
    use shroom_pkt::test_enc_dec_borrow;
    assert_eq!(Packet::SIZE_HINT.0, Some(3));
    assert_eq!(Packet3::SIZE_HINT.0, None);

    test_enc_dec_borrow!(Packet3 {
        name: "aaa",
        bitmask: 1337,
    });

    test_enc_dec_borrow!(Packet4 {
        name: "aaa",
        bitmask: CondOption(None),
        val: 1337u16,
    });
    test_enc_dec_borrow!(Packet4 {
        name: "aaaa",
        bitmask: CondOption(Some(1337)),
        val: 1337u16,
    });

    test_enc_dec_borrow!(Packet5 {
        n: 2,
        either: CondEither(Either::Left("ABC".to_string()))
    });

    test_enc_dec_borrow!(Packet5 {
        n: 1,
        either: CondEither(Either::Right(false))
    });

    test_enc_dec_borrow!(Packet6 {
        n: 1,
        data: vec![0xaa]
    });

    test_enc_dec_borrow!(Packet7(1));
    test_enc_dec_borrow!(Packet8(1, 2));

    assert_eq!(Enum1::C((2, 3)).encode_len(), 1 + 4 + 1);
    test_enc_dec(Enum1::B(11));
    test_enc_dec(Enum1::C((2, 3)));
    test_enc_dec(Enum1::D(2, 5.0));

    assert_eq!(Enum2::C((2, 3)).encode_len(), 4 + 4 + 1);
    test_enc_dec(Enum2::B(11));
    test_enc_dec(Enum2::C((2, 3)));
    test_enc_dec(Enum2::D(2, 5.0));

    test_enc_dec_borrow!(Packet9 {
        check: false,
        either: CondEither(Either::Right(false))
    });

    test_enc_dec_borrow!(Packet9 {
        check: true,
        either: CondEither(Either::Left("ABC".to_string()))
    });

    test_enc_dec_borrow!(Packet10 {
        either: Some(11)
    });

    test_enc_dec_borrow!(Packet10 {
        either: None
    });
}
