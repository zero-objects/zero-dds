// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! AMI e2e of the **generated stub pattern** over real GIOP/IIOP. The stub
//! methods + the ReplyHandler + the pollers here are shaped exactly as
//! `ami_emit.rs` emits them (callback dispatch with typed values, poller with
//! typed `get_reply`), and are driven through the `AmiClient` (as
//! `AsyncCorbaChannel`) against a real server. Proves the typed AMI path
//! end-to-end, which `compile_check_ami` only checks statically.

use std::sync::{Arc, Mutex};

use zerodds_cdr::{BufferReader, BufferWriter, CdrDecode, CdrEncode, Endianness};
use zerodds_corba_interop::runtime::{AmiClient, CorbaServer, object_reference};
use zerodds_corba_rust::{AsyncCorbaChannel, CorbaException, SkeletonResult};

const MARSHAL: CorbaException = CorbaException::SystemException {
    minor: 0,
    message: "CORBA MARSHAL: CDR error",
};

// --- generated pattern: ReplyHandler trait (callback model §22.5) ----------
trait MathAmiHandler: Send + Sync {
    fn add(&self, __return: i32);
    fn add_excep(&self, __excep: CorbaException);
    fn divmod(&self, q: i32, r: i32);
    fn divmod_excep(&self, __excep: CorbaException);
}

// --- generated pattern: poller (polling model §22.6) -----------------------
struct MathAddPoller {
    request_id: u32,
}
impl MathAddPoller {
    fn get_reply(&self, ch: &mut dyn AsyncCorbaChannel) -> Result<i32, CorbaException> {
        let (b, e) = ch.get_reply(self.request_id)??;
        let mut r = BufferReader::new(&b, e);
        let __ret = i32::decode(&mut r).map_err(|_| MARSHAL)?;
        Ok(__ret)
    }
}
struct MathDivmodPoller {
    request_id: u32,
}
impl MathDivmodPoller {
    fn get_reply(&self, ch: &mut dyn AsyncCorbaChannel) -> Result<(i32, i32), CorbaException> {
        let (b, e) = ch.get_reply(self.request_id)??;
        let mut r = BufferReader::new(&b, e);
        let q = i32::decode(&mut r).map_err(|_| MARSHAL)?;
        let rr = i32::decode(&mut r).map_err(|_| MARSHAL)?;
        Ok((q, rr))
    }
}

// --- generated pattern: stub with sendc_/sendp_ ------------------------------
struct MathStub {
    object_ref: zerodds_corba_rust::ObjectReference,
}
impl MathStub {
    fn sendc_add(
        &self,
        ch: &mut dyn AsyncCorbaChannel,
        handler: Arc<dyn MathAmiHandler>,
        a: i32,
        b: i32,
    ) -> Result<u32, CorbaException> {
        let _ = &self.object_ref; // (in the AMI path the channel is target-bound)
        let mut w = BufferWriter::new(Endianness::Big);
        a.encode(&mut w).map_err(|_| MARSHAL)?;
        b.encode(&mut w).map_err(|_| MARSHAL)?;
        ch.send(
            "add",
            &w.into_bytes(),
            Box::new(move |reply| match reply {
                Ok((body, e)) => {
                    let mut r = BufferReader::new(&body, e);
                    let __ret = match i32::decode(&mut r) {
                        Ok(v) => v,
                        Err(_) => {
                            handler.add_excep(MARSHAL);
                            return;
                        }
                    };
                    handler.add(__ret);
                }
                Err(exc) => handler.add_excep(exc),
            }),
        )
    }

    fn sendc_divmod(
        &self,
        ch: &mut dyn AsyncCorbaChannel,
        handler: Arc<dyn MathAmiHandler>,
        a: i32,
        b: i32,
    ) -> Result<u32, CorbaException> {
        let mut w = BufferWriter::new(Endianness::Big);
        a.encode(&mut w).map_err(|_| MARSHAL)?;
        b.encode(&mut w).map_err(|_| MARSHAL)?;
        ch.send(
            "divmod",
            &w.into_bytes(),
            Box::new(move |reply| match reply {
                Ok((body, e)) => {
                    let mut r = BufferReader::new(&body, e);
                    let q = match i32::decode(&mut r) {
                        Ok(v) => v,
                        Err(_) => {
                            handler.divmod_excep(MARSHAL);
                            return;
                        }
                    };
                    let rr = match i32::decode(&mut r) {
                        Ok(v) => v,
                        Err(_) => {
                            handler.divmod_excep(MARSHAL);
                            return;
                        }
                    };
                    handler.divmod(q, rr);
                }
                Err(exc) => handler.divmod_excep(exc),
            }),
        )
    }

