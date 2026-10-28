# `zerodds-omg-security-main`

`ShapeType_main` reference client for the OMG **`dds-security`**
interoperability test suite (`github.com/omg-dds/dds-security`, pinned at
`62f5ec42`). **BUILD + LOCAL-VALIDATE phase artifact.** Nothing from this
crate has been submitted upstream — no CLA PR, no source PR, no release
asset, no push to `github.com/omg-dds`. See "Phase-2 (not done here)".

This is the second of the three OMG interop suites, not a second client
for the first one. `dds-rtps` (`tools/omg-shape-main`) measures QoS and
wire behaviour between plaintext participants; this suite measures mutual
authentication, domain and topic access control, key exchange, and
encrypted or signed messages. They share a type *name* and nothing else:
see `interop/shapes/README.md`.

The public product/version name for an eventual upstream submission is
**undecided** (Sandra's call). Local builds use the crate/binary name
`shapetype_main` (the suite's own source is `ShapeType_main.cxx`; the `dds-rtps` client keeps `shape_main`, and two binaries of one name cannot share a Cargo target directory).

This is a CLI + protocol shim over ZeroDDS's **existing** DCPS API
(`crates/dcps`), QoS machinery (`crates/qos`) and DDS-Security stack
(`crates/security*`). No new wire, CDR, QoS or crypto code was written
for it.

## The type is generated, from the first commit

`build.rs` generates `ShapeType` from `interop/shapes/omg-dds-security.idl`
with `zerodds_build::Config::…dds_type_impls(true)`. There is no
hand-written `dds-security` `ShapeType` anywhere in the tree.

`tools/omg-shape-main` learned why the expensive way and wrote it down:
its type first came from `zerodds_dcps::interop`, written by hand, which
is the wrong side of the measurement — a defect in the generator cannot
fail the suite, and a defect in the hand form is charged to us. Both were
real there (a `type_object()` left at `None`, an `encode_be` that was
never overridden). `src/shape_type.rs::tests` carries that pair of
regression guards for this type, plus the key test below.

## The `//@key` finding

The suite's IDL writes its key as a trailing comment:

```idl
string<COLOR_MAX_LENGTH> color; //@key
```

That form is normative, not a vendor dialect — XTypes 1.3 Table 24: an
annotation may be applied "alternately and equivalently … by suffixing it
with an annotation type name using slash-slash-at".

**Measured on the tree this crate was written against: our front end does
not read it.** The line passes `parse` and `lower` without a single
diagnostic and `color` comes out with `facets.is_key() == false` — a
silent drop at the point where instance keying is decided. The generated
type has `HAS_KEY == false` and `key_hash()` returns `None`.

What that costs on the wire: every sample lands in one instance. No
`PID_KEY_HASH` goes out, `DataReader::take_next_instance` has nothing to
cycle, and an `-color RED` and an `-color BLUE` writer feed the same
instance rather than two. For this suite's own cases that is survivable —
its clients read every sample and print it — but it is not the suite's
type.

**The IDL is not rewritten to `@key` to paper over this.** An IDL that
differs from the suite's no longer measures the suite. Front-end work to
read the comment form is under way; `src/shape_type.rs`'s key test is
written so that the fix makes it *stronger*, not red: it asserts that
`HAS_KEY` and `key_hash()` are one decision and that, where the type is
keyed, `color` decides the hash and `x`/`y`/`shapesize` do not.

## Flag coverage table

Transcribed from the suite's `srcCxx/ShapeType_main.cxx` — its `main()`
argument loop and its `print_usage()` — cross-read against `README.md`'s
two worked invocations and against
`srcCxx/opendds/ShapeType_configurator_opendds.h`, the one committed
configurator that actually configures security instead of ignoring its
arguments.

Status is per row and honest: `full` means wired and exercised by a test
or a local run, `partial` names what is missing, `stub` means accepted
and not acted on. Nothing in this table is `stub`; several rows are
`partial` and say why.

| Flag | Wired to | Status |
|---|---|---|
| `-h` / `-help` | clap | full |
| `-v [e|d]` | the `debugln!` / `errorln!` macros in `src/main.rs` | full — both levels print to **stdout**, as the suite's `Logger` does (`log_message` → `std::cout`). This differs deliberately from `tools/omg-shape-main`, which puts diagnostics on stderr because a `pexpect` harness matches its stdout; this suite has no such harness |
| `-domain <int>` | `DomainParticipantFactory::create(domain_id)` | full |
| `-pub <topic>` | `Topic<ShapeType>` + `DataWriter` | full |
| `-sub <topic>` | `Topic<ShapeType>` + `DataReader` | full — and, as in the suite, one topic object when `-pub` and `-sub` name the same one |
| neither `-pub` nor `-sub` | both, on `OD_OA_OM_OD` | full — `cli::tests::neither_pub_nor_sub_defaults_to_both_on_the_open_topic` |
| `-color <name>` | the sample's `color` member, and `y = (4 * (color[0] - 'A')) % 250` | partial — the member is written and the suite's own arithmetic is reproduced (`main::tests::the_sample_is_the_suites_own_arithmetic`), but the generated type is keyless, so the colour is **not** an instance key. See "The `//@key` finding" |
| `-governance <file>` | `dds.sec.access.governance` on the participant QoS → `crates/dcps` → `SecurityProfileConfig::governance_p7s` | partial — the client only sets the property; the runtime reads it (`security::tests::the_profile_config_follows_the_property_list`, `crates/dcps/tests/security_qos_properties.rs`). **Never run against the suite's own file**, because the suite's PKI does not load. See "What the security half needs" |
| `-permissions <file>` | `dds.sec.access.permissions` on the participant QoS | partial — same wiring, same blocker |
| `-kest <algo>` | `dds.sec.auth.key_establishment_algorithm` on the participant QoS → `PkiAuthenticationPlugin::set_preferred_kx_suite` in `crates/dcps` | partial — `ECDHE-CEUM+P256` (the suite default), `ECDHE-CEUM+P384` and the `X25519` vendor extension map to a `KxSuite`; `DH+MODP-2048-256` is **refused loudly** rather than silently becoming P-256. What goes out as `c.kagree_algo` stays `ECDH+prime256v1-CEUM`, the string CycloneDDS and Fast DDS were captured sending — the flag selects the suite, not the literal |
| `-partition <str>` | `PartitionQosPolicy` on the Publisher **and** the Subscriber | full — one name, as the suite's `push_back` of a single string does |
| `-livelinessPeriod <float>` | `LivelinessQosPolicy.lease_duration` on writer and reader | partial — the QoS is set (`main::tests::a_zero_liveliness_period_leaves_the_default_lease`); live LIVELINESS enforcement was not measured for this client |
| `-disableSecurity` | empties the `PropertyQosPolicy`; plaintext participant | full — `security::tests::disable_security_leaves_no_dds_sec_property_behind`, and the plaintext publisher run below |
| `-logging` | a logger-only `SecurityBundle` handed to `ParticipantBuilder::with_security`, printing to stdout; the runtime wires it into the PKI plugin it builds from the properties | partial — DDS-Security §8.6 events reach stdout, but each vendor's `-logging` prints its own plugin's strings (the suite README shows RTI's `RTI_Security_AccessControl_*` lines), so the transcripts are readable side by side and not diffable |
| `-identityCa <file>` | `dds.sec.auth.identity_ca` | full — **not a suite flag**, see below |
| `-identityCert <file>` | `dds.sec.auth.identity_certificate` | full — **not a suite flag** |
| `-identityKey <file>` | `dds.sec.auth.private_key` | full — **not a suite flag** |
| `-permissionsCa <file>` | `dds.sec.access.permissions_ca` | full — **not a suite flag** |
| `-acceptX509v1IdentityCerts` | `org.zerodds.sec.auth.accept_x509v1_identity_certificate` on the participant QoS → `CertPolicy::accept_x509v1_identity_certificate` in `crates/dcps` | full — **not a suite flag**, and it switches on a deliberate deviation from **DDS-Security 1.2 §10.3.1.3**, which requires an X.509 **v3** identity certificate. It exists because the suite's own identity certificates are v1 (finding 1 below) and a strict client cannot join at all. The deviation relaxes the version check and nothing else — signature, validity period, chain to the identity CA and subject are still checked, and every accepted v1 certificate produces a `Warning` security-log event naming its subject and the clause. Off by default; `zerodds_security_pki::x509v1_compat::clause_10_3_1_3` is the clause test set |

### Why five flags the suite does not have

The suite's clients hardcode their vendor's certificate paths in the
configurator header —
`srcCxx/opendds/ShapeType_configurator_opendds.h` has
`file:../oci_opendds_dds_certs/TESTONLY_oci_opendds_dds_identity_cert.pem`
written into it — and offer environment overrides for two of them
(`OPENDDS_ID_CERTIFICATE`, `OPENDDS_ID_PRIVATE_KEY`).

**This repository carries no vendor's certificates and is not going to.**
So all four paths are parameters here, each with an environment fallback
in the same spirit: `ZERODDS_ID_CA`, `ZERODDS_ID_CERTIFICATE`,
`ZERODDS_ID_PRIVATE_KEY`, `ZERODDS_PERMISSIONS_CA`. The flag wins over
the variable.

The fifth, `-acceptX509v1IdentityCerts`, is a different kind of flag: it
does not name a file, it leaves a clause. The suite's own certificates
violate §10.3.1.3 and every one of its six vendors accepts them anyway,
so a client that holds the line cannot be measured against the suite at
all. The flag says so in its own name — the property it sets is spelled
`…zerodds.spec_deviation.accept_x509v1_identity_certificate` — and the
run prints one `Info:` line when it is on.

### Single-dash long options

The suite's client is a hand-rolled `strcmp` loop, so its options are
single-dash long words. `cli::normalize_args` rewrites exactly the
suite's spellings to clap's `--` form before parsing, so an invocation
copied verbatim out of the suite README works unchanged
(`cli::tests::the_suite_readme_invocation_parses_as_written`). `-v` and
`-h` are genuine short options and are left alone.

## Exact stdout strings

There is **no `pexpect` harness in this suite.** `dds-rtps` has
`interoperability_report.py`; this repository has a README with two
worked transcripts and a `compatibility_matrix.xlsx`, and validation is
by eye. The strings below are therefore transcribed rather than matched —
so that a ZeroDDS transcript can be read beside a vendor's, not because a
regex depends on them.

From `srcCxx/ShapeType_main.cxx`, quoted verbatim:

- `"Publishing:  '<topic>'"` — two spaces after the colon, `main()`.
- `"Subscribing: '<topic>'"` — one space.
- `"Create topic: <topic>"` — `create_topic()`.
- `"Create writer for topic: <topic> color: <color>"` — **with** a
  colon after `topic`. (The `dds-rtps` suite prints the same line
  *without* one. The two suites differ here; do not unify them.)
- `"Create reader for topic: <topic>"`.
- Sample line: `printf("%-10s %-10s %03d %03d [%d]\n", sub_topic, color,
  x, y, shapesize)` — the subscribed topic name, not the reader's.
- `-v d` only: `"Running create_participant() function"`,
  `"Participant created"`, `"Running create_writer() function"`,
  `"Running create_reader() function"`,
  `"\nLoop: wait count = N, sent count = N, received count = N\n"`,
  `"Time: <sec>:<nanosec> -- Sending sample: count = N"`.

On **stderr**, from `main()`: the `Info: "-<flag>" unspecified. Default
to …` lines, and `"Done..."` on exit.

The suite's own README shows an older transcript (`Received sample…`
followed by an indented field dump). The committed source prints the
one-line `%-10s %-10s %03d %03d [%d]` form; the source is what is
reproduced.

## Local validation result

**Plaintext side: runs.** `-disableSecurity -pub OD_OA_OM_OD -color
BLACK -v d` on macOS produces the suite's transcript shape:

```
Info: "-domain" unspecified. Default to: 0
Publishing:  'OD_OA_OM_OD'
Running create_participant() function
Participant created
Create topic: OD_OA_OM_OD
Running create_writer() function
Create writer for topic: OD_OA_OM_OD color: BLACK

Loop: wait count = 0, sent count = 0, received count = 0

Time: 1788232696:750440000 -- Sending sample: count = 0
```

**Secured side: does not run against the suite's own material, and the
reason is not this client.** Measured, with the suite's
`TESTONLY_governance_signed.p7s`, the OpenDDS vendor certificate set and
the two CAs fetched read-only into a scratch directory:

```
failed: security-profile pki: security error [AuthenticationFailed]:
        pki: certificate invalid: parse: UnsupportedCertVersion
```

**No pass count is claimed for this suite. It has not been run.** Two
participants have not been paired, no vendor binary has been downloaded,
and the `compatibility_matrix.xlsx` has not been reproduced.

## What the security half needs that this tree does not have

Five findings, in the order they block.

1. **X.509 **v1** end-entity certificates — answered, by a flag.** Every
   one of the suite's six vendor identity certificates is X.509 version 1
   — measured with `openssl x509 -text` on `rti_connext_dds`,
   `eprosima_fastrtps`, `oci_opendds_dds`, `toc_coredx_dds`,
   `kda_intercom_dds` and `ad_vortex_dds`; so is
   `TESTONLY_permissions_ca_cert.pem`. Only
   `TESTONLY_identity_ca_cert.pem` is v3. `crates/security-pki`'s
   `identity.rs` validates through `webpki::EndEntityCert::try_from`, and
   `rustls-webpki` accepts only v3 end-entity certificates (`cert.rs`,
   `Error::UnsupportedCertVersion`). It *does* accept a v1 **trust
   anchor** — `anchor_from_trusted_cert` has an explicit v1 path — so the
   CAs are not the problem and the identity certificates are.

   DDS-Security 1.2 §10.3.1.3 is on webpki's side: *"An X.509 **v3**
   Certificate [39] that chains up to the Identity CA."* The suite's
   material contradicts its own specification, and all six vendors accept
   each other's v1 identities, so the laxity is theirs and the refusal is
   ours to keep.

   It is kept — as the default. `crates/security-pki`'s
   `x509v1_compat` module adds a second validation path that accepts a v1
   identity certificate **only** when
   `org.zerodds.sec.auth.accept_x509v1_identity_certificate`
   is set, which `-acceptX509v1IdentityCerts` does. That path relaxes the
   version check and nothing else: it verifies the certificate signature
   against the anchor's key, the validity window, the chain (the
   certificate's `issuer` must be the DER-identical `subject` of a
   configured identity CA, and that anchor's key must be the one that
   signed it) and the expected subject, and it writes a `Warning`
   security-log event for every certificate it accepts. What it cannot
   check is key usage — a v1 certificate has no extensions field at all —
   and that is a property of the format, which is why the deviation is
   off unless somebody asks for it. The one check that path does not
   implement is the anchor's `nameConstraints` (RFC 5280 §4.2.1.10),
   which the v3 path gets from `webpki::verify_for_usage`; an identity CA
   carrying that extension refuses the v1 path rather than having its
   constraint dropped. The suite's `TESTONLY_identity_ca_cert.pem`
   carries none.
2. **`crates/dcps` did not read `dds.sec.*` off the participant QoS —
   closed.** `RuntimeConfig::with_security_log_properties` read the
   `dds.sec.log.*` subset and there was no counterpart for the eight that
   configure authentication and access control: a participant created
   with a complete, correct property list came up with no
   `SecurityBundle`, ran plaintext RTPS and said nothing about it — the
   shape of silent drop that costs the most to find. This client used to
   close the distance for itself by reading the list and building the
   bundle by hand, which measured the workaround rather than the product.
   `crates/security-runtime/src/qos_properties.rs` now holds the reader
   and `crates/dcps/src/participant.rs::security_from_participant_properties`
   calls it on every participant. This client only **sets** the
   properties. An incomplete or unreadable list fails
   `create_participant` with `DdsError::SecurityConfiguration` naming the
   property; it never falls back to plaintext.
   `crates/dcps/tests/security_qos_properties.rs` is the clause set.
3. **`zerodds_security_runtime::strip_file_url` does not strip the form
   this suite uses — covered by `resolve_file_uri`.** `strip_file_url`
   handles `file://host/path` and `file:///abs/path`. The suite — and
   DDS-Security 1.2 §10.4.1 Table 63's own examples — write
   `file:../TESTONLY_governance_signed.p7s`, the scheme with no
   authority, and that string comes back unchanged, so the open fails on
   a path that literally begins `file:`.
   `zerodds_security_runtime::resolve_file_uri` handles all three and is
   what the participant path calls;
   `security::tests::the_suites_authority_less_file_uri_resolves_to_a_path`
   and `crates/dcps/tests/security_qos_properties.rs::every_file_uri_spelling_is_resolved_on_the_participant_path`
   pin both halves.
4. **The `dds.sec.*` property names had no single home — closed.**
   `dds.sec.access.governance`, the property that decides whether a
   domain has access control at all, was named nowhere;
   `SecurityProfile` took it as a `PathBuf` field and this client
   declared the string locally. All eight of §9.3.1 Table 44 and
   §9.4.1.2 Table 46 are now constants in
   `crates/security-runtime/src/qos_properties.rs`, re-exported here
   rather than re-spelled.
5. **The `//@key` comment form.** Above, under its own heading.

Beside those: the `dds.sec.*` properties in `crates/security*` carry
**inline PEM and XML content** as their values, while DDS-Security 1.2
§10.3.1 Table 50 and this suite both carry **URIs**. The participant path
resolves the URI and hands `SecurityProfile::from_files` a path, so a
spec-shaped list reaches the plugins as paths — but a caller that feeds
`PkiAuthenticationPlugin` a property list directly still gets a PEM parse
error on the string `file:/etc/…` rather than a message about URIs.

## Where the certificates are

**Not in this repository, by decision.** The suite's material lives in
`github.com/omg-dds/dds-security` at the repository root and in six
per-vendor directories:

| what | path in the suite |
|---|---|
| governance, signed | `TESTONLY_governance_signed.p7s` |
| governance, plain XML | `TESTONLY_governance.xml` |
| identity CA | `TESTONLY_identity_ca_cert.pem` |
| permissions CA | `TESTONLY_permissions_ca_cert.pem` |
| vendor identity certs | `<vendor>_certs/TESTONLY_<vendor>_identity_cert.pem` |
| vendor private keys | `<vendor>_certs/private/TESTONLY_<vendor>_identity_private_key.pem` |
| vendor permissions | `<vendor>_certs/TESTONLY_<vendor>_permissions_{join,read,write,readwrite,allowall}_signed.p7s` |

with `<vendor>` one of `rti_connext_dds`, `eprosima_fastrtps`,
`oci_opendds_dds`, `toc_coredx_dds`, `kda_intercom_dds`,
`ad_vortex_dds`. Point the four `-identity*` / `-permissionsCa` flags at
a checkout, or set the `ZERODDS_*` variables.

## Reading a topic name

`TESTONLY_governance.xml` names its own convention in a comment, and
every topic in this suite is spelled that way:

```
OD / PD             : Open / Protected Discovery
OA / RA / WA / RWA  : Open / Read / Write / ReadWrite Access
OM / SM / EM        : Open / Signed / Encrypted  Meta-data
OD / SD / ED        : Open / Signed / Encrypted  Data
```

So `OD_OA_OM_OD` is the topic with every protection off — the default —
and `PD_RWA_EM_ED` is protected discovery, read/write access control,
encrypted metadata and encrypted data. A `SET_`-prefixed rule with a
trailing `*` is the same thing as a topic expression.

## Phase-2 (not done here — Sandra's call)

- Company/product name for the OMG submission.
- `CLA/CLA_<CompanyName>.md` from the suite's `CLA/CLA_TEMPLATE.md`, and
  the CLA PR (per its `CONTRIBUTING.md`).
- The source PR adding a vendor directory beside `srcCxx/opendds`,
  `srcCxx/rti_connext_dds` and the rest.
- A real pairwise run: another vendor's binary against this one, and the
  `compatibility_matrix.xlsx` row that comes out of it. Blocked on
  finding 1 above, and on the binary download being Sandra's call.
