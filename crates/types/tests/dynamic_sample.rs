//! DDS-XTypes 1.3 §7.5.2.10 and §7.5.2.11: `DynamicDataFactory` and
//! `DynamicData` with its type, the typed getters and setters of Annex C
//! and the promotions of §7.5.2.11.9.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used, missing_docs)]

use std::sync::Arc;

use zerodds_ast::Ast;
use zerodds_types::dynamic::{
    DynamicDataFactory, DynamicError, DynamicSample, DynamicType, MEMBER_ID_INVALID, TypeKind,
};

const IDL: &str = r#"
    enum Color { RED, GREEN, BLUE };
    @bit_bound(8) bitmask Flags { READ, WRITE, @position(5) EXEC };
    struct Inner { long v; };
    struct Demo {
        @key long serial;
        short small;
        double ratio;
        string<8> label;
        @optional long maybe;
        Color shade;
        Flags permissions;
        Inner embedded;
        sequence<long, 4> numbers;
        long grid[2][3];
        map<string, long> table;
        sequence<string> names;
    };
    union U switch (long) {
        case 1: long a;
        case 2: case 3: string b;
        default: double c;
    };
"#;

fn model() -> Arc<Ast> {
    let parsed = zerodds_idl_parse::parse(IDL);
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "sample.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let mut ast = lowered.ast;
    let report = ast.resolve_all_by_name().expect("the resolve pass");
    assert!(report.is_clean(), "{:?}", report.broken);
    Arc::new(ast)
}

fn demo() -> (DynamicType, DynamicSample) {
    let ast = model();
    let ty = DynamicType::find(&ast, "Demo").unwrap();
    let sample = DynamicDataFactory::get_instance().create_data(&ty).unwrap();
    (ty, sample)
}

fn id(sample: &mut DynamicSample, name: &str) -> u32 {
    sample.get_member_id_by_name(name).unwrap()
}

/// §7.5.2.10.1 and §7.5.2.11.8: a sample has its type, every non-optional
/// member at its default and the optional one absent; §7.5.2.11.6 counts
/// the members present.
#[test]
fn a_new_sample_has_its_type_and_defaults() {
    let (ty, mut sample) = demo();
    assert!(sample.type_().equals(&ty));
    assert_eq!(sample.get_item_count(), 11, "the optional member is absent");
    let serial = id(&mut sample, "serial");
    assert_eq!(sample.get_int32_value(serial).unwrap(), 0);
    assert_eq!(sample.get_member_id_at_index(0).unwrap(), serial);
    assert!(sample.get_member_id_at_index(11).is_err());
    assert!(sample.get_member_id_by_name("zzz").is_err());
    // A sample of a primitive: no contained values, MEMBER_ID_INVALID.
    let long = DynamicType::find(&model(), "Inner")
        .unwrap()
        .member_by_name("v")
        .unwrap()
        .dynamic_type()
        .unwrap();
    let mut atom = DynamicSample::new(&long).unwrap();
    assert_eq!(atom.get_item_count(), 1);
    assert_eq!(atom.get_member_id_by_name("").unwrap(), MEMBER_ID_INVALID);
    assert_eq!(atom.get_member_id_at_index(0).unwrap(), MEMBER_ID_INVALID);
    atom.set_int32_value(MEMBER_ID_INVALID, 5).unwrap();
    assert_eq!(atom.get_int32_value(MEMBER_ID_INVALID).unwrap(), 5);
    atom.clear_value(MEMBER_ID_INVALID).unwrap();
    assert_eq!(atom.get_int32_value(MEMBER_ID_INVALID).unwrap(), 0);
}