    fn sendp_add(
        &self,
        ch: &mut dyn AsyncCorbaChannel,
        a: i32,
        b: i32,
    ) -> Result<MathAddPoller, CorbaException> {
        let mut w = BufferWriter::new(Endianness::Big);
        a.encode(&mut w).map_err(|_| MARSHAL)?;
        b.encode(&mut w).map_err(|_| MARSHAL)?;
        let id = ch.send_poll("add", &w.into_bytes())?;
        Ok(MathAddPoller { request_id: id })
    }

    fn sendp_divmod(
        &self,
        ch: &mut dyn AsyncCorbaChannel,
        a: i32,
        b: i32,
    ) -> Result<MathDivmodPoller, CorbaException> {
        let mut w = BufferWriter::new(Endianness::Big);
        a.encode(&mut w).map_err(|_| MARSHAL)?;
        b.encode(&mut w).map_err(|_| MARSHAL)?;
        let id = ch.send_poll("divmod", &w.into_bytes())?;
        Ok(MathDivmodPoller { request_id: id })
    }
}

/// `add(a,b)->a+b` and `divmod(a,b)->(a/b, a%b)`.
fn with_math<F: FnOnce(MathStub, AmiClient)>(f: F) {
    let server = CorbaServer::new();
    server.register(b"Math", |op, body, e| {
        let mut r = BufferReader::new(body, e);
        let a = r.read_u32().unwrap() as i32;
        let b = r.read_u32().unwrap() as i32;
        let mut w = BufferWriter::new(e);
        match op {
            "add" => {
                w.write_u32(a.wrapping_add(b) as u32).unwrap();
                SkeletonResult::Reply(w.into_bytes())
            }
            "divmod" => {
                w.write_u32((a / b) as u32).unwrap();
                w.write_u32((a % b) as u32).unwrap();
                SkeletonResult::Reply(w.into_bytes())
            }
            _ => SkeletonResult::BadOperation,
        }
    });
    let acceptor = server.serve("127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = acceptor.listen_addr();
    let ior = object_reference("IDL:Math:1.0", &addr.ip().to_string(), addr.port(), b"Math");
    let stub = MathStub {
        object_ref: ior.clone(),
    };
    let client = AmiClient::connect(&ior).unwrap();
    f(stub, client);
    acceptor.shutdown();
}

/// Callback model, generated pattern: typed handler receives decoded values
/// (return-only and return-plus-multiple-outs).
#[test]
fn generated_callback_typed_dispatch() {
    struct H {
        add: Mutex<Option<i32>>,
        dm: Mutex<Option<(i32, i32)>>,
    }
    impl MathAmiHandler for H {
        fn add(&self, v: i32) {
            *self.add.lock().unwrap() = Some(v);
        }
        fn add_excep(&self, _e: CorbaException) {
            panic!("unexpected add fault");
        }
        fn divmod(&self, q: i32, r: i32) {
            *self.dm.lock().unwrap() = Some((q, r));
        }
        fn divmod_excep(&self, _e: CorbaException) {
            panic!("unexpected divmod fault");
        }
    }
    with_math(|stub, mut client| {
        let h = Arc::new(H {
            add: Mutex::new(None),
            dm: Mutex::new(None),
        });
        stub.sendc_add(&mut client, h.clone(), 17, 25).unwrap();
        stub.sendc_divmod(&mut client, h.clone(), 17, 5).unwrap();
        client.perform_all().unwrap();
        assert_eq!(*h.add.lock().unwrap(), Some(42));
        assert_eq!(*h.dm.lock().unwrap(), Some((3, 2)));
    });
}

/// Polling model, generated pattern: typed poller returns the return value or
/// an (out, out) tuple.
#[test]
fn generated_polling_typed_get_reply() {
    with_math(|stub, mut client| {
        let p_add = stub.sendp_add(&mut client, 100, 11).unwrap();
        let p_dm = stub.sendp_divmod(&mut client, 23, 4).unwrap();
        assert_eq!(p_dm.get_reply(&mut client).unwrap(), (5, 3));
        assert_eq!(p_add.get_reply(&mut client).unwrap(), 111);
    });
}
