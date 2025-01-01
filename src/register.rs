use crate::{deref::Deref, mi::Register};

#[derive(Debug, Clone)]
pub struct RegisterStorage {
    pub name: String,
    pub register: Option<Register>,
    pub deref: Deref,
}

impl RegisterStorage {
    pub fn new(name: String, register: Option<Register>, deref: Deref) -> Self {
        Self { name, register, deref }
    }
}