/// §7.5.2.11.9: "a smaller type promotes to a large type but not vice
/// versa" — a short can be read as a long and a long can be set as if it
/// were a short, and neither the other way.
#[test]
fn integers_promote_upwards_only() {
    let (_, mut sample) = demo();
    let small = id(&mut sample, "small");
    let serial = id(&mut sample, "serial");
    let ratio = id(&mut sample, "ratio");

    sample.set_int16_value(small, 7).unwrap();
    assert_eq!(
        sample.get_int32_value(small).unwrap(),
        7,
        "short read as long"
    );
    assert_eq!(sample.get_int64_value(small).unwrap(), 7);
    assert_eq!(sample.get_float64_value(small).unwrap(), 7.0);
    // A long is not a short.
    let error = sample.get_int16_value(serial).unwrap_err();
    assert!(
        matches!(error, DynamicError::BadParameter { .. }),
        "{error}"
    );
    assert!(
        sample.set_int32_value(small, 1).is_err(),
        "a long set into a short"
    );
    // A smaller value type is set into a larger member.
    sample.set_int8_value(small, -3).unwrap();
    assert_eq!(sample.get_int16_value(small).unwrap(), -3);
    sample.set_uint8_value(small, 200).unwrap();
    assert_eq!(sample.get_int16_value(small).unwrap(), 200);
    // UInt16 → Int16 is no promotion.
    assert!(sample.set_uint16_value(small, 1).is_err());
    assert!(sample.get_uint16_value(small).is_err());

    // A long is a double (Int32 → Float64), not a float.
    assert_eq!(sample.get_float64_value(serial).unwrap(), 0.0);
    assert!(sample.get_float32_value(serial).is_err());
    sample.set_float32_value(ratio, 1.5).unwrap();
    assert_eq!(sample.get_float64_value(ratio).unwrap(), 1.5);
    sample.set_int32_value(ratio, 4).unwrap();
    assert_eq!(sample.get_float64_value(ratio).unwrap(), 4.0);
    assert!(sample.get_float32_value(ratio).is_err());
    let wide = sample.get_float128_value(ratio).unwrap();
    assert_eq!(wide.to_f64_rounded(), 4.0);
    // Boolean → any integer; nothing → Boolean.
    sample.set_boolean_value(small, true).unwrap();
    assert_eq!(sample.get_int16_value(small).unwrap(), 1);
    assert!(sample.get_boolean_value(small).is_err());
    // Byte promotes to any primitive.
    sample.set_byte_value(small, 9).unwrap();
    assert_eq!(sample.get_int16_value(small).unwrap(), 9);
    assert!(sample.get_byte_value(small).is_err());
}

/// Annex C `get_string_value` and the bound of the string.
#[test]
fn strings_keep_their_bound_and_their_width() {
    let (_, mut sample) = demo();
    let label = id(&mut sample, "label");
    sample.set_string_value(label, "abc").unwrap();
    assert_eq!(sample.get_string_value(label).unwrap(), "abc");
    assert!(sample.set_string_value(label, "123456789").is_err());
    assert!(
        sample.get_wstring_value(label).is_err(),
        "a string is no wstring"
    );
    // §7.5.2.11.1: the elements of a string are reached by index.
    let mut text = sample.get_complex_value(label).unwrap();
    assert_eq!(text.get_item_count(), 3);
    assert_eq!(text.get_char8_value(1).unwrap(), 'b');
    text.set_char8_value(3, 'd').unwrap();
    assert_eq!(text.get_member_id_at_index(4).unwrap(), 4);
    text.clear_value(0).unwrap();
    sample.set_complex_value(label, &text).unwrap();
    assert_eq!(sample.get_string_value(label).unwrap(), "bcd");
}

/// §7.5.2.11.3 and §7.5.2.11.5: an optional member has no value until set,
/// and is removed by clearing; a clone is equal, and a change makes it
/// unequal.
#[test]
fn an_optional_member_is_set_and_removed() {
    let (_, mut sample) = demo();
    let maybe = id(&mut sample, "maybe");
    let error = sample.get_int32_value(maybe).unwrap_err();
    assert_eq!(error.return_code(), "RETCODE_NO_DATA");
    sample.set_int32_value(maybe, 5).unwrap();
    assert_eq!(sample.get_int32_value(maybe).unwrap(), 5);
    assert_eq!(sample.get_item_count(), 12);

    let copy = sample.clone();
    assert!(copy.equals(&sample));
    sample.clear_value(maybe).unwrap();
    assert_eq!(sample.get_item_count(), 11);
    assert!(sample.get_int32_value(maybe).is_err());
    assert!(!copy.equals(&sample));
}

