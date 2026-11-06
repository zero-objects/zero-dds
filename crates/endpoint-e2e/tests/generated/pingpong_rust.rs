// Generated from zerodds_endpoint_e2e::IDL by codegen::source(Language::Rust);
// tests/rust_reliable.rs holds this file to the generator.
use zerodds_wire::{Reader, WireError, Writer};

#[derive(Debug, Clone, PartialEq)]
pub struct Ping {
    pub seq: i32,
    pub msg: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Pong {
    pub seq: i32,
    pub reply: String,
}

impl Ping {
    pub fn encode(&self, w: &mut Writer) -> Result<(), WireError> {
        w.write_i32(self.seq);
        w.write_string(&self.msg);
        Ok(())
    }

    pub fn decode(r: &mut Reader) -> Result<Self, WireError> {
        let seq = r.read_i32()?;
        let msg = r.read_string()?;
        Ok(Self { seq, msg })
    }
}

#[allow(clippy::derivable_impls)]
impl Default for Ping {
    fn default() -> Self {
        Ping { seq: 0, msg: String::new() }
    }
}

impl Pong {
    pub fn encode(&self, w: &mut Writer) -> Result<(), WireError> {
        w.write_i32(self.seq);
        w.write_string(&self.reply);
        Ok(())
    }

    pub fn decode(r: &mut Reader) -> Result<Self, WireError> {
        let seq = r.read_i32()?;
        let reply = r.read_string()?;
        Ok(Self { seq, reply })
    }
}

#[allow(clippy::derivable_impls)]
impl Default for Pong {
    fn default() -> Self {
        Pong { seq: 0, reply: String::new() }
    }
}
