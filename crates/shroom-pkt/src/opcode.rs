use crate::{error::Error, PacketResult};

/// OpCode trait which allows conversion from and to the opcode from an `u16`
pub trait ShroomOpCode: TryFrom<u16> + Into<u16> + Copy + Clone + Send + Sync + PartialEq + Eq {
    /// Parses the opcode from an u16
    fn get_opcode(v: u16) -> PacketResult<Self> {
        Self::try_from(v).map_err(|_| Error::InvalidOpCode(v))
    }
}

/// Blanket implementation for u16
impl ShroomOpCode for u16 {}

/// Adds an opcode to the type by implementing this trait
pub trait HasOpCode {
    /// OpCode type
    type OpCode: ShroomOpCode;

    /// OpCode value
    const OPCODE: Self::OpCode;
}


/// Helper macro to easily implment `HasOpCode` for a packet
/// Example ```packet_opcode!(PingPacket, SendOpCode::Ping);```
#[macro_export]
macro_rules! with_opcode {
    ($packet_ty:ty, $op:path, $ty:ty) => {
        impl $crate::opcode::HasOpCode for $packet_ty {
            type OpCode = $ty;

            const OPCODE: Self::OpCode = $op;
        }
    };
    ($packet_ty:ty, $ty:ident::$op:ident) => {
        impl $crate::opcode::HasOpCode for $packet_ty {
            type OpCode = $ty;

            const OPCODE: Self::OpCode = $ty::$op;
        }
    };
}