/// §7.5.2.11.9: an enumeration reads and sets as a signed integer wide
/// enough for its bit bound; §7.5.2.11.3: clearing gives the first value.
#[test]
fn an_enumeration_is_a_signed_integer() {
    let (_, mut sample) = demo();
    let color = id(&mut sample, "shade");
    assert_eq!(sample.get_int32_value(color).unwrap(), 0);
    sample.set_int32_value(color, 2).unwrap();
    assert_eq!(sample.get_int32_value(color).unwrap(), 2);
    assert!(
        sample.set_int32_value(color, 7).is_err(),
        "no such enumerator"
    );
    // The default bit bound is 32: a short is too narrow.
    assert!(sample.get_int16_value(color).is_err());
    assert!(sample.get_int64_value(color).is_ok());
    sample.clear_value(color).unwrap();
    assert_eq!(sample.get_int32_value(color).unwrap(), 0);
}

/// §7.5.2.11.1, .3, .6, §7.5.2.11.9: the flags of a bitmask are Boolean
/// values by name or bit; the item count is the number of flags set; the
/// bitmask reads as an unsigned integer.
#[test]
fn a_bitmask_has_boolean_flags_and_reads_as_an_unsigned_integer() {
    let (_, mut sample) = demo();
    let flags = id(&mut sample, "permissions");
    let mut mask = sample.get_complex_value(flags).unwrap();
    assert_eq!(mask.type_().kind(), TypeKind::Bitmask);
    assert_eq!(mask.get_item_count(), 0);
    let exec = mask.get_member_id_by_name("EXEC").unwrap();
    assert_eq!(exec, 5, "the id of a flag is its bit");
    let write = mask.get_member_id_by_name("WRITE").unwrap();
    assert_eq!(write, 1);
    assert!(!mask.get_boolean_value(exec).unwrap());
    mask.set_boolean_value(exec, true).unwrap();
    mask.set_boolean_value(0, true).unwrap();
    assert_eq!(mask.get_item_count(), 2);
    assert!(mask.get_boolean_value(exec).unwrap());
    // A bit no flag names cannot be held.
    assert!(mask.set_boolean_value(2, true).is_err());
    assert!(mask.get_member_id_at_index(8).is_err(), "outside the bound");
    assert_eq!(mask.get_member_id_at_index(2).unwrap(), 2);
    sample.set_complex_value(flags, &mask).unwrap();

    assert_eq!(sample.get_uint8_value(flags).unwrap(), 0b10_0001);
    assert_eq!(sample.get_uint16_value(flags).unwrap(), 33);
    assert_eq!(sample.get_int16_value(flags).unwrap(), 33, "UInt8 → Int16");
    assert!(sample.get_int8_value(flags).is_err());
    sample.set_uint8_value(flags, 0b10).unwrap();
    assert_eq!(sample.get_complex_value(flags).unwrap().get_item_count(), 1);
    assert!(
        sample.set_uint8_value(flags, 0x80).is_err(),
        "no flag at bit 7"
    );

    // §7.5.2.11.3: clearing a bit clears it.
    let mut mask = sample.get_complex_value(flags).unwrap();
    mask.clear_value(1).unwrap();
    assert_eq!(mask.get_item_count(), 0);
}

/// §7.5.2.11.7: a loan takes the value out as a sample of its own; at most
/// one is outstanding, and a value that is not the loan is refused.
#[test]
fn a_loan_is_taken_and_returned() {
    let (_, mut sample) = demo();
    let inner = id(&mut sample, "embedded");
    let label = id(&mut sample, "label");

    let mut child = sample.loan_value(inner).unwrap();
    assert_eq!(child.type_().name(), "Inner");
    let v = id(&mut child, "v");
    child.set_int32_value(v, 9).unwrap();
    // A second loan while one is outstanding.
    let error = sample.loan_value(label).unwrap_err();
    assert!(
        matches!(error, DynamicError::PreconditionNotMet { .. }),
        "{error}"
    );
    // A value that is not the loan.
    let stranger = sample.get_complex_value(inner).unwrap();
    assert!(sample.return_loaned_value(stranger).is_err());
    // A clone has no loan, and is deleted without one.
    assert!(
        DynamicDataFactory::get_instance()
            .delete_data(sample.clone())
            .is_ok()
    );
    sample.return_loaned_value(child).unwrap();
    assert_eq!(
        sample
            .get_complex_value(inner)
            .unwrap()
            .get_int32_value(v)
            .unwrap(),
        9
    );
    // Returned: a new loan is possible; a copy of the loan is not the loan.
    let again = sample.loan_value(inner).unwrap();
    let copy = again.clone();
    assert!(sample.return_loaned_value(copy).is_err());
    sample.return_loaned_value(again).unwrap();
    // With nothing on loan, nothing can be returned.
    let stranger = sample.get_complex_value(inner).unwrap();
    assert!(sample.return_loaned_value(stranger).is_err());
}

