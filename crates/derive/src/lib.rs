// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-derive`. Safety classification: **SAFE**.
//!
//! `derive(DdsType)` and `derive(WireRepr)` for `zerodds-codec`, which
//! re-exports both; depend on `zerodds-codec` and write
//! `use zerodds_codec::{DdsType, WireRepr};`.
//!
//! ## `derive(DdsType)` on a struct
//!
//! A DDS topic type defined in Rust:
//!
//! ```ignore
//! #[derive(DdsType)]
//! #[dds(name = "app::Track", extensibility = "appendable")]
//! struct Track {
//!     #[dds(key)]
//!     id: core::num::NonZeroU32,
//!     at: Dtg,
//!     #[dds(bound = 64)]
//!     label: String,
//!     note: Option<String>,
//! }
//! ```
//!
//! The expansion is the type's description (`DdsTypeInfo`: the member
//! types as a list, the declaration as a constant), its codec
//! (`DdsSerialize`, `DdsDeserialize`), its `DdsMember` and `DdsMemberRead`
//! impls for use as a member of another type, the `zerodds_dcps::DdsType`
//! impl, and `Track::idl()`. The macro sees the struct's tokens and nothing
//! else, so it decides no IDL type: each member's comes from its Rust type
//! through `DdsMember::DESC` when the program is compiled, and the layout,
//! the `TypeObject` and the `TypeIdentifier` come from that IDL through the
//! IDL front end and the layout planner when it runs.
//!
//! On the struct: `name = "mod::Name"` (default: the struct's name),
//! `extensibility = "final" | "appendable" | "mutable"` (default
//! `appendable`, the XTypes 1.3 default), `autoid = "sequential" |
//! "hash"`, `nested`, `codec = "manual"` (the description, the member
//! impls and the DCPS impl only; `DdsSerialize` and `DdsDeserialize` are
//! written by hand).
//!
//! On a field: `key`, `id = N`, `hashid` or `hashid = "text"`, `bound =
//! N` (of a `string` or the outermost `sequence`), `name = "idl_name"`,
//! `optional` (checked: the field's type must be an `Option`, which is
//! what makes a member `@optional`), `with = "path::Adapter"` (a type from
//! another crate, through a `WireReprWith` adapter that implements
//! `Default`).
//!
//! ## `derive(DdsType)` on an enum
//!
//! A field-less enum is an IDL `enum`: `name`, `extensibility = "final" |
//! "appendable"` (DDS-XTypes 1.3 §7.2.3, Table 12; unwritten, the IDL
//! default), `bit_bound = N`; on a variant `name` and `value = N` (or an
//! integer discriminant), the IDL `@value`. It is also a union
//! discriminator type.
//!
//! An enum whose variants carry data is an IDL `union`:
//!
//! ```ignore
//! #[derive(DdsType)]
//! #[dds(name = "app::Reading", discriminator = "i16", extensibility = "final")]
//! enum Reading {
//!     #[dds(case = 1)]
//!     Celsius(f64),                // case 1: double celsius;
//!     #[dds(case = [2, 3], name = "kelvin")]
//!     Kelvin(i16, f64),            // case 2: case 3: double kelvin;
//!     #[dds(default)]
//!     Raw(i16, Vec<u8>),           // default: sequence<octet> raw;
//! }
//! ```
//!
//! On the enum, beside `name`, `extensibility`, `autoid`, `nested` and
//! `codec = "manual"`: `discriminator = "Type"`, the Rust type of the
//! discriminator — an integer, `bool`, `char` or a derived field-less enum;
//! default `i32` (`long`); `key`, the discriminator is the union's key
//! (`switch (@key D)`, DDS-XTypes 1.3 §7.2.2.4.4.4.8): a union topic type
//! is then keyed and its key holder is the discriminator, and as a `key`
//! field of a struct only its discriminator is part of that key. On a
//! variant: `case = label` or `case = [label,
//! …]` (expressions of the discriminator type); without `case` and without
//! `default` the label is one past the highest label before it, the first
//! `0`; `default`; `id = N`, `hashid`, `name`, `bound`; `no_member` for a
//! variant `V(D)` that holds a discriminator no case takes (the member is
//! absent, IDL 4.2 §7.4.1.4.4.4). A unit variant with one label,
//! `#[dds(case = label)] V`, is that discriminator value without a member:
//! it goes out alone and declares no branch, beside or instead of the
//! `no_member` variant, which then holds the values no variant names; a
//! `default` case leaves no such value and excludes both. A variant `V(T)` holds the branch member;
//! a variant of a case with several labels, and of the `default:` case, is
//! `V(D, T)` and holds the discriminator it was read with, so it goes out
//! again under the same label. A hand-written codec names the branch by a
//! type-level index (`B0`, `B1`, …) and a value of exactly that branch's
//! type: another type does not compile.
//!
//! ## Generic types and borrowed fields
//!
//! A generic type is one IDL type per instantiation. Its name is a
//! template in which `{P}` stands for the argument of the parameter `P`;
//! the default template is the type's name with `_{P}` per type or `const`
//! parameter (`Pair<T>` is `Pair_{T}`; `Pair<u32>` is `Pair_unsigned_long`).
//! `name = "app::Pair_{T}"` sets it, and must name every parameter. An
//! argument is spelled from its IDL type (`zerodds_wire::codec::desc::
//! arg_spelling`). A type parameter is bound by `DdsMember`.
//!
//! A field may borrow from the received sample: `&'a str`, `&'a [u8]`,
//! `Cow<'a, str>`, `Cow<'a, [u8]>`, or a derived type with a lifetime. The
//! codec API reads them lent out of the bytes (`zerodds_codec::decode`
//! over `&'de [u8]`). A type with a lifetime gets its `zerodds_dcps::
//! DdsType` impl for its `'static` form (`DdsType::BORROWS`): a DataReader
//! and a DataWriter are of that form, the reader lends its samples
//! (`DataReader::take_loan`) and `zerodds_codec::LoanExt::read_as` reads
//! the type with its lifetime from the loan; `zerodds_codec::write` writes
//! one of any lifetime. Its `decode` refuses: `'static` borrows do not
//! outlive the received octets.
//!
//! ## `derive(WireRepr)`
//!
//! A checked type that stands for an IDL primitive or `string`:
//!
//! ```ignore
//! #[derive(WireRepr)]
//! #[wire(idl = "string")] // reads through TryFrom<&str>, writes through Display
//! struct Dtg(DateTimeGroup);
//!
//! #[derive(WireRepr, Clone)]
//! #[wire(idl = "uint32")] // reads through TryFrom<u32>, writes through Into<u32>
//! struct Channel(u32);
//! ```
//!
//! A refused `try_from` refuses the sample, with the error's text as the
//! reason. The type works as a member of a derived type and as the
//! target of `rust_type` in a `build.rs` of `zerodds-build`.
#![forbid(unsafe_code)]

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as Tokens};
use quote::{format_ident, quote};
use syn::spanned::Spanned;
use syn::{
    Attribute, Data, DeriveInput, Expr, ExprLit, Fields, GenericParam, Lit, LitInt, LitStr, Path,
    Type, parse_macro_input,
};

