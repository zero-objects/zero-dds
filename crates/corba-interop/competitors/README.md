# CORBA Test & Interop Suite

Complete suite for the entire CORBA crate family.

## Running

```sh
# Everything (unit tests of all crates + codegen E2E + cross-ORB interop):
bash crates/corba-interop/competitors/run_suite.sh        # auf codepit (ORBs installiert)

# Cross-ORB interop only (ZeroDDS ↔ omniORB/TAO/JacORB):
bash crates/corba-interop/competitors/run_interop.sh

# Codegen E2E only (stub→IIOP→skeleton, without foreign ORBs):
cargo test -p zerodds-corba-interop --test codegen_roundtrip
```

## What is covered

**1. Unit / integration tests** — all 26 CORBA crates (~3260 tests):
idl, idl-rust/-cpp/-java/-csharp/-ts/-python, cdr, corba-giop/-iiop/-ior/-poa/-rust,
corba-interop, corba-cosnaming/-csiv2/-ir/-cos-event/-dds-bridge/-dnc,
corba-ccm/-ccm-ejb/-ccm-lib/-codegen, ccm, ami4ccm.

> `idl-cpp` / `idl-java` were previously blocked via `zerodds-rpc → zerodds-dcps`
> (default build); since the dcps gate fix
> (`prepare_endpoint_crypto_tokens` back to `#[cfg(feature="security")]`) they
> build and test fully again.

**2. Codegen E2E** (`codegen_roundtrip.rs`) — generated stub → real
IIOP/GIOP loopback → generated skeleton, full feature matrix:
primitives (incl. char=1B/wchar=2B/double/long long), string, **wstring (UTF-16)**,
**any (TypeCode+Value)**, sequence, struct, enum, union, out/inout/oneway,
attribute, **object references (IOR)**, **typed exceptions (raises)**, plus
a **CosNaming NameService wired live over GIOP** (bind/resolve with a live
return reference).

**3. Cross-ORB interop** (`run_interop.sh`) — ZeroDDS ↔ **omniORB 4.3.3 /
TAO 2.5.24 / JacORB 3.9**, both directions, Echo + Bench matrix (long/double/
long long/char/string/**wstring**/sequence/out/inout/object-ref/typed-exception).
6/6 green. **Plus CosNaming** (real `CosNaming::NamingContext` wire): foreign
client→ZeroDDS naming server AND ZeroDDS client→foreign daemon (omniNames /
JacORB NameServer / TAO `tao_cosnaming`) — both directions × 3 ORBs = 6/6.
Setup: TAO via `/opt/opendds-secure` (ACE_ROOT/TAO_ROOT), JacORB via
`/opt/jacorb` + JDK8. The TAO naming daemon needs orbsvcs (not included in the
OpenDDS bundle) — build it once via `build_tao_naming_service.sh` from the
ACE+TAO-6.5.24 source; without it only the TAO daemon direction is SKIPped.

## Cross-ORB coverage per feature

| Feature | omniORB | TAO | JacORB |
|---|---|---|---|
| Echo (string, 4 kB) | ✅ | ✅ | ✅ |
| Primitives (long/double/long long) | ✅ | ✅ | ✅ |
| char (1 byte) | ✅ | ✅ | ✅ |
| sequence<long> | ✅ | ✅ | ✅ |
| out / inout | ✅ | ✅ | ✅ |
| object reference (IOR) | ✅ | ✅ | ✅ |
| typed exception (raises) | ✅ | ✅ | ✅ |
| wstring (UTF-16, codeset negotiation) | ✅ | ✅ | ✅ |
| CosNaming: foreign client → ZeroDDS server | ✅ | ✅ | ✅ |
| CosNaming: ZeroDDS client → foreign daemon | ✅ omniNames | ✅ tao_cosnaming | ✅ jacorb-ns |

CosNaming uses the standardized `CosNaming::NamingContext` wire
(`Name = sequence<NameComponent{id,kind}>`, RepositoryId
`IDL:omg.org/CosNaming/NamingContext:1.0`); foreign clients generate their stubs
from `cosnaming.idl` (`#pragma prefix "omg.org"`) via omniidl/tao_idl or use
`org.omg.CosNaming` (JacORB). The happy path (bind/resolve/rebind/unbind) is 6/6;
the cross-ORB `NotFound` exception assertion is pending the correct exception
RepositoryId (codegen hardening, see below).

wstring is cross-ORB proven since the codeset-negotiation WP: UTF-16 BOM
(§15.3.1.6) on the wire + TAG_CODE_SETS component in the IOR (§13.10.2.4) — the
latter is the gate for omniORB (otherwise it throws INV_OBJREF when sending
wstring); TAO/JacORB are lax here. ZeroDDS-only (e2e over IIOP): any (TypeCode
encoding dependent).