/// §7.5.2.11.1 and §7.5.2.11.9: a sequence is appended to by the id one past
/// its end, respects its bound, loses an element by clearing it, and is
/// read and set whole through the plural accessors.
#[test]
fn a_sequence_grows_shrinks_and_is_read_whole() {
    let (_, mut sample) = demo();
    let numbers = id(&mut sample, "numbers");
    sample.set_int32_values(numbers, &[1, 2, 3]).unwrap();
    assert_eq!(sample.get_int32_values(numbers).unwrap(), [1, 2, 3]);
    assert_eq!(sample.get_int64_values(numbers).unwrap(), [1, 2, 3]);
    assert!(sample.get_int16_values(numbers).is_err());
    assert!(
        sample.set_int32_values(numbers, &[1, 2, 3, 4, 5]).is_err(),
        "past the bound"
    );

    let mut seq = sample.get_complex_value(numbers).unwrap();
    assert_eq!(seq.get_item_count(), 3);
    let next = seq.get_member_id_at_index(3).unwrap();
    seq.set_int32_value(next, 4).unwrap();
    assert!(seq.get_member_id_at_index(5).is_err());
    assert!(
        seq.set_int32_value(4, 5).is_err(),
        "the sequence is at its bound"
    );
    seq.clear_value(1).unwrap();
    assert_eq!(seq.get_int32_values(MEMBER_ID_INVALID).unwrap(), [1, 3, 4]);
    seq.set_int32_values(MEMBER_ID_INVALID, &[8, 9]).unwrap();
    sample.set_complex_value(numbers, &seq).unwrap();
    assert_eq!(sample.get_int32_values(numbers).unwrap(), [8, 9]);

    let names = id(&mut sample, "names");
    sample
        .set_string_values(names, &["a".to_string(), "b".to_string()])
        .unwrap();
    assert_eq!(sample.get_string_values(names).unwrap(), ["a", "b"]);
    assert!(sample.get_wstring_values(names).is_err());
}

/// §7.5.2.11.1: a multi-dimensional array is accessed as if flattened; an
/// element is set to its default by clearing, and the count is the product
/// of the bounds.
#[test]
fn an_array_is_flattened() {
    let (_, mut sample) = demo();
    let grid = id(&mut sample, "grid");
    let mut array = sample.get_complex_value(grid).unwrap();
    assert_eq!(array.get_item_count(), 6);
    array.set_int32_value(4, 42).unwrap();
    assert_eq!(
        array.get_int32_values(MEMBER_ID_INVALID).unwrap(),
        [0, 0, 0, 0, 42, 0]
    );
    assert!(array.get_member_id_at_index(6).is_err());
    assert!(array.set_int32_value(6, 1).is_err());
    array.clear_value(4).unwrap();
    assert_eq!(array.get_int32_value(4).unwrap(), 0);
    assert_eq!(array.get_item_count(), 6, "an array keeps its size");
    assert!(array.set_int32_values(MEMBER_ID_INVALID, &[1, 2]).is_err());
    array
        .set_int32_values(MEMBER_ID_INVALID, &[1, 2, 3, 4, 5, 6])
        .unwrap();
    assert_eq!(array.get_int32_value(5).unwrap(), 6);
}

/// §7.5.2.11.1: the values of a map are found by their key, converted to
/// a string; the id of a new key is one past the end.
#[test]
fn a_map_is_reached_by_key() {
    let (_, mut sample) = demo();
    let table = id(&mut sample, "table");
    let mut map = sample.get_complex_value(table).unwrap();
    let a = map.get_member_id_by_name("a").unwrap();
    assert_eq!(a, 0, "a new key takes the next index");
    map.set_int32_value(a, 5).unwrap();
    let b = map.get_member_id_by_name("b").unwrap();
    assert_eq!(b, 1);
    map.set_int32_value(b, 6).unwrap();
    assert_eq!(map.get_item_count(), 2);
    assert_eq!(map.get_member_id_by_name("a").unwrap(), 0);
    assert_eq!(map.get_int32_value(0).unwrap(), 5);
    map.set_int32_value(0, 50).unwrap();
    assert_eq!(map.get_int32_value(0).unwrap(), 50);
    // A value without a key is refused.
    assert!(map.set_int32_value(2, 1).is_err());
    map.clear_value(0).unwrap();
    assert_eq!(map.get_item_count(), 1);
    assert_eq!(map.get_member_id_by_name("b").unwrap(), 0);
}

