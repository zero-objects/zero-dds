// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-omg-shape-main` with `zerodds-rtps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this tool uses of `zerodds-rtps` (`src/main.rs`, `vendor_forms`):
//! `dialect::GroupWireForm::CoherentSetsOnly`, `dialect::EndOfSetForm::Gap`
//! and `dialect::WriterGroupInfoWidth::Sixteen` set with
//! `RtpsBehaviour::with_group_wire_form`/`with_end_of_set_form`/
//! `with_writer_group_info_width` on a copy of the runtime's writer
//! behaviour, per peer `wire_types::VendorId` (`RTI_CONNEXT`, `HDDS`,
//! `ZZDDS`, `INTERCOM`, `DUST`). The contract: the setters change their one
//! axis and nothing else, the defaults are the forms of the specification
//! (DDSI-RTPS 2.5 §8.7.5/§8.7.6, Table 9.22 Example 3, §9.3.2.5), and the
//! vendor ids are those the peers put in their RTPS header (the OMG list for
//! RTI 0x0101 and InterCOM 0x0105; HDDS 0x011A, zzdds 0x011B and Dust 0x0114
//! as their participants announce them).

use zerodds_rtps::dialect::{EndOfSetForm, GroupWireForm, RtpsBehaviour, WriterGroupInfoWidth};
use zerodds_rtps::wire_types::VendorId;

#[test]
fn the_default_forms_are_those_of_the_specification() {
    let behaviour = RtpsBehaviour::default();
    assert_eq!(behaviour.group_wire_form, GroupWireForm::Specification);
    assert_eq!(behaviour.end_of_set_form, EndOfSetForm::Data);
    assert_eq!(
        behaviour.writer_group_info_width,
        WriterGroupInfoWidth::Specification
    );
    assert_eq!(RtpsBehaviour::new(), behaviour);
}

#[test]
fn each_setter_changes_its_axis_only() {
    let base = RtpsBehaviour::default();
    let group = base.with_group_wire_form(GroupWireForm::CoherentSetsOnly);
    assert_eq!(group.group_wire_form, GroupWireForm::CoherentSetsOnly);
    assert_eq!(
        group.with_group_wire_form(GroupWireForm::Specification),
        base
    );
    let end = base.with_end_of_set_form(EndOfSetForm::Gap);
    assert_eq!(end.end_of_set_form, EndOfSetForm::Gap);
    assert_eq!(end.group_wire_form, base.group_wire_form);
    assert_eq!(end.with_end_of_set_form(EndOfSetForm::Data), base);
    let wide = base.with_writer_group_info_width(WriterGroupInfoWidth::Sixteen);
    assert_eq!(wide.writer_group_info_width, WriterGroupInfoWidth::Sixteen);
    assert_eq!(wide.group_wire_form, base.group_wire_form);
    assert_eq!(
        wide.with_writer_group_info_width(WriterGroupInfoWidth::Specification),
        base
    );
}

#[test]
fn the_peer_vendor_ids() {
    assert_eq!(VendorId::RTI_CONNEXT.to_bytes(), [0x01, 0x01]);
    assert_eq!(VendorId::INTERCOM.to_bytes(), [0x01, 0x05]);
    assert_eq!(VendorId::HDDS.to_bytes(), [0x01, 0x1A]);
    assert_eq!(VendorId::ZZDDS.to_bytes(), [0x01, 0x1B]);
    assert_eq!(VendorId::DUST.to_bytes(), [0x01, 0x14]);
}
