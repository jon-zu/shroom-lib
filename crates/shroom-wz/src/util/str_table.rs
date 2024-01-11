use std::{collections::HashMap, rc::Rc};

pub type RcStr = Rc<str>;
pub type Offset = u32;

#[derive(Debug, Default)]
pub struct StrOffsetTable(HashMap<RcStr, Offset>);

impl StrOffsetTable {
    pub fn get(&self, s: &str) -> Option<Offset> {
        self.0.get(s).copied()
    }

    pub fn insert(&mut self, s: RcStr, offset: Offset) -> bool {
        self.0.insert(s, offset).is_none()
    }
}


#[derive(Debug, Default)]
pub struct OffsetStrTable(HashMap<Offset, RcStr>);

impl OffsetStrTable {
    pub fn get(&self, offset: Offset) -> Option<RcStr> {
        self.0.get(&offset).cloned()
    }

    pub fn insert(&mut self, offset: Offset, s: RcStr) -> bool {
        self.0.insert(offset, s).is_none()
    }
}