/// §7.5.2.11.6: a union holds the discriminator and the selected member,
/// two values; writing a member selects it.
#[test]
fn a_union_member_is_selected_by_being_set() {
    let ast = model();
    let ty = DynamicType::find(&ast, "U").unwrap();
    let mut sample = DynamicSample::new(&ty).unwrap();
    // The default: the discriminator at its default selects `default:`.
    assert_eq!(sample.get_item_count(), 2);
    let (a, b, c) = (
        id(&mut sample, "a"),
        id(&mut sample, "b"),
        id(&mut sample, "c"),
    );
    sample.set_string_value(b, "hi").unwrap();
    assert_eq!(sample.get_item_count(), 2);
    assert_eq!(sample.get_string_value(b).unwrap(), "hi");
    sample.set_int32_value(a, 4).unwrap();
    assert_eq!(sample.get_int32_value(a).unwrap(), 4);
    assert_eq!(sample.get_item_count(), 2, "the other member is gone");
    sample.set_float64_value(c, 1.0).unwrap();
    assert_eq!(sample.get_float64_value(c).unwrap(), 1.0);
    assert_eq!(sample.get_item_count(), 2);
}

/// §7.5.2.11.3: `clear_all_values` resets everything; `clear_nonkey_values`
/// keeps the key members.
#[test]
fn clearing_all_values_and_the_nonkey_values() {
    let (_, mut sample) = demo();
    let serial = id(&mut sample, "serial");
    let small = id(&mut sample, "small");
    sample.set_int32_value(serial, 5).unwrap();
    sample.set_int16_value(small, 3).unwrap();
    sample.clear_nonkey_values().unwrap();
    assert_eq!(
        sample.get_int32_value(serial).unwrap(),
        5,
        "the key keeps its value"
    );
    assert_eq!(sample.get_int16_value(small).unwrap(), 0);
    sample.clear_all_values().unwrap();
    assert_eq!(sample.get_int32_value(serial).unwrap(), 0);
}

/// Annex C `get_descriptor` and `set_descriptor`.
#[test]
fn the_descriptor_of_a_value_is_its_types() {
    let (_, mut sample) = demo();
    let serial = id(&mut sample, "serial");
    let descriptor = sample.get_descriptor(serial).unwrap();
    assert_eq!(descriptor.name, "serial");
    assert!(descriptor.is_key);
    assert!(sample.set_descriptor(serial, &descriptor).is_ok());
    let mut other = descriptor.clone();
    other.is_key = false;
    assert!(sample.set_descriptor(serial, &other).is_err());
    // An element of a collection has a descriptor with the element type.
    let numbers = id(&mut sample, "numbers");
    let seq = sample.get_complex_value(numbers).unwrap();
    assert_eq!(seq.get_descriptor(2).unwrap().type_.kind(), TypeKind::Int32);
}

/// §7.5.2.11.7 and §7.5.2.10.2: a sample with a loan out is not deleted.
#[test]
fn a_sample_with_a_loan_out_is_not_deleted() {
    let (_, mut sample) = demo();
    let inner = id(&mut sample, "embedded");
    let child = sample.loan_value(inner).unwrap();
    let factory = DynamicDataFactory::get_instance();
    let error = factory.delete_data(sample).unwrap_err();
    assert!(
        matches!(error, DynamicError::PreconditionNotMet { .. }),
        "{error}"
    );
    drop(child);
    let (_, fresh) = demo();
    factory.delete_data(fresh).unwrap();
}

