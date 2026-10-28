# Columns whose toolchain is not on the measurement host

`PROVENANCE.tsv` records what each toolchain reports **on the host that
ran it**. Where a column exists in the ledger and its toolchain does not
exist here, that is recorded rather than skipped, and the reason belongs
in this file.

A column in this list is one nobody can re-derive on the current host.
Its rows are still valid — they were measured when the toolchain was
installed — but a re-measurement needs the toolchain back first. That is
a property of the comparison and not an accident of one run.
`opensplice` is the one exception to "not on the host": its toolchain
never left, only the provenance script's coverage of it does not exist
yet, which is a smaller gap and named as one below.

`tools/interop-results/tests/provenance_matches_systems.rs` reads this
file: a column absent from the host and absent from this list is a
failure. An entry here has to name what is missing and what it would
take to restore it, so that "known absence" never quietly becomes
"forgotten".

## The entries

| column | claimed version | why it is not on the host | to restore |
|---|---|---|---|
| `tao` | TAO 2.5.24 | `TAO_IDL` is unset; the ACE+TAO build is not installed under `/opt`. OpenDDS ships its own `tao_idl` for its own use — `/opt/opendds/bin/tao_idl` — but the TAO column was recorded against a standalone TAO, and pointing at OpenDDS's copy would silently attribute one product's ORB to another. | install ACE+TAO 2.5.24 and set `TAO_IDL`; 207 ledger rows |
| `ros-fastrtps` | rosidl_typesupport_fastrtps_c 2.2.2 (ROS 2 Humble) | `ROS_DISTRO` is unset. The Humble underlay is not sourced in the recording environment, and `tools/interop-env.sh` does not source it. | source the Humble underlay before `record.sh --ros`; 244 ledger rows |
| `ros-cyclone` | rmw_cyclonedds_cpp 1.3.4 (ROS 2 Humble) | as above — the same underlay carries both ROS columns. | as above; 125 ledger rows |
| `opensplice` | OpenSplice Community Edition 6.9 (OSPL_V6_9_210323OSS_RELEASE) | Not absent from the host — its 191 ledger rows came from `record.sh --vendors opensplice` against `OSPL_HOME`, so the toolchain is there. `PROVENANCE.tsv` predates the column: it was recorded 2026-08-16, before OpenSplice was built on this host, and `toolchain-provenance.sh` has no probe block for it yet, so a re-run would still leave the row unprobed. | add an `opensplice` block to `toolchain-provenance.sh` (`$OSPL_HOME/bin/idlpp` plus a release string — `release.com` in the HDE tree is the candidate, unverified from here) and re-run it on the host; 191 ledger rows |

767 of the ledger's rows-with-bytes come from these four columns.

## What this does not excuse

An entry here is a debt, not a dispensation. Every row those three
columns carry was measured against a toolchain nobody on this host can
question, and the tables print them beside columns that can be
re-derived at any time. The distinction is invisible in
`SYSTEMS.md` today; making it visible there is the obvious next step and
is not done.
