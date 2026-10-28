//! DDS-XTypes 1.3 §7.5.2.2.13 and §7.5.2.2.14: `create_type_w_uri` and
//! `create_type_w_document`.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use zerodds_types::dynamic::TypeKind;
use zerodds_xml_compose::{create_type_w_document, create_type_w_uri};

const DOCUMENT: &str = "<dds><types><module name=\"T\">\
    <struct name=\"S\"><member name=\"a\" type=\"int32\" key=\"true\"/>\
    <member name=\"b\" type=\"string\" stringMaxLength=\"8\"/></struct>\
    </module></types></dds>";

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("xc_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// §7.5.2.2.14: the type of that fully qualified name, from the document
/// in the string.
#[test]
fn a_type_is_created_from_a_document() {
    let ty = create_type_w_document(DOCUMENT, "T::S", &[]).unwrap();
    assert_eq!(ty.kind(), TypeKind::Structure);
    assert_eq!(ty.qualified_name(), "T::S");
    assert_eq!(ty.member_count(), 2);
    assert!(ty.member_by_name("a").unwrap().descriptor().unwrap().is_key);
    assert_eq!(
        ty.member_by_name("b")
            .unwrap()
            .dynamic_type()
            .unwrap()
            .descriptor()
            .unwrap()
            .bound,
        [8]
    );
}

/// "If no type exists of this name in the document (which will trivially
/// be the case if the name is nil or the empty string), the operation
/// shall fail"; a document with an error fails too.
#[test]
fn a_missing_type_and_a_bad_document_fail() {
    assert!(create_type_w_document(DOCUMENT, "T::Nope", &[]).is_err());
    assert!(create_type_w_document(DOCUMENT, "", &[]).is_err());
    assert!(
        create_type_w_document("<dds><types><widget name=\"x\"/></types></dds>", "x", &[]).is_err()
    );
}

/// §7.5.2.2.14: the include paths are searched for the files the
/// document includes.
#[test]
fn the_include_paths_are_searched() {
    let dir = temp_dir("includes");
    std::fs::write(
        dir.join("base.xml"),
        "<dds><types><module name=\"B\">\
         <struct name=\"Base\"><member name=\"x\" type=\"int32\"/></struct>\
         </module></types></dds>",
    )
    .unwrap();
    let document = "<dds><types><include file=\"base.xml\"/><module name=\"D\">\
         <struct name=\"Derived\" baseType=\"B::Base\"><member name=\"y\" type=\"int32\"/></struct>\
         </module></types></dds>";
    // Without the path the include cannot be read.
    assert!(create_type_w_document(document, "D::Derived", &[]).is_err());
    let ty = create_type_w_document(document, "D::Derived", &[dir.to_str().unwrap()]).unwrap();
    assert_eq!(ty.base_type().unwrap().qualified_name(), "B::Base");
    std::fs::remove_dir_all(&dir).unwrap();
}

/// §7.5.2.2.13: a `file://` URL is read, and the directory of the
/// document is on the include path implicitly.
#[test]
fn a_type_is_created_from_a_file_url() {
    let dir = temp_dir("uri");
    std::fs::write(dir.join("types.xml"), DOCUMENT).unwrap();
    let url = format!("file://{}", dir.join("types.xml").to_str().unwrap());
    let ty = create_type_w_uri(&url, "T::S", &[]).unwrap();
    assert_eq!(ty.qualified_name(), "T::S");

    std::fs::write(
        dir.join("main.xml"),
        "<dds><types><include file=\"types.xml\"/><module name=\"M\">\
         <struct name=\"U\" baseType=\"T::S\"><member name=\"c\" type=\"int32\"/></struct>\
         </module></types></dds>",
    )
    .unwrap();
    let url = format!("file://{}", dir.join("main.xml").to_str().unwrap());
    let ty = create_type_w_uri(&url, "M::U", &[]).unwrap();
    assert_eq!(ty.base_type().unwrap().qualified_name(), "T::S");

    // Other schemes, and a file that is not there.
    assert!(create_type_w_uri("http://example.org/types.xml", "T::S", &[]).is_err());
    assert!(create_type_w_uri("file:///nonexistent/types.xml", "T::S", &[]).is_err());
    std::fs::remove_dir_all(&dir).unwrap();
}