/// §7.5.2.11: "DynamicData objects have reference semantics". The value
/// `get_complex_value` returns is the value inside the sample: a setter on
/// it is seen through the sample, and a setter on the sample is seen
/// through it.
#[test]
fn a_value_from_get_complex_value_is_the_value_in_the_sample() {
    let (_, mut sample) = demo();
    let inner = id(&mut sample, "embedded");
    let mut child = sample.get_complex_value(inner).unwrap();
    let v = id(&mut child, "v");
    child.set_int32_value(v, 9).unwrap();
    let mut seen = sample.get_complex_value(inner).unwrap();
    assert_eq!(seen.get_int32_value(v).unwrap(), 9, "through the sample");
    assert!(seen.is_same(&child));
    // The other way round.
    seen.set_int32_value(v, 11).unwrap();
    assert_eq!(child.get_int32_value(v).unwrap(), 11);
    // A handle on a handle reaches a value two levels down.
    let mut a = sample.get_complex_value(inner).unwrap();
    let mut b = a.get_complex_value(v).unwrap();
    b.set_int32_value(MEMBER_ID_INVALID, 12).unwrap();
    assert_eq!(child.get_int32_value(v).unwrap(), 12);
    // The sample replaced as a whole through a handle on it.
    let mut whole = sample.share();
    let serial = id(&mut whole, "serial");
    whole.set_int32_value(serial, 3).unwrap();
    assert_eq!(sample.get_int32_value(serial).unwrap(), 3);
    assert!(whole.is_same(&sample));
    // `clone` is the copy of §7.5.2.11.4: equal, and apart.
    let copy = sample.clone();
    assert!(copy.equals(&sample));
    assert!(!copy.is_same(&sample));
    whole.set_int32_value(serial, 4).unwrap();
    assert_eq!(copy.get_int32_value(serial).unwrap(), 3);
    assert!(!copy.equals(&sample));
    a.clear_all_values().unwrap();
    assert_eq!(child.get_int32_value(v).unwrap(), 0);
}

/// A non-optional member that has no value yet is stored when a handle is
/// taken, an absent optional member is `RETCODE_NO_DATA`, and a handle
/// whose value was removed is `RETCODE_NO_DATA` too.
#[test]
fn a_handle_refers_to_stored_values_and_says_when_it_does_not() {
    let (_, mut sample) = demo();
    let maybe = id(&mut sample, "maybe");
    let error = sample.get_complex_value(maybe).unwrap_err();
    assert_eq!(error.return_code(), "RETCODE_NO_DATA");
    sample.set_int32_value(maybe, 5).unwrap();
    let mut held = sample.get_complex_value(maybe).unwrap();
    held.set_int32_value(MEMBER_ID_INVALID, 6).unwrap();
    assert_eq!(sample.get_int32_value(maybe).unwrap(), 6);
    sample.clear_value(maybe).unwrap();
    let error = held.get_int32_value(MEMBER_ID_INVALID).unwrap_err();
    assert_eq!(error.return_code(), "RETCODE_NO_DATA");
    let error = held.set_int32_value(MEMBER_ID_INVALID, 1).unwrap_err();
    assert_eq!(error.return_code(), "RETCODE_NO_DATA");
    // An element removed from the end of a sequence.
    let numbers = id(&mut sample, "numbers");
    sample.set_int32_values(numbers, &[1, 2, 3]).unwrap();
    let mut seq = sample.get_complex_value(numbers).unwrap();
    let mut last = seq.get_complex_value(2).unwrap();
    last.set_int32_value(MEMBER_ID_INVALID, 30).unwrap();
    assert_eq!(sample.get_int32_values(numbers).unwrap(), [1, 2, 30]);
    seq.clear_value(2).unwrap();
    assert_eq!(
        last.get_int32_value(MEMBER_ID_INVALID)
            .unwrap_err()
            .return_code(),
        "RETCODE_NO_DATA"
    );
    // There is no element to refer to past the end.
    assert!(seq.get_complex_value(5).is_err());
}

/// An element of an array, flattened, and the value of a map entry are
/// references, too.
#[test]
fn elements_of_an_array_and_values_of_a_map_are_references() {
    let (_, mut sample) = demo();
    let grid = id(&mut sample, "grid");
    let mut cell = sample
        .get_complex_value(grid)
        .unwrap()
        .get_complex_value(4)
        .unwrap();
    cell.set_int32_value(MEMBER_ID_INVALID, 42).unwrap();
    assert_eq!(sample.get_int32_values(grid).unwrap(), [0, 0, 0, 0, 42, 0]);
    let table = id(&mut sample, "table");
    let mut map = sample.get_complex_value(table).unwrap();
    let key = map.get_member_id_by_name("k").unwrap();
    map.set_int32_value(key, 1).unwrap();
    let mut entry = map.get_complex_value(key).unwrap();
    entry.set_int32_value(MEMBER_ID_INVALID, 2).unwrap();
    assert_eq!(
        sample
            .get_complex_value(table)
            .unwrap()
            .get_int32_value(0)
            .unwrap(),
        2
    );
}