/// `derive(DdsType)`: see the crate documentation.
#[proc_macro_derive(DdsType, attributes(dds))]
pub fn derive_dds_type(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match dds_type(&input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

/// `derive(WireRepr)`: see the crate documentation.
#[proc_macro_derive(WireRepr, attributes(wire))]
pub fn derive_wire_repr(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match wire_repr(&input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

// --- derive(DdsType) -------------------------------------------------

#[derive(Default)]
struct TypeAttrs {
    name: Option<LitStr>,
    extensibility: Option<(String, Span)>,
    autoid: Option<(String, Span)>,
    nested: bool,
    manual: bool,
    bit_bound: Option<LitInt>,
    discriminator: Option<Type>,
    /// `key` on a union: its discriminator is its key.
    key: Option<Span>,
}

#[derive(Default)]
struct FieldAttrs {
    key: bool,
    id: Option<LitInt>,
    hashid: Option<Option<LitStr>>,
    bound: Option<LitInt>,
    name: Option<LitStr>,
    optional: bool,
    with: Option<Path>,
}

#[derive(Default)]
struct VariantAttrs {
    name: Option<LitStr>,
    value: Option<LitInt>,
    labels: Option<Vec<Expr>>,
    default: bool,
    no_member: bool,
    id: Option<LitInt>,
    hashid: Option<Option<LitStr>>,
    bound: Option<LitInt>,
}

fn dds_attrs(
    attrs: &[Attribute],
    mut each: impl FnMut(syn::meta::ParseNestedMeta<'_>) -> syn::Result<()>,
) -> syn::Result<()> {
    for attr in attrs {
        if attr.path().is_ident("dds") {
            attr.parse_nested_meta(&mut each)?;
        }
    }
    Ok(())
}

fn type_attrs(input: &DeriveInput) -> syn::Result<TypeAttrs> {
    let mut out = TypeAttrs::default();
    dds_attrs(&input.attrs, |meta| {
        if meta.path.is_ident("name") {
            out.name = Some(meta.value()?.parse()?);
        } else if meta.path.is_ident("extensibility") {
            let value: LitStr = meta.value()?.parse()?;
            out.extensibility = Some((value.value(), value.span()));
        } else if meta.path.is_ident("autoid") {
            let value: LitStr = meta.value()?.parse()?;
            out.autoid = Some((value.value(), value.span()));
        } else if meta.path.is_ident("nested") {
            out.nested = true;
        } else if meta.path.is_ident("codec") {
            let value: LitStr = meta.value()?.parse()?;
            if value.value() != "manual" {
                return Err(syn::Error::new(
                    value.span(),
                    "`codec` takes \"manual\": the codec is written by hand",
                ));
            }
            out.manual = true;
        } else if meta.path.is_ident("bit_bound") {
            out.bit_bound = Some(meta.value()?.parse()?);
        } else if meta.path.is_ident("discriminator") {
            let value: LitStr = meta.value()?.parse()?;
            out.discriminator = Some(value.parse()?);
        } else if meta.path.is_ident("key") {
            out.key = Some(meta.path.span());
        } else {
            return Err(meta.error(
                "unknown `dds` attribute on a type; known: name, extensibility, autoid, nested, \
                 codec, bit_bound, discriminator, key",
            ));
        }
        Ok(())
    })?;
    Ok(out)
}

fn field_attrs(attrs: &[Attribute]) -> syn::Result<FieldAttrs> {
    let mut out = FieldAttrs::default();
    dds_attrs(attrs, |meta| {
        if meta.path.is_ident("key") {
            out.key = true;
        } else if meta.path.is_ident("id") {
            out.id = Some(meta.value()?.parse()?);
        } else if meta.path.is_ident("hashid") {
            out.hashid = Some(if meta.input.peek(syn::Token![=]) {
                Some(meta.value()?.parse()?)
            } else {
                None
            });
        } else if meta.path.is_ident("bound") {
            out.bound = Some(meta.value()?.parse()?);
        } else if meta.path.is_ident("name") {
            out.name = Some(meta.value()?.parse()?);
        } else if meta.path.is_ident("optional") {
            out.optional = true;
        } else if meta.path.is_ident("with") {
            let value: LitStr = meta.value()?.parse()?;
            out.with = Some(value.parse()?);
        } else {
            return Err(meta.error(
                "unknown `dds` attribute on a field; known: key, id, hashid, bound, name, \
                 optional, with",
            ));
        }
        Ok(())
    })?;
    Ok(out)
}

fn variant_attrs(attrs: &[Attribute]) -> syn::Result<VariantAttrs> {
    let mut out = VariantAttrs::default();
    dds_attrs(attrs, |meta| {
        if meta.path.is_ident("name") {
            out.name = Some(meta.value()?.parse()?);
        } else if meta.path.is_ident("value") {
            out.value = Some(meta.value()?.parse()?);
        } else if meta.path.is_ident("case") {
            let expr: Expr = meta.value()?.parse()?;
            out.labels = Some(match expr {
                Expr::Array(array) => array.elems.into_iter().collect(),
                other => vec![other],
            });
        } else if meta.path.is_ident("default") {
            out.default = true;
        } else if meta.path.is_ident("no_member") {
            out.no_member = true;
        } else if meta.path.is_ident("id") {
            out.id = Some(meta.value()?.parse()?);
        } else if meta.path.is_ident("hashid") {
            out.hashid = Some(if meta.input.peek(syn::Token![=]) {
                Some(meta.value()?.parse()?)
            } else {
                None
            });
        } else if meta.path.is_ident("bound") {
            out.bound = Some(meta.value()?.parse()?);
        } else {
            return Err(meta.error(
                "unknown `dds` attribute on a variant; known: name, value, case, default, \
                 no_member, id, hashid, bound",
            ));
        }
        Ok(())
    })?;
    Ok(out)
}

/// The IDL name: the attribute's, else the Rust identifier without a raw
/// prefix.
fn idl_name(given: Option<&LitStr>, ident: &syn::Ident) -> String {
    given.map_or_else(
        || {
            let text = ident.to_string();
            text.strip_prefix("r#").map_or(text.clone(), str::to_string)
        },
        LitStr::value,
    )
}

fn check_scoped_name(name: &str, span: Span) -> syn::Result<()> {
    let ok = !name.is_empty()
        && name.split("::").all(|segment| {
            let mut chars = segment.chars();
            chars
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        });
    if ok {
        Ok(())
    } else {
        Err(syn::Error::new(
            span,
            "an IDL scoped name: identifiers separated by `::`, as `app::Track`",
        ))
    }
}

fn check_identifier(name: &str, span: Span) -> syn::Result<()> {
    if name.contains("::") {
        return Err(syn::Error::new(span, "an IDL identifier, without `::`"));
    }
    check_scoped_name(name, span)
}

/// `Option<…>` written as such.
///
/// zerodds-lint: recursion-depth 64 — one frame per `Type::Group` or
/// `Type::Paren` around the field's type, as many as the source writes.
fn is_option(ty: &Type) -> bool {
    match ty {
        Type::Path(path) => path
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "Option"),
        Type::Group(group) => is_option(&group.elem),
        Type::Paren(paren) => is_option(&paren.elem),
        _ => false,
    }
}

/// The generic parameters of the derived type, and the impl headers made
/// from them.
struct Generics<'a> {
    input: &'a DeriveInput,
    /// The type and `const` parameters, by name, in declaration order.
    named: Vec<syn::Ident>,
    /// The type parameters.
    types: Vec<syn::Ident>,
    /// The lifetimes.
    lifetimes: Vec<syn::Lifetime>,
    /// `TypeArg` of each type and `const` parameter, for the name.
    args: Vec<Tokens>,
    /// The parameters of `__args`, the function that lists them.
    args_params: Vec<Tokens>,
}

impl<'a> Generics<'a> {
    fn new(input: &'a DeriveInput) -> syn::Result<Self> {
        let codec = quote!(::zerodds_codec::codec);
        let mut out = Self {
            input,
            named: Vec::new(),
            types: Vec::new(),
            lifetimes: Vec::new(),
            args: Vec::new(),
            args_params: Vec::new(),
        };
        for param in &input.generics.params {
            match param {
                GenericParam::Lifetime(lifetime) => out.lifetimes.push(lifetime.lifetime.clone()),
                GenericParam::Type(type_param) => {
                    let ident = &type_param.ident;
                    out.named.push(ident.clone());
                    out.types.push(ident.clone());
                    out.args
                        .push(quote!(#codec::TypeArg::Type(<#ident as #codec::DdsMember>::DESC)));
                    out.args_params.push(quote!(#ident: #codec::DdsMember));
                }
                GenericParam::Const(const_param) => {
                    let ident = &const_param.ident;
                    let ty = &const_param.ty;
                    out.named.push(ident.clone());
                    let kind = match ty {
                        Type::Path(path) if path.path.is_ident("bool") => {
                            quote!(#codec::TypeArg::Bool(#ident))
                        }
                        Type::Path(path) if path.path.is_ident("char") => {
                            quote!(#codec::TypeArg::Char(#ident))
                        }
                        Type::Path(path)
                            if [
                                "u8", "u16", "u32", "u64", "usize", "i8", "i16", "i32", "i64",
                                "isize",
                            ]
                            .iter()
                            .any(|name| path.path.is_ident(name)) =>
                        {
                            quote!(#codec::TypeArg::Int(#ident as i128))
                        }
                        _ => {
                            return Err(syn::Error::new(
                                ty.span(),
                                "a `const` parameter of an integer type, `bool` or `char`",
                            ));
                        }
                    };
                    out.args.push(kind);
                    out.args_params.push(quote!(const #ident: #ty));
                }
            }
        }
        Ok(out)
    }

    fn is_generic(&self) -> bool {
        !self.named.is_empty()
    }

    fn has_lifetimes(&self) -> bool {
        !self.lifetimes.is_empty()
    }

    /// The parameters as written, for an impl header.
    fn params(&self) -> Vec<&GenericParam> {
        self.input.generics.params.iter().collect()
    }

    /// The type as the impls name it: `Track<'a, T>`.
    fn self_ty(&self) -> Tokens {
        let ident = &self.input.ident;
        let (_, ty_generics, _) = self.input.generics.split_for_impl();
        quote!(#ident #ty_generics)
    }

    /// The written `where` predicates, and `extra` per type parameter.
    fn predicates(&self, extra: &impl Fn(&syn::Ident) -> Tokens) -> Vec<Tokens> {
        let mut out: Vec<Tokens> = self
            .input
            .generics
            .where_clause
            .iter()
            .flat_map(|clause| clause.predicates.iter().map(|p| quote!(#p)))
            .collect();
        for ident in &self.types {
            out.push(extra(ident));
        }
        out
    }

    /// `impl<…> #trait for Self where …` with every type parameter a
    /// `DdsMember`.
    fn owned_impl(&self, trait_: &Tokens, more: &[Tokens]) -> Tokens {
        let codec = quote!(::zerodds_codec::codec);
        let params = self.params();
        let self_ty = self.self_ty();
        let mut predicates = self.predicates(&|ident| quote!(#ident: #codec::DdsMember));
        predicates.extend(more.iter().cloned());
        quote!(impl<#(#params),*> #trait_ for #self_ty where #(#predicates,)*)
    }

    /// `impl<…> #trait for Track<'static, …> where …`: a type with
    /// lifetimes, each of them `'static`; the type and `const` parameters
    /// as written, every type parameter a `DdsMember`.
    fn static_impl(&self, trait_: &Tokens) -> Tokens {
        let codec = quote!(::zerodds_codec::codec);
        let ident = &self.input.ident;
        let params: Vec<&GenericParam> = self
            .input
            .generics
            .params
            .iter()
            .filter(|param| !matches!(param, GenericParam::Lifetime(_)))
            .collect();
        let args: Vec<Tokens> = self
            .input
            .generics
            .params
            .iter()
            .map(|param| match param {
                GenericParam::Lifetime(_) => quote!('static),
                GenericParam::Type(type_param) => {
                    let name = &type_param.ident;
                    quote!(#name)
                }
                GenericParam::Const(const_param) => {
                    let name = &const_param.ident;
                    quote!(#name)
                }
            })
            .collect();
        let predicates = self.predicates(&|name| quote!(#name: #codec::DdsMember));
        quote!(impl<#(#params),*> #trait_ for #ident<#(#args),*> where #(#predicates,)*)
    }

    /// `impl<'__de: 'a + …, …> #trait for Self where …` with every type
    /// parameter a `DdsMemberRead<'__de>`: the impls that read, and may
    /// lend from the bytes.
    fn read_impl(&self, trait_: &Tokens, more: &[Tokens]) -> Tokens {
        let codec = quote!(::zerodds_codec::codec);
        let params = self.params();
        let self_ty = self.self_ty();
        let lifetimes = &self.lifetimes;
        let de = if lifetimes.is_empty() {
            quote!('__de)
        } else {
            quote!('__de: #(#lifetimes)+*)
        };
        let mut predicates = self.predicates(&|ident| quote!(#ident: #codec::DdsMemberRead<'__de>));
        predicates.extend(more.iter().cloned());
        quote!(impl<#de, #(#params),*> #trait_ for #self_ty where #(#predicates,)*)
    }

    /// The name template: the given one or the type's name with `_{P}` per
    /// parameter; every parameter must appear in it.
    fn template(&self, given: Option<&LitStr>) -> syn::Result<String> {
        let ident = &self.input.ident;
        let span = given.map_or(ident.span(), LitStr::span);
        let template = match given {
            Some(name) => name.value(),
            None => {
                let mut name = idl_name(None, ident);
                for param in &self.named {
                    name.push_str(&format!("_{{{param}}}"));
                }
                name
            }
        };
        // `{P}` for every parameter, nothing else in braces, and a scoped
        // name once each is replaced by an identifier part.
        let mut plain = template.clone();
        for param in &self.named {
            let placeholder = format!("{{{param}}}");
            if !plain.contains(&placeholder) {
                return Err(syn::Error::new(
                    span,
                    format!(
                        "the IDL name of a generic type names every parameter: `{placeholder}` \
                         is missing, and every instantiation would have one name"
                    ),
                ));
            }
            plain = plain.replace(&placeholder, "x");
        }
        if plain.contains('{') || plain.contains('}') {
            return Err(syn::Error::new(
                span,
                "a `{…}` in the IDL name that names no type or const parameter",
            ));
        }
        check_scoped_name(&plain, span)?;
        Ok(template)
    }

    /// `__args`, the function that lists the arguments, and the
    /// `TypeArgs` the description takes; nothing for a type without
    /// parameters.
    fn args_items(&self) -> (Tokens, Tokens) {
        if !self.is_generic() {
            return (quote!(), quote!());
        }
        let codec = quote!(::zerodds_codec::codec);
        let params = &self.args_params;
        let args = &self.args;
        let names: Vec<String> = self.named.iter().map(ToString::to_string).collect();
        let named = &self.named;
        (
            quote! {
                #[allow(non_snake_case)]
                fn __args<#(#params),*>() -> ::std::vec::Vec<#codec::TypeArg> {
                    ::std::vec![#(#args),*]
                }
            },
            quote! {
                .generic(#codec::TypeArgs {
                    params: &[#(#names),*],
                    values: __args::<#(#named),*>,
                })
            },
        )
    }
}

fn dds_type(input: &DeriveInput) -> syn::Result<Tokens> {
    let attrs = type_attrs(input)?;
    let generics = Generics::new(input)?;
    match &input.data {
        Data::Struct(data) => {
            if let Some(bits) = &attrs.bit_bound {
                return Err(syn::Error::new(
                    bits.span(),
                    "`bit_bound` is for a field-less enum",
                ));
            }
            if let Some(discriminator) = &attrs.discriminator {
                return Err(syn::Error::new(
                    discriminator.span(),
                    "`discriminator` is for an enum whose variants carry data",
                ));
            }
            if let Some(span) = attrs.key {
                return Err(syn::Error::new(
                    span,
                    "`key` on a type is for a union, whose key is its discriminator; a struct's \
                     key members are its fields marked `#[dds(key)]`",
                ));
            }
            let template = generics.template(attrs.name.as_ref())?;
            let fields: Vec<&syn::Field> = match &data.fields {
                Fields::Named(fields) => fields.named.iter().collect(),
                Fields::Unit => Vec::new(),
                Fields::Unnamed(fields) => {
                    return Err(syn::Error::new(
                        fields.span(),
                        "derive(DdsType) takes a struct with named fields: the field names are \
                         the IDL member names",
                    ));
                }
            };
            structure(&attrs, &generics, &template, &fields)
        }
        Data::Enum(data) => {
            let all_unit = data
                .variants
                .iter()
                .all(|variant| matches!(variant.fields, Fields::Unit));
            if all_unit && attrs.discriminator.is_none() {
                if let Some(span) = attrs.key {
                    return Err(syn::Error::new(
                        span,
                        "`key` is for a union, whose key is its discriminator; an IDL enum has \
                         no members to be a key",
                    ));
                }
                if generics.is_generic() || generics.has_lifetimes() {
                    return Err(syn::Error::new(
                        input.generics.span(),
                        "a field-less enum is an IDL enum, which takes no parameters",
                    ));
                }
                let name = idl_name(attrs.name.as_ref(), &input.ident);
                check_scoped_name(
                    &name,
                    attrs.name.as_ref().map_or(input.ident.span(), LitStr::span),
                )?;
                enumeration(input, &attrs, &name, data)
            } else {
                if let Some(bits) = &attrs.bit_bound {
                    return Err(syn::Error::new(
                        bits.span(),
                        "`bit_bound` is for a field-less enum",
                    ));
                }
                let template = generics.template(attrs.name.as_ref())?;
                union(input, &attrs, &generics, &template, data)
            }
        }
        Data::Union(data) => Err(syn::Error::new(
            data.union_token.span(),
            "derive(DdsType) takes a struct or an enum",
        )),
    }
}

/// The extensibility named by the attribute: the kind's type and the
/// `zerodds_dcps::Extensibility` variant.
fn extensibility(attrs: &TypeAttrs) -> syn::Result<(Tokens, syn::Ident)> {
    let codec = quote!(::zerodds_codec::codec);
    Ok(match attrs.extensibility.as_ref() {
        None => (quote!(#codec::Appendable), format_ident!("Appendable")),
        Some((text, span)) => match text.as_str() {
            "final" => (quote!(#codec::Final), format_ident!("Final")),
            "appendable" => (quote!(#codec::Appendable), format_ident!("Appendable")),
            "mutable" => (quote!(#codec::Mutable), format_ident!("Mutable")),
            _ => {
                return Err(syn::Error::new(
                    *span,
                    "`extensibility` is \"final\", \"appendable\" or \"mutable\"",
                ));
            }
        },
    })
}

/// `.autoid(…)` and `.nested()` behind a layout constructor.
fn layout_modifiers(attrs: &TypeAttrs) -> syn::Result<Tokens> {
    let codec = quote!(::zerodds_codec::codec);
    let mut out = quote!();
    if let Some((text, span)) = &attrs.autoid {
        let autoid = match text.as_str() {
            "sequential" => quote!(#codec::AutoId::Sequential),
            "hash" => quote!(#codec::AutoId::Hash),
            _ => {
                return Err(syn::Error::new(
                    *span,
                    "`autoid` is \"sequential\" or \"hash\"",
                ));
            }
        };
        out = quote!(#out.autoid(#autoid));
    }
    if attrs.nested {
        out = quote!(#out.nested());
    }
    Ok(out)
}

/// The impls a struct and a union share: `DdsMember`, `DdsMemberRead`,
/// the `zerodds_dcps::DdsType` impl (for a type without a lifetime) and
/// `idl()`.
fn common_impls(
    generics: &Generics<'_>,
    template: &str,
    kind_value: &syn::Ident,
    nested: bool,
    field_value_body: &Tokens,
    default_body: &Tokens,
) -> Tokens {
    let codec = quote!(::zerodds_codec::codec);
    let private = quote!(::zerodds_codec::__private);
    let self_ty = generics.self_ty();
    // The impls carry bounds on the type parameters only, and the member
    // impl none that reads: a `where` bound on `Self`, or one reading at
    // `'static`, shadows the type's own impls inside the reading ones (the
    // compiler then reads every `DdsMemberRead` at `'static`).
    let member_impl = generics.owned_impl(&quote!(#codec::DdsMember), &[]);
    let read_impl = generics.read_impl(&quote!(#codec::DdsMemberRead<'__de>), &[]);
    let doc_idl = format!(
        "The IDL `{template}` stands for, with every type it reaches: what a C, C++ or other \
         vendor's peer is generated from."
    );
    let idl_impl = {
        let params = generics.params();
        let predicates = generics.predicates(&|ident| quote!(#ident: #codec::DdsMember));
        quote! {
            impl<#(#params),*> #self_ty where #(#predicates,)* {
                #[doc = #doc_idl]
                ///
                /// # Errors
                /// A description that does not compose, with the front end's reason.
                #[allow(dead_code)]
                pub fn idl() -> ::core::result::Result<::std::string::String, ::zerodds_codec::ModelError> {
                    ::zerodds_codec::idl::<Self>()
                }
            }
        }
    };
    // The DCPS impl reads into an owned `T`. A type that borrows from the
    // sample gets it for its `'static` form: the topic type a DataReader and
    // a DataWriter are of. It writes and names the type and its key; it
    // cannot be read from received octets, which `'static` borrows do not
    // outlive: a DataReader lends them (`take_loan`) and
    // `zerodds_codec::LoanExt::read_as` reads the type with its lifetime
    // from the loan. `zerodds_codec::write` writes one of any lifetime.
    let dcps_impl = {
        let borrowing = generics.has_lifetimes();
        let header = if borrowing {
            generics.static_impl(&quote!(#private::dcps::DdsType))
        } else {
            let owned_reads: Vec<Tokens> = generics
                .types
                .iter()
                .map(|ident| quote!(#ident: for<'__de> #codec::DdsMemberRead<'__de>))
                .collect();
            generics.owned_impl(&quote!(#private::dcps::DdsType), &owned_reads)
        };
        let refused = quote! {
            ::core::result::Result::Err(#private::dcps::DecodeError::Invalid {
                what: "a type that borrows from the sample is read from a DataReader's \
                       take_loan with zerodds_codec::LoanExt::read_as",
            })
        };
        let decode = |encapsulation: Tokens| {
            if borrowing {
                quote!({
                    let _ = bytes;
                    #refused
                })
            } else {
                quote!(#private::decode(bytes, ::zerodds_codec::Encapsulation::#encapsulation))
            }
        };
        let decode_le = decode(quote!(Xcdr2Le));
        let decode_be = decode(quote!(Xcdr2Be));
        let decode_xcdr1 = decode(quote!(Xcdr1Le));
        let decode_xcdr1_be = decode(quote!(Xcdr1Be));
        let decode_continuing = if borrowing {
            quote!({
                let _ = (bytes, offset, xcdr1);
                #refused
            })
        } else {
            quote!(#private::decode_continuing(bytes, offset, xcdr1))
        };
        let decode_key_holder = if borrowing {
            quote!({
                let _ = bytes;
                #refused
            })
        } else {
            quote!(#private::decode_key_holder_be(bytes))
        };
        // A type that borrows: its readers learn the instance from the
        // key read lent out of the body.
        let key_items = if borrowing {
            let ident = &generics.input.ident;
            let lent_args: Vec<Tokens> = generics
                .input
                .generics
                .params
                .iter()
                .map(|param| match param {
                    GenericParam::Lifetime(_) => quote!('_),
                    GenericParam::Type(type_param) => {
                        let name = &type_param.ident;
                        quote!(#name)
                    }
                    GenericParam::Const(const_param) => {
                        let name = &const_param.ident;
                        quote!(#name)
                    }
                })
                .collect();
            quote! {
                const BORROWS: bool = true;

                fn key_material_from_body(
                    bytes: &[u8],
                    xcdr1: bool,
                    big_endian: bool,
                ) -> #private::dcps::dds_type::KeyMaterialRead {
                    ::core::option::Option::Some(
                        #private::key_material_from_body::<#ident<#(#lent_args),*>>(
                            bytes, xcdr1, big_endian,
                        ),
                    )
                }
            }
        } else {
            // A type that owns its fields: the key read from the body by
            // the plan, so a reader that admits by key builds no value.
            quote! {
                fn key_material_from_body(
                    bytes: &[u8],
                    xcdr1: bool,
                    big_endian: bool,
                ) -> #private::dcps::dds_type::KeyMaterialRead {
                    #private::key_material_by_plan::<Self>(bytes, xcdr1, big_endian)
                }
            }
        };
        quote! {
            #header {
                const TYPE_NAME: &'static str = #template;
                const EXTENSIBILITY: #private::dcps::Extensibility =
                    #private::dcps::Extensibility::#kind_value;
                const HAS_KEY: bool =
                    #private::has_key(&<Self as #codec::DdsTypeInfo>::LAYOUT);
                const IS_NESTED: bool = #nested;
                #key_items

                fn type_name() -> &'static str {
                    #private::type_name::<Self>()
                }

                fn type_identifier() -> #private::types::TypeIdentifier {
                    #private::type_identifier::<Self>()
                }

                fn type_object() -> ::core::option::Option<#private::types::type_object::TypeObject> {
                    #private::type_object::<Self>()
                }

                fn type_object_dependencies()
                    -> ::std::vec::Vec<#private::types::type_object::TypeObject>
                {
                    #private::type_object_dependencies::<Self>()
                }

                fn type_component() -> ::core::option::Option<#private::types::resolve::TypeComponent> {
                    #private::type_component::<Self>()
                }

                fn key_holder_max_size() -> ::core::option::Option<usize> {
                    #private::key_holder_max_size::<Self>()
                }

                fn encode(
                    &self,
                    out: &mut ::std::vec::Vec<u8>,
                ) -> ::core::result::Result<(), #private::dcps::EncodeError> {
                    #private::encode(self, ::zerodds_codec::Encapsulation::Xcdr2Le, out)
                }

                fn encode_be(
                    &self,
                    out: &mut ::std::vec::Vec<u8>,
                ) -> ::core::result::Result<(), #private::dcps::EncodeError> {
                    #private::encode(self, ::zerodds_codec::Encapsulation::Xcdr2Be, out)
                }

                fn decode(bytes: &[u8]) -> ::core::result::Result<Self, #private::dcps::DecodeError> {
                    #decode_le
                }

                fn decode_be(bytes: &[u8]) -> ::core::result::Result<Self, #private::dcps::DecodeError> {
                    #decode_be
                }

                fn encode_xcdr1(
                    &self,
                    out: &mut ::std::vec::Vec<u8>,
                ) -> ::core::result::Result<(), #private::dcps::EncodeError> {
                    #private::encode(self, ::zerodds_codec::Encapsulation::Xcdr1Le, out)
                }

                fn encode_xcdr1_be(
                    &self,
                    out: &mut ::std::vec::Vec<u8>,
                ) -> ::core::result::Result<(), #private::dcps::EncodeError> {
                    #private::encode(self, ::zerodds_codec::Encapsulation::Xcdr1Be, out)
                }

                fn decode_xcdr1(bytes: &[u8]) -> ::core::result::Result<Self, #private::dcps::DecodeError> {
                    #decode_xcdr1
                }

                fn decode_xcdr1_be(
                    bytes: &[u8],
                ) -> ::core::result::Result<Self, #private::dcps::DecodeError> {
                    #decode_xcdr1_be
                }

                fn encode_continuing(
                    &self,
                    prefix: &mut ::std::vec::Vec<u8>,
                    xcdr1: bool,
                ) -> ::core::result::Result<(), #private::dcps::EncodeError> {
                    #private::encode_continuing(self, prefix, xcdr1)
                }

                fn decode_continuing(
                    bytes: &[u8],
                    offset: usize,
                    xcdr1: bool,
                ) -> ::core::result::Result<Self, #private::dcps::DecodeError> {
                    #decode_continuing
                }

                fn encode_key_holder_be(
                    &self,
                    holder: &mut #private::dcps::dds_type::PlainCdr2BeKeyHolder,
                ) {
                    if <Self as #private::dcps::DdsType>::HAS_KEY {
                        #private::encode_key_holder_be(self, holder);
                    }
                }

                fn decode_key_holder_be(
                    bytes: &[u8],
                ) -> ::core::result::Result<Self, #private::dcps::DecodeError> {
                    #decode_key_holder
                }

                fn field_value(&self, path: &str) -> ::core::option::Option<#private::dcps::FilterValue> {
                    #private::field_value(
                        <Self as #codec::DdsMember>::field_value(self, ::core::option::Option::Some(path)),
                    )
                }

                fn field_unselected(&self, path: &str) -> bool {
                    #private::field_unselected(
                        <Self as #codec::DdsMember>::field_value(self, ::core::option::Option::Some(path)),
                    )
                }
            }
        }
    };
    quote! {
        #member_impl {
            const DESC: #codec::ValueDesc = #codec::ValueDesc::Type {
                desc: <Self as #codec::DdsTypeInfo>::LAYOUT_REF,
            };

            fn serialize_member(
                &self,
                __out: #codec::ValueSerializer<'_>,
            ) -> ::core::result::Result<(), ::zerodds_codec::WireError> {
                __out.nested(self)
            }

            /// XTypes 1.3 Table 9: every member at its default.
            fn default_member() -> ::core::result::Result<Self, ::zerodds_codec::WireError> {
                #default_body
            }

            fn field_value(
                &self,
                __path: ::core::option::Option<&str>,
            ) -> ::core::option::Option<#codec::FieldValue> {
                #field_value_body
            }
        }

        #read_impl {
            fn deserialize_member(
                __input: #codec::ValueDeserializer<'_, '__de>,
            ) -> ::core::result::Result<Self, ::zerodds_codec::WireError> {
                __input.nested()
            }
        }

        #dcps_impl

        #idl_impl
    }
}

#[allow(clippy::too_many_lines)] // one expansion, read top to bottom
fn structure(
    attrs: &TypeAttrs,
    generics: &Generics<'_>,
    template: &str,
    fields: &[&syn::Field],
) -> syn::Result<Tokens> {
    let codec = quote!(::zerodds_codec::codec);

    let mut member_descs = Vec::new();
    let mut member_types = Vec::new();
    let mut checks = Vec::new();
    let mut ser_calls = Vec::new();
    let mut de_steps = Vec::new();
    let mut bindings = Vec::new();
    let mut inits = Vec::new();
    let mut filter_arms = Vec::new();
    let mut withs = Vec::new();
    let mut seen_names: Vec<String> = Vec::new();
    let simple = template
        .rfind("::")
        .map_or(template, |at| &template[at + 2..]);

    for (index, field) in fields.iter().enumerate() {
        let field_ident = field
            .ident
            .as_ref()
            .ok_or_else(|| syn::Error::new(field.span(), "a named field"))?;
        let fa = field_attrs(&field.attrs)?;
        let member_name = idl_name(fa.name.as_ref(), field_ident);
        check_identifier(
            &member_name,
            fa.name.as_ref().map_or(field_ident.span(), LitStr::span),
        )?;
        if member_name.eq_ignore_ascii_case(simple) {
            return Err(syn::Error::new(
                field_ident.span(),
                "a member named like its struct, which IDL 4.2 §7.5 refuses: the struct's \
                 name is declared in its own scope",
            ));
        }
        if seen_names
            .iter()
            .any(|seen| seen.eq_ignore_ascii_case(&member_name))
        {
            return Err(syn::Error::new(
                field_ident.span(),
                "two members whose IDL names differ only in case, which IDL 4.2 §7.2.3 refuses",
            ));
        }
        seen_names.push(member_name.clone());
        let ty = &field.ty;
        if fa.optional && !is_option(ty) {
            return Err(syn::Error::new(
                ty.span(),
                "`optional` is on an `Option<T>` field: the `Option` is what makes the member \
                 @optional",
            ));
        }
        if fa.optional && !generics.is_generic() && !generics.has_lifetimes() {
            checks.push(quote! {
                const _: () = ::core::assert!(
                    ::core::matches!(
                        <#ty as #codec::DdsMember>::DESC,
                        #codec::ValueDesc::Optional { .. }
                    ),
                    "`optional` is on a field whose type is not `Option<T>`"
                );
            });
        }
        if fa.key && is_option(ty) {
            return Err(syn::Error::new(
                ty.span(),
                "a @key member cannot be @optional (XTypes 1.3 §7.2.2.4.4.4.8)",
            ));
        }

        let mut desc = quote!(#codec::MemberDesc::new(#member_name));
        if fa.key {
            desc = quote!(#desc.key());
        }
        if let Some(id) = &fa.id {
            desc = quote!(#desc.id(#id));
        }
        match &fa.hashid {
            Some(Some(text)) => desc = quote!(#desc.hash_id(#codec::HashId::Text(#text))),
            Some(None) => desc = quote!(#desc.hash_id(#codec::HashId::Name)),
            None => {}
        }
        if let Some(bound) = &fa.bound {
            desc = quote!(#desc.bound(#bound));
        }
        member_descs.push(desc);

        let binding = format_ident!("__field{}", index);
        withs.push(fa.with.is_some());
        match &fa.with {
            Some(adapter) => {
                member_types.push(quote!(#codec::With<#adapter, #ty>));
                ser_calls.push(quote!(.member_with(&self.#field_ident)?));
                de_steps.push(quote!(let (#binding, __input) = __input.member_with()?;));
                inits.push(quote!(#field_ident: #binding));
                bindings.push(binding.clone());
                filter_arms.push(quote! {
                    #member_name => #codec::with_field_value::<#adapter, #ty>(&self.#field_ident, __rest),
                });
            }
            None => {
                member_types.push(quote!(#ty));
                ser_calls.push(quote!(.member(&self.#field_ident)?));
                de_steps.push(quote!(let (#binding, __input) = __input.member()?;));
                inits.push(quote!(#field_ident: #binding));
                bindings.push(binding.clone());
                filter_arms.push(quote! {
                    #member_name => <#ty as #codec::DdsMember>::field_value(&self.#field_ident, __rest),
                });
            }
        }
    }

    let members_list = member_types
        .iter()
        .rev()
        .fold(quote!(()), |rest, ty| quote!((#ty, #rest)));
    let (kind_ty, kind_value) = extensibility(attrs)?;
    let mutable = kind_value == "Mutable";
    let modifiers = layout_modifiers(attrs)?;
    let (args_fn, generic) = generics.args_items();
    let layout = quote! {
        #codec::TypeDesc::structure::<#members_list, #kind_ty>(#template, __MEMBERS) #modifiers #generic
    };

    let deserialize_body = if mutable {
        // One typed slot per member, filled in the order the stream names
        // them; a `With` member arrives wrapped.
        let pattern = bindings
            .iter()
            .rev()
            .fold(quote!(()), |rest, binding| quote!((#binding, #rest)));
        let unwrapped: Vec<Tokens> = fields
            .iter()
            .zip(&bindings)
            .zip(&withs)
            .map(|((field, binding), with)| {
                let field_ident = &field.ident;
                if *with {
                    quote!(#field_ident: #binding.into_inner())
                } else {
                    quote!(#field_ident: #binding)
                }
            })
            .collect();
        quote! {
            let __slots = __input.deserialize_mutable::<Self>()?;
            let (#pattern, __finish) = __slots.values()?;
            ::core::result::Result::Ok(__finish.with(Self { #(#unwrapped),* }))
        }
    } else {
        quote! {
            let __input = __input.deserialize_struct::<Self>()?;
            #(#de_steps)*
            let __finish = __input.end()?;
            ::core::result::Result::Ok(__finish.with(Self { #(#inits),* }))
        }
    };
    let codec_impls = if attrs.manual {
        quote!()
    } else {
        let serialize_header = generics.owned_impl(&quote!(#codec::DdsSerialize), &[]);
        let deserialize_header = generics.read_impl(&quote!(#codec::DdsDeserialize<'__de>), &[]);
        quote! {
            #serialize_header {
                fn serialize(
                    &self,
                    __out: #codec::DdsSerializer<'_>,
                ) -> ::core::result::Result<#codec::Serialized, ::zerodds_codec::WireError> {
                    __out.serialize_struct::<Self>()?
                        #(#ser_calls)*
                        .end()
                }
            }

            #deserialize_header {
                fn deserialize(
                    __input: #codec::DdsDeserializer<'_, '__de>,
                ) -> ::core::result::Result<#codec::Decoded<Self>, ::zerodds_codec::WireError> {
                    #deserialize_body
                }
            }
        }
    };
    let field_value_body = quote! {
        let __path = __path?;
        let (__head, __rest) = match __path.split_once('.') {
            ::core::option::Option::Some((__head, __rest)) => {
                (__head, ::core::option::Option::Some(__rest))
            }
            ::core::option::Option::None => (__path, ::core::option::Option::None),
        };
        match __head {
            #(#filter_arms)*
            _ => ::core::option::Option::None,
        }
    };
    let defaults: Vec<Tokens> = fields
        .iter()
        .zip(&member_types)
        .zip(&withs)
        .map(|((field, member_ty), with)| {
            let field_ident = &field.ident;
            if *with {
                quote!(#field_ident: <#member_ty as #codec::DdsMember>::default_member()?.into_inner())
            } else {
                quote!(#field_ident: <#member_ty as #codec::DdsMember>::default_member()?)
            }
        })
        .collect();
    let default_body = quote!(::core::result::Result::Ok(Self { #(#defaults),* }));
    let info_header = generics.owned_impl(&quote!(#codec::DdsTypeInfo), &[]);
    let common = common_impls(
        generics,
        template,
        &kind_value,
        attrs.nested,
        &field_value_body,
        &default_body,
    );

    Ok(quote! {
        const _: () = {
            const __MEMBERS: &[#codec::MemberDesc] = &[#(#member_descs),*];

            #args_fn

            #(#checks)*

            #info_header {
                type Members = #members_list;
                type Extensibility = #kind_ty;
                const LAYOUT: #codec::TypeDesc = #layout;
            }

            #codec_impls

            #common
        };
    })
}

/// The type-level index of branch `index`: `Here`, `There<Here>`, ….
fn branch_index(index: usize) -> Tokens {
    let codec = quote!(::zerodds_codec::codec);
    let mut out = quote!(#codec::Here);
    for _ in 0..index {
        out = quote!(#codec::There<#out>);
    }
    out
}

/// The shape of one variant of a union that holds a member.
enum Shape<'a> {
    /// `V(T)`: the branch member.
    Member(&'a Type),
    /// `V(D, T)`: the discriminator it was read with, then the member.
    Carried(&'a Type),
}

#[allow(clippy::too_many_lines)] // one expansion, read top to bottom
fn union(
    input: &DeriveInput,
    attrs: &TypeAttrs,
    generics: &Generics<'_>,
    template: &str,
    data: &syn::DataEnum,
) -> syn::Result<Tokens> {
    let codec = quote!(::zerodds_codec::codec);
    let discriminator: Type = attrs
        .discriminator
        .clone()
        .unwrap_or_else(|| syn::parse_quote!(i32));
    let simple = template
        .rfind("::")
        .map_or(template, |at| &template[at + 2..]);

    let mut cases = Vec::new();
    let mut label_fns = Vec::new();
    let mut branch_types = Vec::new();
    let mut serialize_arms = Vec::new();
    let mut deserialize_arms = Vec::new();
    let mut field_arms = Vec::new();
    let mut branch_names = Vec::new();
    let mut no_member_arm = None;
    // Variants without a member, each one discriminator value:
    // `#[dds(case = label)] V`.
    let mut unit_variants: Vec<(syn::Ident, syn::Expr)> = Vec::new();
    let mut seen_names: Vec<String> = Vec::new();
    let mut defaults = 0usize;
    // XTypes 1.3 Table 9 for a union: the discriminator's default, and the
    // member it selects at its own default.
    let mut default_by_label = Vec::new();
    let mut default_case = None;

    for variant in &data.variants {
        let va = variant_attrs(&variant.attrs)?;
        let variant_ident = &variant.ident;
        if let Some(value) = &va.value {
            return Err(syn::Error::new(
                value.span(),
                "`value` is for an enumerator; a union branch takes `case`",
            ));
        }
        let fields: Vec<&syn::Field> = match &variant.fields {
            Fields::Unnamed(fields) => fields.unnamed.iter().collect(),
            // A discriminator value that selects no member (IDL 4.2
            // §7.4.1.4.4.4): it goes out alone, and the union's type
            // declares no branch for it.
            Fields::Unit => {
                let labels = va.labels.clone().unwrap_or_default();
                let extra = va.default
                    || va.no_member
                    || va.id.is_some()
                    || va.hashid.is_some()
                    || va.name.is_some()
                    || va.bound.is_some();
                let [label] = labels.as_slice() else {
                    return Err(syn::Error::new(
                        variant.span(),
                        "a variant without a member is one discriminator value: \
                         `#[dds(case = label)] V`; for several, `#[dds(no_member)] V(D)`; a branch \
                         with a member is `V(T)` or `V(D, T)`",
                    ));
                };
                if extra {
                    return Err(syn::Error::new(
                        variant.span(),
                        "a variant without a member takes `case` only: it declares no branch",
                    ));
                }
                unit_variants.push((variant_ident.clone(), label.clone()));
                serialize_arms.push(quote! {
                    Self::#variant_ident => {
                        let __d: #discriminator = #label;
                        __union.no_member(&__d)
                    }
                });
                field_arms.push(quote! {
                    Self::#variant_ident => {
                        if __head == "#discriminator" {
                            let __d: #discriminator = #label;
                            return match __rest {
                                ::core::option::Option::None => #codec::DdsMember::field_value(&__d, ::core::option::Option::None),
                                ::core::option::Option::Some(_) => ::core::option::Option::None,
                            };
                        }
                    }
                });
                default_by_label.push(quote! {
                    if <#discriminator as #codec::Discriminator>::label(&(#label)) == __label {
                        return ::core::result::Result::Ok(Self::#variant_ident);
                    }
                });
                continue;
            }
            Fields::Named(fields) => {
                return Err(syn::Error::new(
                    fields.span(),
                    "a union branch is `V(T)` or `V(D, T)`: an IDL branch holds one member, and \
                     a struct of several is a derived struct as `T`",
                ));
            }
        };
        if va.no_member {
            if fields.len() != 1 || va.labels.is_some() || va.default {
                return Err(syn::Error::new(
                    variant.span(),
                    "`no_member` is on a variant `V(D)` without `case` or `default`",
                ));
            }
            if no_member_arm.is_some() {
                return Err(syn::Error::new(variant.span(), "one `no_member` variant"));
            }
            no_member_arm = Some(variant_ident.clone());
            field_arms.push(quote! {
                Self::#variant_ident(__d) => {
                    if __head == "#discriminator" {
                        return match __rest {
                            ::core::option::Option::None => #codec::DdsMember::field_value(__d, ::core::option::Option::None),
                            ::core::option::Option::Some(_) => ::core::option::Option::None,
                        };
                    }
                }
            });
            continue;
        }
        let shape = match fields.as_slice() {
            [member] => Shape::Member(&member.ty),
            [_, member] => Shape::Carried(&member.ty),
            _ => {
                return Err(syn::Error::new(
                    variant.fields.span(),
                    "a union branch is `V(T)` or `V(D, T)`",
                ));
            }
        };
        let branch_name = idl_name(va.name.as_ref(), variant_ident);
        check_identifier(
            &branch_name,
            va.name.as_ref().map_or(variant_ident.span(), LitStr::span),
        )?;
        if branch_name.eq_ignore_ascii_case(simple) {
            return Err(syn::Error::new(
                variant_ident.span(),
                "a branch named like its union, which IDL 4.2 §7.5 refuses",
            ));
        }
        if seen_names
            .iter()
            .any(|seen| seen.eq_ignore_ascii_case(&branch_name))
        {
            return Err(syn::Error::new(
                variant_ident.span(),
                "two branches whose IDL names differ only in case, which IDL 4.2 §7.2.3 refuses",
            ));
        }
        seen_names.push(branch_name.clone());
        branch_names.push(branch_name.clone());
        if va.default {
            defaults += 1;
        }

        let index = branch_types.len();
        let label_fn = format_ident!("__labels{}", index);
        // The labels: the written ones, or one past the highest so far.
        let explicit = va.labels.clone().unwrap_or_default();
        let label_body = if !explicit.is_empty() {
            let values: Vec<Tokens> = explicit
                .iter()
                .map(|label| quote!(<#discriminator as #codec::Discriminator>::label(&(#label))))
                .collect();
            quote!(::std::vec![#(#values),*])
        } else if va.default {
            quote!(::std::vec::Vec::new())
        } else if index == 0 {
            quote!(::std::vec![0])
        } else {
            let earlier: Vec<syn::Ident> =
                (0..index).map(|k| format_ident!("__labels{}", k)).collect();
            quote! {
                let __earlier: ::std::vec::Vec<i128> =
                    [#(#earlier()),*].into_iter().flatten().collect();
                ::std::vec![__earlier.into_iter().max().map_or(0, |__m| __m + 1)]
            }
        };
        label_fns.push(quote! {
            fn #label_fn() -> ::std::vec::Vec<i128> { #label_body }
        });
        let several = explicit.len() > 1 || va.default;
        let member_ty = match shape {
            Shape::Member(ty) => {
                if several {
                    return Err(syn::Error::new(
                        variant.fields.span(),
                        "a case with several labels, or the `default:` case, holds the \
                         discriminator it was read with: `V(D, T)`",
                    ));
                }
                ty
            }
            Shape::Carried(ty) => ty,
        };
        branch_types.push(member_ty.clone());

        let mut case = quote!(#codec::CaseDesc::new(#branch_name, #label_fn));
        if va.default {
            case = quote!(#case.default_case());
        }
        if let Some(id) = &va.id {
            case = quote!(#case.id(#id));
        }
        match &va.hashid {
            Some(Some(text)) => case = quote!(#case.hash_id(#codec::HashId::Text(#text))),
            Some(None) => case = quote!(#case.hash_id(#codec::HashId::Name)),
            None => {}
        }
        if let Some(bound) = &va.bound {
            case = quote!(#case.bound(#bound));
        }
        cases.push(case);

        let member_default = quote!(<#member_ty as #codec::DdsMember>::default_member()?);
        let constructed = match shape {
            Shape::Carried(_) => quote!(Self::#variant_ident(__d, #member_default)),
            Shape::Member(_) => quote!(Self::#variant_ident(#member_default)),
        };
        if va.default {
            default_case = Some(constructed);
        } else {
            default_by_label.push(quote! {
                if #label_fn().contains(&__label) {
                    return ::core::result::Result::Ok(#constructed);
                }
            });
        }

        let index_ty = branch_index(index);
        // DDS-XTypes 1.4 Beta 1 §7.6.7.1: a branch by its name, the
        // discriminator by `#discriminator`.
        let field_discriminator = match (&shape, explicit.first()) {
            (Shape::Carried(_), _) => quote!(let __d: &#discriminator = __d;),
            (Shape::Member(_), Some(label)) => {
                quote!(let __d: &#discriminator = &(#label);)
            }
            (Shape::Member(_), None) => quote! {
                let __owned: #discriminator = #label_fn()
                    .first()
                    .copied()
                    .and_then(<#discriminator as #codec::Discriminator>::from_label)?;
                let __d: &#discriminator = &__owned;
            },
        };
        let field_pattern = match shape {
            Shape::Carried(_) => quote!(Self::#variant_ident(__d, __v)),
            Shape::Member(_) => quote!(Self::#variant_ident(__v)),
        };
        field_arms.push(quote! {
            #field_pattern => {
                if __head == "#discriminator" {
                    #field_discriminator
                    return match __rest {
                        ::core::option::Option::None => #codec::DdsMember::field_value(__d, ::core::option::Option::None),
                        ::core::option::Option::Some(_) => ::core::option::Option::None,
                    };
                }
                if __head == #branch_name {
                    return #codec::DdsMember::field_value(__v, __rest);
                }
            }
        });
        match shape {
            Shape::Carried(_) => {
                serialize_arms.push(quote! {
                    Self::#variant_ident(__d, __v) => __union.branch::<#index_ty>(__d, __v),
                });
                deserialize_arms.push(quote! {
                    ::core::option::Option::Some(#index) => {
                        let (__d, __v, __finish) = __input.branch::<#index_ty>()?;
                        ::core::result::Result::Ok(__finish.with(Self::#variant_ident(__d, __v)))
                    }
                });
            }
            Shape::Member(_) => {
                // One label: a written one is an expression of the
                // discriminator type, checked by the compiler; a counted
                // one is converted from its value.
                let discriminator_value = match explicit.first() {
                    Some(label) => quote!(let __d: #discriminator = #label;),
                    None => quote! {
                        let __d: #discriminator = #label_fn()
                            .first()
                            .copied()
                            .and_then(<#discriminator as #codec::Discriminator>::from_label)
                            .ok_or_else(|| ::zerodds_codec::WireError::refused(
                                0,
                                "a counted label the discriminator type holds",
                            ))?;
                    },
                };
                serialize_arms.push(quote! {
                    Self::#variant_ident(__v) => {
                        #discriminator_value
                        __union.branch::<#index_ty>(&__d, __v)
                    }
                });
                deserialize_arms.push(quote! {
                    ::core::option::Option::Some(#index) => {
                        let (_, __v, __finish) = __input.branch::<#index_ty>()?;
                        ::core::result::Result::Ok(__finish.with(Self::#variant_ident(__v)))
                    }
                });
            }
        }
    }
    if defaults > 1 {
        return Err(syn::Error::new(
            input.ident.span(),
            "one `default` case, as IDL 4.2 §7.4.1.4.4.4 allows",
        ));
    }
    // The `no_member` variant holds the values no unit variant names: one
    // that a unit variant names would be read back as that variant.
    if let Some(variant_ident) = &no_member_arm {
        let unit_labels: Vec<Tokens> = unit_variants
            .iter()
            .map(|(_, label)| quote!(<#discriminator as #codec::Discriminator>::label(&(#label))))
            .collect();
        serialize_arms.push(if unit_labels.is_empty() {
            quote!(Self::#variant_ident(__d) => __union.no_member(__d),)
        } else {
            quote! {
            Self::#variant_ident(__d) => {
                let __label = <#discriminator as #codec::Discriminator>::label(__d);
                if [#(#unit_labels),*].contains(&__label) {
                    return ::core::result::Result::Err(::zerodds_codec::WireError::refused(
                        0,
                        "the variant without a member that names this discriminator",
                    ));
                }
                __union.no_member(__d)
            }
            }
        });
    }
    if defaults == 1 && !unit_variants.is_empty() {
        return Err(syn::Error::new(
            input.ident.span(),
            "a variant without a member and a `default` case exclude each other: the \
             `default:` case takes every label no other case takes (IDL 4.2 §7.4.1.4.4.4)",
        ));
    }
    if branch_types.is_empty() {
        return Err(syn::Error::new(
            input.ident.span(),
            "an IDL union has at least one case",
        ));
    }

    let members_list = branch_types
        .iter()
        .rev()
        .fold(quote!(()), |rest, ty| quote!((#ty, #rest)));
    let (kind_ty, kind_value) = extensibility(attrs)?;
    let modifiers = layout_modifiers(attrs)?;
    let (args_fn, generic) = generics.args_items();
    // DDS-XTypes 1.3 §7.2.2.4.4.4.8: "for union types, only the
    // discriminator member can have the “is key” attribute set to true".
    let keyed = if attrs.key.is_some() {
        quote!(.key_discriminator())
    } else {
        quote!()
    };
    let layout = quote! {
        #codec::TypeDesc::union::<#members_list, #kind_ty, #discriminator>(#template, __CASES)
            #modifiers #keyed #generic
    };
    // A discriminator that selects no member: the variant of its value,
    // else the `no_member` variant, else refused.
    let unit_reads: Vec<Tokens> = unit_variants
        .iter()
        .map(|(variant_ident, label)| {
            quote! {
                if __label == <#discriminator as #codec::Discriminator>::label(&(#label)) {
                    return ::core::result::Result::Ok(__finish.with(Self::#variant_ident));
                }
            }
        })
        .collect();
    let no_member_read = match (&no_member_arm, unit_variants.is_empty()) {
        (Some(variant_ident), _) => quote! {
            ::core::option::Option::None => {
                let (__d, __finish) = __input.no_member()?;
                let __label = <#discriminator as #codec::Discriminator>::label(&__d);
                #(#unit_reads)*
                ::core::result::Result::Ok(__finish.with(Self::#variant_ident(__d)))
            }
        },
        (None, false) => quote! {
            ::core::option::Option::None => {
                let (__d, __finish) = __input.no_member()?;
                let __label = <#discriminator as #codec::Discriminator>::label(&__d);
                #(#unit_reads)*
                let _ = __finish;
                ::core::result::Result::Err(::zerodds_codec::WireError::refused(
                    0,
                    "a discriminator without a member that the Rust type holds",
                ))
            }
        },
        (None, true) => quote! {
            ::core::option::Option::None => ::core::result::Result::Err(
                ::zerodds_codec::WireError::refused(
                    0,
                    "a discriminator that selects a branch: the Rust type holds no \
                     discriminator without a member",
                ),
            ),
        },
    };
    let codec_impls = if attrs.manual {
        quote!()
    } else {
        let serialize_header = generics.owned_impl(&quote!(#codec::DdsSerialize), &[]);
        let deserialize_header = generics.read_impl(&quote!(#codec::DdsDeserialize<'__de>), &[]);
        quote! {
            #serialize_header {
                fn serialize(
                    &self,
                    __out: #codec::DdsSerializer<'_>,
                ) -> ::core::result::Result<#codec::Serialized, ::zerodds_codec::WireError> {
                    let __union = __out.serialize_union::<Self>()?;
                    match self {
                        #(#serialize_arms)*
                    }
                }
            }

            #deserialize_header {
                fn deserialize(
                    __input: #codec::DdsDeserializer<'_, '__de>,
                ) -> ::core::result::Result<#codec::Decoded<Self>, ::zerodds_codec::WireError> {
                    let __input = __input.deserialize_union::<Self>()?;
                    match __input.selected() {
                        #(#deserialize_arms)*
                        #no_member_read
                        ::core::option::Option::Some(_) => ::core::result::Result::Err(
                            ::zerodds_codec::WireError::refused(0, "a branch the type declares"),
                        ),
                    }
                }
            }
        }
    };
    let info_header = generics.owned_impl(&quote!(#codec::DdsTypeInfo), &[]);
    let union_header = generics.owned_impl(&quote!(#codec::DdsUnionInfo), &[]);
    // DDS-XTypes 1.4 Beta 1 §7.6.7.1: a branch the discriminator does not
    // select is `Unselected`, and the filter that names it is false.
    let field_value_body = quote! {
        let __path = __path?;
        let (__head, __rest) = match __path.split_once('.') {
            ::core::option::Option::Some((__head, __rest)) => {
                (__head, ::core::option::Option::Some(__rest))
            }
            ::core::option::Option::None => (__path, ::core::option::Option::None),
        };
        match self {
            #(#field_arms)*
        }
        if [#(#branch_names),*].contains(&__head) {
            ::core::option::Option::Some(#codec::FieldValue::Unselected)
        } else {
            ::core::option::Option::None
        }
    };
    let fallback = match (&default_case, &no_member_arm) {
        (Some(constructed), _) => quote!(::core::result::Result::Ok(#constructed)),
        (None, Some(variant_ident)) => {
            quote!(::core::result::Result::Ok(Self::#variant_ident(__d)))
        }
        (None, None) => quote! {
            ::core::result::Result::Err(::zerodds_codec::WireError::refused(
                0,
                "a union whose discriminator's default selects a branch",
            ))
        },
    };
    let default_body = quote! {
        let __d = <#discriminator as #codec::DdsMember>::default_member()?;
        let __label = <#discriminator as #codec::Discriminator>::label(&__d);
        #(#default_by_label)*
        #fallback
    };
    let common = common_impls(
        generics,
        template,
        &kind_value,
        attrs.nested,
        &field_value_body,
        &default_body,
    );

    Ok(quote! {
        const _: () = {
            #(#label_fns)*

            const __CASES: &[#codec::CaseDesc] = &[#(#cases),*];

            #args_fn

            #info_header {
                type Members = #members_list;
                type Extensibility = #kind_ty;
                const LAYOUT: #codec::TypeDesc = #layout;
            }

            #union_header {
                type Discriminator = #discriminator;
            }

            #codec_impls

            #common
        };
    })
}

fn enumeration(
    input: &DeriveInput,
    attrs: &TypeAttrs,
    name: &str,
    data: &syn::DataEnum,
) -> syn::Result<Tokens> {
    for (what, set) in [
        ("autoid", attrs.autoid.is_some()),
        ("nested", attrs.nested),
        ("codec", attrs.manual),
    ] {
        if set {
            return Err(syn::Error::new(
                input.ident.span(),
                format!(
                    "`{what}` is for a struct or a union; an enum takes `name`, `extensibility` \
                     and `bit_bound`"
                ),
            ));
        }
    }
    // DDS-XTypes 1.3 §7.2.3, Table 12: "Enumerated Types: These types may
    // be final or appendable on a type-by-type basis." Unwritten, the IDL
    // default; the TypeObject carries the kind in `enum_flags`, and Table 18
    // assigns enumerations of one kind only.
    let enum_extensibility = match attrs.extensibility.as_ref() {
        None => None,
        Some((kind, _)) if kind == "final" => Some(quote!(Final)),
        Some((kind, _)) if kind == "appendable" => Some(quote!(Appendable)),
        Some((_, span)) => {
            return Err(syn::Error::new(
                *span,
                "an enum is \"final\" or \"appendable\" (DDS-XTypes 1.3 §7.2.3, Table 12)",
            ));
        }
    };
    if data.variants.is_empty() {
        return Err(syn::Error::new(
            input.ident.span(),
            "an IDL enum has at least one enumerator",
        ));
    }
    let ident = &input.ident;
    let codec = quote!(::zerodds_codec::codec);
    let mut enumerators = Vec::new();
    let mut to_index = Vec::new();
    let mut from_index = Vec::new();
    let mut to_label = Vec::new();
    let mut from_label = Vec::new();
    let mut to_name = Vec::new();
    let mut first = None;
    // The wire value of each enumerator: its `@value`, else one past the
    // one before, the first 0.
    let mut next: i128 = 0;
    for (index, variant) in data.variants.iter().enumerate() {
        let va = variant_attrs(&variant.attrs)?;
        let enumerator = idl_name(va.name.as_ref(), &variant.ident);
        check_identifier(
            &enumerator,
            va.name.as_ref().map_or(variant.ident.span(), LitStr::span),
        )?;
        if va.labels.is_some() || va.default || va.no_member || va.id.is_some() {
            return Err(syn::Error::new(
                variant.span(),
                "`case`, `default`, `no_member` and `id` are for a union branch",
            ));
        }
        let value = match (&va.value, &variant.discriminant) {
            (Some(value), _) => Some(value.base10_parse::<i32>()?),
            (
                None,
                Some((
                    _,
                    Expr::Lit(ExprLit {
                        lit: Lit::Int(value),
                        ..
                    }),
                )),
            ) => Some(value.base10_parse::<i32>()?),
            (None, Some((_, other))) => {
                return Err(syn::Error::new(
                    other.span(),
                    "an integer literal, the enumerator's IDL @value; or `#[dds(value = N)]`",
                ));
            }
            (None, None) => None,
        };
        let wire = value.map_or(next, i128::from);
        next = wire + 1;
        let value = match value {
            Some(value) => quote!(::core::option::Option::Some(#value)),
            None => quote!(::core::option::Option::None),
        };
        enumerators.push(quote! {
            #codec::EnumeratorDesc { name: #enumerator, value: #value }
        });
        let variant_ident = &variant.ident;
        to_index.push(quote!(Self::#variant_ident => #index,));
        from_index.push(quote!(#index => ::core::result::Result::Ok(Self::#variant_ident),));
        to_label.push(quote!(Self::#variant_ident => #wire,));
        to_name.push(quote!(Self::#variant_ident => #enumerator,));
        from_label.push(quote!(#wire => ::core::option::Option::Some(Self::#variant_ident),));
        if first.is_none() {
            first = Some(variant_ident.clone());
        }
    }
    let mut layout = quote!(#codec::TypeDesc::enumeration(#name, __ENUMERATORS));
    if let Some(bits) = &attrs.bit_bound {
        let bits: u16 = bits.base10_parse()?;
        if bits == 0 || bits > 32 {
            return Err(syn::Error::new(
                attrs.bit_bound.span(),
                "an enum's @bit_bound is 1 to 32 (IDL 4.2 §8.3.1.2)",
            ));
        }
        layout = quote!(#layout.bit_bound(#bits));
    }
    if let Some(kind) = enum_extensibility {
        layout = quote!(#layout.enum_extensibility(#codec::Extensibility::#kind));
    }
    Ok(quote! {
        const _: () = {
            const __ENUMERATORS: &[#codec::EnumeratorDesc] = &[#(#enumerators),*];
            const __LAYOUT: #codec::TypeDesc = #layout;

            impl #codec::DdsMember for #ident {
                const DESC: #codec::ValueDesc = #codec::ValueDesc::Type { desc: &__LAYOUT };

                fn serialize_member(
                    &self,
                    __out: #codec::ValueSerializer<'_>,
                ) -> ::core::result::Result<(), ::zerodds_codec::WireError> {
                    __out.enumeration(match self { #(#to_index)* })
                }

                /// XTypes 1.3 Table 9: the first enumerator.
                fn default_member() -> ::core::result::Result<Self, ::zerodds_codec::WireError> {
                    ::core::result::Result::Ok(Self::#first)
                }

                /// DDS 1.4 Annex B.2 ENUMERATEDVALUE: an enumerated member
                /// compares with the name of its enumerator.
                fn field_value(
                    &self,
                    __path: ::core::option::Option<&str>,
                ) -> ::core::option::Option<#codec::FieldValue> {
                    match __path {
                        ::core::option::Option::None => ::core::option::Option::Some(
                            #codec::FieldValue::Text(::std::string::String::from(
                                match self { #(#to_name)* },
                            )),
                        ),
                        ::core::option::Option::Some(_) => ::core::option::Option::None,
                    }
                }
            }

            impl<'__de> #codec::DdsMemberRead<'__de> for #ident {
                fn deserialize_member(
                    __input: #codec::ValueDeserializer<'_, '__de>,
                ) -> ::core::result::Result<Self, ::zerodds_codec::WireError> {
                    let __at = __input.position();
                    match __input.enumeration()? {
                        #(#from_index)*
                        _ => ::core::result::Result::Err(::zerodds_codec::WireError::refused(
                            __at,
                            "a value that names an enumerator",
                        )),
                    }
                }
            }

            /// As a union's discriminator: the enumerator's wire value.
            impl #codec::Discriminator for #ident {
                fn label(&self) -> i128 {
                    match self { #(#to_label)* }
                }

                fn from_label(__label: i128) -> ::core::option::Option<Self> {
                    match __label {
                        #(#from_label)*
                        _ => ::core::option::Option::None,
                    }
                }
            }
        };
    })
}

// --- derive(WireRepr) ------------------------------------------------

/// The IDL types `#[wire(idl = …)]` takes, with the Rust type that holds
/// each.
fn idl_holder(idl: &str) -> Option<Tokens> {
    Some(match idl {
        "boolean" => quote!(bool),
        "octet" | "uint8" => quote!(u8),
        "int8" => quote!(i8),
        "short" | "int16" => quote!(i16),
        "unsigned short" | "uint16" => quote!(u16),
        "long" | "int32" => quote!(i32),
        "unsigned long" | "uint32" => quote!(u32),
        "long long" | "int64" => quote!(i64),
        "unsigned long long" | "uint64" => quote!(u64),
        "float" => quote!(f32),
        "double" => quote!(f64),
        "char" => quote!(char),
        _ => return None,
    })
}

fn wire_repr(input: &DeriveInput) -> syn::Result<Tokens> {
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new(
            input.generics.span(),
            "derive(WireRepr) does not take a generic type in this version",
        ));
    }
    let mut idl: Option<LitStr> = None;
    for attr in &input.attrs {
        if attr.path().is_ident("wire") {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("idl") {
                    idl = Some(meta.value()?.parse()?);
                    Ok(())
                } else {
                    Err(meta.error("unknown `wire` attribute; known: idl"))
                }
            })?;
        }
    }
    let idl = idl.ok_or_else(|| {
        syn::Error::new(
            input.ident.span(),
            "derive(WireRepr) needs `#[wire(idl = \"string\")]` or an IDL primitive \
             (`\"uint32\"`, `\"double\"`, …)",
        )
    })?;
    let ident = &input.ident;
    let wire = quote!(::zerodds_codec);
    let text = idl.value();
    let body = if text == "string" {
        quote! {
            impl #wire::WireRepr for #ident {
                type Idl = str;

                fn to_wire(
                    &self,
                    __out: #wire::IdlOut<'_, str>,
                ) -> ::core::result::Result<#wire::Written, #wire::WireError> {
                    __out.display(self)
                }

                fn from_wire(__wire: &str) -> ::core::result::Result<Self, #wire::WireError> {
                    <Self as ::core::convert::TryFrom<&str>>::try_from(__wire).map_err(|__error| {
                        #wire::WireError::invalid_value(::std::string::ToString::to_string(&__error))
                    })
                }
            }
        }
    } else if let Some(holder) = idl_holder(&text) {
        quote! {
            impl #wire::WireRepr for #ident {
                type Idl = #holder;

                fn to_wire(
                    &self,
                    __out: #wire::IdlOut<'_, #holder>,
                ) -> ::core::result::Result<#wire::Written, #wire::WireError> {
                    __out.value(::core::convert::Into::<#holder>::into(
                        ::core::clone::Clone::clone(self),
                    ))
                }

                fn from_wire(__wire: &#holder) -> ::core::result::Result<Self, #wire::WireError> {
                    <Self as ::core::convert::TryFrom<#holder>>::try_from(*__wire).map_err(|__error| {
                        #wire::WireError::invalid_value(::std::string::ToString::to_string(&__error))
                    })
                }
            }
        }
    } else {
        return Err(syn::Error::new(
            idl.span(),
            "`idl` is \"string\" or an IDL primitive: boolean, octet, char, int8, uint8, short \
             (int16), unsigned short (uint16), long (int32), unsigned long (uint32), long long \
             (int64), unsigned long long (uint64), float, double",
        ));
    };
    Ok(quote! {
        #body
        ::zerodds_codec::__private::wire::dds_member_via_wire_repr!(#ident);
    })
}
