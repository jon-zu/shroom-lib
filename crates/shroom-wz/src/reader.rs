use std::sync::Arc;

use crate::WzContext;

#[derive(Debug)]
pub struct WzReader<T> {
    pub reader: T,
    pub ctx: Arc<WzContext>,
}