/// What has no storage of its own comes back as a copy: a character of a
/// string and a flag of a bitmask.
#[test]
fn a_character_and_a_flag_are_copies() {
    let (_, mut sample) = demo();
    let label = id(&mut sample, "label");
    sample.set_string_value(label, "abc").unwrap();
    let text = sample.get_complex_value(label).unwrap();
    let mut first = text.get_complex_value(0).unwrap();
    first.set_char8_value(MEMBER_ID_INVALID, 'z').unwrap();
    assert_eq!(
        text.get_char8_value(0).unwrap(),
        'a',
        "a character is a copy"
    );
    let flags = id(&mut sample, "permissions");
    let mask = sample.get_complex_value(flags).unwrap();
    let mut flag = mask.get_complex_value(0).unwrap();
    flag.set_boolean_value(MEMBER_ID_INVALID, true).unwrap();
    assert_eq!(mask.get_item_count(), 0, "a flag is a copy");
}

/// A loan is the reference with the clause's bookkeeping: what the
/// borrower does is in the sample before the loan is returned.
#[test]
fn a_loan_is_a_reference() {
    let (_, mut sample) = demo();
    let inner = id(&mut sample, "embedded");
    let mut loan = sample.loan_value(inner).unwrap();
    let v = id(&mut loan, "v");
    loan.set_int32_value(v, 7).unwrap();
    let seen = sample.get_complex_value(inner).unwrap();
    assert_eq!(seen.get_int32_value(v).unwrap(), 7, "before the return");
    sample.return_loaned_value(loan).unwrap();
}

const PRIMITIVES: &str = r#"
    struct Every {
        boolean boolean_;
        octet byte_;
        int8 int8_;
        uint8 uint8_;
        short int16_;
        unsigned short uint16_;
        long int32_;
        unsigned long uint32_;
        long long int64_;
        unsigned long long uint64_;
        float float32_;
        double float64_;
        long double float128_;
        char char8_;
        wchar char16_;
    };
"#;

type Reader = fn(&DynamicSample, u32) -> bool;
type Writer = fn(&mut DynamicSample, u32) -> bool;

