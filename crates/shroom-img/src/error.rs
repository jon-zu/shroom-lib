use std::io::Seek;

use crate::Offset;

#[derive(thiserror::Error, Debug)]
pub enum ImgError {
    #[error("unknown object type: `{0}`")]
    UnknownObjectType(String),
    #[error("Only Vec2 are allowed in a convex")]
    NoVec2InConvex,
    #[error("unknown string offset: {0}")]
    UnknownStringOffset(Offset),
    #[error("expected data offset")]
    ExpectedDataOffset,
    #[error("decompression failed: {1} at {0:X}")]
    DecompressionFailed(u64, std::io::Error),
}

impl ImgError {
    pub fn binrw_error<R: Seek>(self, mut r: R) -> binrw::error::Error {
        binrw::error::Error::Custom {
            pos: r.stream_position().unwrap_or(0),
            err: Box::new(self),
        }
    }
}