/// §7.5.2.11.9: the promotions, as the clause lists them. A getter works
/// on a member of its own type and on a member of each type that promotes
/// to it; a setter works on a member of its own type and on a member of
/// each type it promotes to. Every pair of the 15 primitive types is
/// tried, getter and setter, against the clause's list.
#[test]
fn the_promotion_table_of_the_specification() {
    let parsed = zerodds_idl_parse::parse(PRIMITIVES);
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "every.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let mut ast = lowered.ast;
    assert!(ast.resolve_all_by_name().unwrap().is_clean());
    let ty = DynamicType::find(&Arc::new(ast), "Every").unwrap();

    use zerodds_float128::Float128;
    // (type, member, getter, setter)
    let kinds: Vec<(&str, &str, Reader, Writer)> = vec![
        (
            "Boolean",
            "boolean_",
            |s, i| s.get_boolean_value(i).is_ok(),
            |s, i| s.set_boolean_value(i, true).is_ok(),
        ),
        (
            "Byte",
            "byte_",
            |s, i| s.get_byte_value(i).is_ok(),
            |s, i| s.set_byte_value(i, 1).is_ok(),
        ),
        (
            "Int8",
            "int8_",
            |s, i| s.get_int8_value(i).is_ok(),
            |s, i| s.set_int8_value(i, 1).is_ok(),
        ),
        (
            "UInt8",
            "uint8_",
            |s, i| s.get_uint8_value(i).is_ok(),
            |s, i| s.set_uint8_value(i, 1).is_ok(),
        ),
        (
            "Int16",
            "int16_",
            |s, i| s.get_int16_value(i).is_ok(),
            |s, i| s.set_int16_value(i, 1).is_ok(),
        ),
        (
            "UInt16",
            "uint16_",
            |s, i| s.get_uint16_value(i).is_ok(),
            |s, i| s.set_uint16_value(i, 1).is_ok(),
        ),
        (
            "Int32",
            "int32_",
            |s, i| s.get_int32_value(i).is_ok(),
            |s, i| s.set_int32_value(i, 1).is_ok(),
        ),
        (
            "UInt32",
            "uint32_",
            |s, i| s.get_uint32_value(i).is_ok(),
            |s, i| s.set_uint32_value(i, 1).is_ok(),
        ),
        (
            "Int64",
            "int64_",
            |s, i| s.get_int64_value(i).is_ok(),
            |s, i| s.set_int64_value(i, 1).is_ok(),
        ),
        (
            "UInt64",
            "uint64_",
            |s, i| s.get_uint64_value(i).is_ok(),
            |s, i| s.set_uint64_value(i, 1).is_ok(),
        ),
        (
            "Float32",
            "float32_",
            |s, i| s.get_float32_value(i).is_ok(),
            |s, i| s.set_float32_value(i, 1.0).is_ok(),
        ),
        (
            "Float64",
            "float64_",
            |s, i| s.get_float64_value(i).is_ok(),
            |s, i| s.set_float64_value(i, 1.0).is_ok(),
        ),
        (
            "Float128",
            "float128_",
            |s, i| s.get_float128_value(i).is_ok(),
            |s, i| s.set_float128_value(i, Float128::from_f64(1.0)).is_ok(),
        ),
        (
            "Char8",
            "char8_",
            |s, i| s.get_char8_value(i).is_ok(),
            |s, i| s.set_char8_value(i, 'a').is_ok(),
        ),
        (
            "Char16",
            "char16_",
            |s, i| s.get_char16_value(i).is_ok(),
            |s, i| s.set_char16_value(i, 'a').is_ok(),
        ),
    ];
    // The clause's list: from → to.
    let promotes = |from: &str, to: &str| -> bool {
        let all = [
            "Boolean", "Byte", "Int8", "UInt8", "Int16", "UInt16", "Int32", "UInt32", "Int64",
            "UInt64", "Float32", "Float64", "Float128", "Char8", "Char16",
        ];
        let list: &[&str] = match from {
            "Int8" => &["Int16", "Int32", "Int64", "Float32", "Float64", "Float128"],
            "Int16" => &["Int32", "Int64", "Float32", "Float64", "Float128"],
            "Int32" => &["Int64", "Float64", "Float128"],
            "Int64" => &["Float128"],
            "UInt8" => &[
                "Int16", "Int32", "Int64", "UInt16", "UInt32", "UInt64", "Float32", "Float64",
                "Float128",
            ],
            "UInt16" => &[
                "Int32", "Int64", "UInt32", "UInt64", "Float32", "Float64", "Float128",
            ],
            "UInt32" => &["Int64", "UInt64", "Float64", "Float128"],
            "UInt64" => &["Float128"],
            "Float32" => &["Float64", "Float128"],
            "Float64" => &["Float128"],
            "Float128" => &[],
            "Char8" => &[
                "Char16", "Int16", "Int32", "Int64", "Float32", "Float64", "Float128",
            ],
            "Char16" => &["Int32", "Int64", "Float32", "Float64", "Float128"],
            "Byte" => &all,
            "Boolean" => &[
                "Int8", "Int16", "Int32", "Int64", "UInt8", "UInt16", "UInt32", "UInt64",
                "Float32", "Float64", "Float128",
            ],
            other => panic!("{other}"),
        };
        list.contains(&to)
    };
    let mut sample = DynamicSample::new(&ty).unwrap();
    // Every pair is checked and the mismatches are collected, so that one
    // run shows all of them.
    let mut mismatches: Vec<String> = Vec::new();
    for (member_kind, member, _, _) in &kinds {
        let id = sample.get_member_id_by_name(member).unwrap();
        for (accessor_kind, _, get, set) in &kinds {
            let readable = member_kind == accessor_kind || promotes(member_kind, accessor_kind);
            if get(&sample, id) != readable {
                mismatches.push(format!(
                    "get_{accessor_kind} on a {member_kind} member: expected {readable}"
                ));
            }
            let settable = member_kind == accessor_kind || promotes(accessor_kind, member_kind);
            if set(&mut sample, id) != settable {
                mismatches.push(format!(
                    "set_{accessor_kind} on a {member_kind} member: expected {settable}"
                ));
            }
        }
    }
    assert!(
        mismatches.is_empty(),
        "{} mismatches against §7.5.2.11.9:\n{}",
        mismatches.len(),
        mismatches.join("\n")
    );
}
