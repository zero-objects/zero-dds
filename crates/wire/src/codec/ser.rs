// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The serializer: a [`PlanSet`] executed against the values an impl hands
//! over.
//!
//! A key holder's members are held until all are there, because the
//! holder's order need not be the declaration's (XTypes 1.3 §7.6.8); the
//! values held are of the members' different Rust types, so `Pending`
//! holds them as `dyn Erased`. A list typed per member would have to be
//! as long as the struct, in the type of the serializer.
// zerodds-lint: allow no_dyn_in_safe

use alloc::vec::Vec;
use core::marker::PhantomData;

use super::{
    DdsMember, DdsSerialize, DdsTypeInfo, DdsUnionInfo, Discriminator, Header, PlanSet, Primitive,
    Root, Select, ValuePlan,
};
use crate::repr::{IdlOut, IdlPrimitive};
use crate::{ParameterRole, WireError, Writer, Written};

/// Proof that a struct's members went out in full: only
/// [`StructSerializer::end`] makes one, and [`DdsSerialize::serialize`]
/// returns it.
#[derive(Debug)]
pub struct Serialized {
    _private: (),
}

/// Which plan a serializer is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Target {
    /// A struct's data plan, by index.
    Struct(usize),
    /// A union's plan, by index: its data plan, or the one of a union
    /// inside a key holder.
    Union(usize),
    /// A struct's key holder, by index (XTypes 1.3 §7.6.8).
    Holder(usize),
    /// The key holder of a type that has none.
    NoKey,
}

impl Target {
    /// The target of a set's root.
    pub(crate) const fn root(plans: &PlanSet) -> Self {
        match plans.root {
            Root::Struct(index) => Self::Struct(index),
            Root::Union(index) => Self::Union(index),
        }
    }

    /// The target of a set's key holder.
    pub(crate) const fn key(plans: &PlanSet) -> Self {
        match plans.key {
            Some(super::KeyPlan {
                root: Root::Struct(index),
                ..
            }) => Self::Holder(index),
            Some(super::KeyPlan {
                root: Root::Union(index),
                ..
            }) => Self::Union(index),
            None => Self::NoKey,
        }
    }
}

/// The serializer of one struct value, for one encapsulation.
#[derive(Debug)]
pub struct DdsSerializer<'a> {
    w: &'a mut Writer,
    plans: &'a PlanSet,
    target: Target,
}

impl<'a> DdsSerializer<'a> {
    /// The serializer of a sample whose type `plans` was built for.
    ///
    /// `writer` carries the encapsulation's byte order and alignment cap
    /// ([`PlanSet::max_align`]).
    pub const fn new(writer: &'a mut Writer, plans: &'a PlanSet) -> Self {
        Self {
            w: writer,
            plans,
            target: Target::root(plans),
        }
    }

    /// The serializer of the type's key holder (XTypes 1.3 §7.6.8): the
    /// key members, in the holder's order, and nothing else; of a union
    /// keyed by its discriminator, the discriminator.
    ///
    /// `writer` is big-endian with [`super::KeyPlan::max_align`].
    pub const fn key_holder(writer: &'a mut Writer, plans: &'a PlanSet) -> Self {
        Self {
            w: writer,
            plans,
            target: Target::key(plans),
        }
    }

    /// Begin the members of a `T`.
    ///
    /// # Errors
    /// A plan that is not `T`'s, or a key holder of a type with no key.
    pub fn serialize_struct<'v, T: DdsTypeInfo>(
        self,
    ) -> Result<StructSerializer<'a, 'v, T::Members>, WireError> {
        let () = T::CHECK;
        let mut dheader = None;
        match self.target {
            Target::Union(_) => {
                return Err(WireError::refused(
                    self.w.position(),
                    "a struct where the plan has a union",
                ));
            }
            Target::Struct(index) => {
                let plan = self.plans.structs.get(index).ok_or_else(no_plan)?;
                if plan.name != T::LAYOUT.scoped_name() {
                    return Err(WireError::refused(
                        self.w.position(),
                        "the plan of the type being serialized",
                    ));
                }
                if plan.needs_dheader {
                    dheader = Some(self.w.write_dheader());
                }
            }
            Target::Holder(index) => {
                let plan = self.plans.holders.get(index).ok_or_else(no_plan)?;
                if plan.name != T::LAYOUT.scoped_name() {
                    return Err(WireError::refused(
                        self.w.position(),
                        "the plan of the type being serialized",
                    ));
                }
            }
            Target::NoKey => return Err(no_key(self.w.position())),
        }
        Ok(StructSerializer {
            w: self.w,
            plans: self.plans,
            target: self.target,
            next: 0,
            dheader,
            pending: Vec::new(),
            members: PhantomData,
        })
    }

    /// Begin a `T` that is a union: the DHEADER where the plan has one.
    /// [`UnionSerializer::branch`] writes the discriminator and the
    /// branch.
    ///
    /// # Errors
    /// A plan that is not `T`'s, or the key holder of a union that is not
    /// keyed by its discriminator.
    pub fn serialize_union<T: DdsUnionInfo>(
        self,
    ) -> Result<UnionSerializer<'a, T::Members, T::Discriminator>, WireError> {
        let () = T::CHECK;
        let () = T::IS_UNION;
        let index = match self.target {
            Target::Union(index) => index,
            Target::NoKey => return Err(no_key(self.w.position())),
            Target::Struct(_) | Target::Holder(_) => {
                return Err(WireError::refused(
                    self.w.position(),
                    "a union where the plan has a struct",
                ));
            }
        };
        let plan = self.plans.unions.get(index).ok_or_else(no_plan)?;
        if plan.name != T::LAYOUT.scoped_name() {
            return Err(WireError::refused(
                self.w.position(),
                "the plan of the type being serialized",
            ));
        }
        let dheader = plan.needs_dheader.then(|| self.w.write_dheader());
        Ok(UnionSerializer {
            w: self.w,
            plans: self.plans,
            index,
            dheader,
            branches: PhantomData,
        })
    }
}

/// A union value about to be written: `M` lists the branches' Rust types,
/// `D` is the discriminator's.
pub struct UnionSerializer<'a, M, D> {
    w: &'a mut Writer,
    plans: &'a PlanSet,
    index: usize,
    dheader: Option<crate::writer::DHeader>,
    branches: PhantomData<fn() -> (M, D)>,
}

impl<M, D> core::fmt::Debug for UnionSerializer<'_, M, D> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("UnionSerializer")
            .field("index", &self.index)
            .finish_non_exhaustive()
    }
}

impl<M, D: Discriminator> UnionSerializer<'_, M, D> {
    /// The discriminator `discriminator`, then the value of the branch at
    /// the type-level index `I` ([`super::B0`], [`super::B1`], …): `value`
    /// has exactly that branch's type.
    ///
    /// # Errors
    /// A discriminator that selects another branch than `I` (a label of
    /// another case, or — for the `default:` case — a label of any
    /// case), and what the value or the writer refuses.
    pub fn branch<I>(
        self,
        discriminator: &D,
        value: &<M as Select<I>>::Item,
    ) -> Result<Serialized, WireError>
    where
        M: Select<I>,
    {
        let plan = &self.plans.unions[self.index];
        if plan.select(discriminator.label()) != Some(<M as Select<I>>::INDEX) {
            return Err(WireError::refused(
                self.w.position(),
                "a discriminator that selects the branch being written",
            ));
        }
        let branch = plan
            .branches
            .get(<M as Select<I>>::INDEX)
            .ok_or_else(no_plan)?;
        write_frame(self.w, self.plans, &plan.discriminator, discriminator)?;
        // The key holder of a union keyed by its discriminator holds the
        // discriminator alone (XTypes 1.3 §7.6.8 step 1).
        if !plan.discriminator_only {
            write_frame(self.w, self.plans, &branch.frame, value)?;
        }
        self.close()
    }

    /// The discriminator `discriminator` alone: a value no case and no
    /// `default:` takes, which leaves the member absent (IDL 4.2
    /// §7.4.1.4.4.4).
    ///
    /// # Errors
    /// A discriminator some branch takes, and what the writer refuses.
    pub fn no_member(self, discriminator: &D) -> Result<Serialized, WireError> {
        let plan = &self.plans.unions[self.index];
        if plan.select(discriminator.label()).is_some() {
            return Err(WireError::refused(
                self.w.position(),
                "a discriminator that no case and no default takes, for a union without a member",
            ));
        }
        write_frame(self.w, self.plans, &plan.discriminator, discriminator)?;
        self.close()
    }

    fn close(self) -> Result<Serialized, WireError> {
        let plan = &self.plans.unions[self.index];
        if plan.needs_sentinel {
            self.w.write_sentinel();
        }
        if let Some(header) = self.dheader {
            self.w.try_finish_dheader(header)?;
        }
        Ok(Serialized { _private: () })
    }
}

/// The members of one struct value still to come, as their Rust types:
/// `M` is the rest of [`DdsTypeInfo::Members`].
pub struct StructSerializer<'a, 'v, M> {
    w: &'a mut Writer,
    plans: &'a PlanSet,
    target: Target,
    next: usize,
    dheader: Option<crate::writer::DHeader>,
    /// The key holder's members, held until all are there: the holder's
    /// order need not be the declaration's.
    pending: Vec<Pending<'v>>,
    members: PhantomData<fn() -> M>,
}

impl<M> core::fmt::Debug for StructSerializer<'_, '_, M> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("StructSerializer")
            .field("target", &self.target)
            .field("next", &self.next)
            .finish_non_exhaustive()
    }
}

impl<'a, 'v, M> StructSerializer<'a, 'v, M> {
    /// Write or hold the next member and step to the rest of the list.
    fn push<N>(mut self, value: Pending<'v>) -> Result<StructSerializer<'a, 'v, N>, WireError> {
        match self.target {
            Target::Struct(index) => {
                let plan = &self.plans.structs[index];
                let frame = plan.members.get(self.next).ok_or_else(no_plan)?;
                write_frame(self.w, self.plans, frame, value.erased())?;
            }
            Target::Holder(_) => self.pending.push(value),
            Target::Union(_) | Target::NoKey => return Err(no_plan()),
        }
        Ok(StructSerializer {
            w: self.w,
            plans: self.plans,
            target: self.target,
            next: self.next + 1,
            dheader: self.dheader,
            pending: self.pending,
            members: PhantomData,
        })
    }
}

impl<'a, 'v, H: DdsMember + 'v, T> StructSerializer<'a, 'v, (H, T)> {
    /// The next member's value.
    ///
    /// # Errors
    /// What the value or the writer refuses.
    pub fn member(self, value: &'v H) -> Result<StructSerializer<'a, 'v, T>, WireError> {
        self.push(Pending::Ref(value))
    }
}

impl<'a, 'v, A, V, T> StructSerializer<'a, 'v, (super::With<A, V>, T)>
where
    A: crate::WireReprWith<V> + Default + 'v,
    A::Idl: super::members::IdlType,
    V: 'v,
{
    /// The next member's value, through the adapter `A` its
    /// [`super::With`] names.
    ///
    /// # Errors
    /// What the value, the adapter or the writer refuses.
    pub fn member_with(self, value: &'v V) -> Result<StructSerializer<'a, 'v, T>, WireError> {
        self.push(Pending::Owned(alloc::boxed::Box::new(
            super::members::WithRef::<A, V>::new(value),
        )))
    }
}

/// A member value the serializer holds until it is written.
enum Pending<'v> {
    Ref(&'v dyn Erased),
    Owned(alloc::boxed::Box<dyn Erased + 'v>),
}

impl Pending<'_> {
    fn erased(&self) -> &dyn Erased {
        match self {
            Self::Ref(value) => *value,
            Self::Owned(value) => value.as_ref(),
        }
    }
}

impl StructSerializer<'_, '_, ()> {
    /// Close the struct: the PL_CDR1 sentinel and the DHEADER's length
    /// where the plan has them, or the key holder in its order.
    ///
    /// # Errors
    /// An object a DHEADER cannot count, and what a key member refuses.
    pub fn end(self) -> Result<Serialized, WireError> {
        match self.target {
            Target::Struct(index) => {
                let plan = &self.plans.structs[index];
                if plan.needs_sentinel {
                    self.w.write_sentinel();
                }
                if let Some(header) = self.dheader {
                    self.w.try_finish_dheader(header)?;
                }
            }
            Target::Holder(holder) => {
                let plan = self.plans.holders.get(holder).ok_or_else(no_plan)?;
                for (index, frame) in &plan.members {
                    let value = self.pending.get(*index).ok_or_else(no_plan)?;
                    write_frame(self.w, self.plans, frame, value.erased())?;
                }
            }
            Target::Union(_) | Target::NoKey => return Err(no_plan()),
        }
        Ok(Serialized { _private: () })
    }
}

/// A member's framing, then its value.
fn write_frame(
    w: &mut Writer,
    plans: &PlanSet,
    frame: &super::MemberFrame,
    value: &dyn Erased,
) -> Result<(), WireError> {
    let present = value.erased_present();
    let plan = &frame.value;
    match frame.header {
        Header::None => {
            if !present {
                return Err(WireError::refused(
                    w.position(),
                    "a value: the member is not @optional",
                ));
            }
            value.erased_serialize(ValueSerializer::new(w, plans, plan))
        }
        // Rule (20): the BOOLEAN `<is_present>`, then the value if it is.
        Header::OptionalFlag => {
            w.write_bool(present);
            if present {
                value.erased_serialize(ValueSerializer::new(w, plans, plan))?;
            }
            Ok(())
        }
        Header::Pid {
            pid,
            extended,
            must_understand,
            role,
        } => {
            // Rule (19): an `@optional` member of a type that is not
            // `@mutable` has its header either way, and a declared length
            // of zero is its absence. A `@mutable` type leaves an absent
            // member out (§7.4.1.2.3).
            if role == ParameterRole::Member && !present {
                return Ok(());
            }
            // The short form carries the 16-bit parameter id, the extended
            // one the member id in 28 bits (§7.4.1.2.2).
            let opened = if extended {
                w.write_pid_extended(frame.member_id, role, must_understand)?
            } else {
                w.write_pid(pid, role, must_understand)?
            };
            if present {
                value.erased_serialize(ValueSerializer::new(w, plans, plan))?;
            }
            w.finish_pid(opened);
            Ok(())
        }
        // §7.4.3.4.2: the member id, the plan's length code and the
        // must-understand flag; an absent `@optional` member of a
        // `@mutable` type is left out.
        Header::EmHeader {
            length_code,
            must_understand,
        } => {
            if !present {
                return Ok(());
            }
            let opened = w.write_emheader(frame.member_id, length_code, must_understand)?;
            value.erased_serialize(ValueSerializer::new(w, plans, plan))?;
            w.finish_emheader(opened)
        }
    }
}

fn no_plan() -> WireError {
    WireError::refused(0, "a layout plan for every member the type declares")
}

/// The key holder of a type that is not keyed.
pub(crate) fn no_key(position: usize) -> WireError {
    WireError::refused(
        position,
        "a keyed type (a @key member, or a union keyed by its discriminator), for its key holder",
    )
}

/// [`DdsMember`] behind a pointer, for the key holder's reordering.
pub(crate) trait Erased {
    fn erased_serialize(&self, out: ValueSerializer<'_>) -> Result<(), WireError>;
    fn erased_present(&self) -> bool;
}

impl<T: DdsMember> Erased for T {
    fn erased_serialize(&self, out: ValueSerializer<'_>) -> Result<(), WireError> {
        self.serialize_member(out)
    }

    fn erased_present(&self) -> bool {
        self.is_present()
    }
}

/// Where one value goes, and the plan it goes out under.
pub struct ValueSerializer<'a> {
    w: &'a mut Writer,
    plans: &'a PlanSet,
    plan: &'a ValuePlan,
    /// How many of an array's dimensions the enclosing calls have taken.
    dims: usize,
}

impl core::fmt::Debug for ValueSerializer<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ValueSerializer")
            .field("plan", self.plan)
            .field("dims", &self.dims)
            .finish_non_exhaustive()
    }
}

impl<'a> ValueSerializer<'a> {
    pub(crate) const fn new(w: &'a mut Writer, plans: &'a PlanSet, plan: &'a ValuePlan) -> Self {
        Self {
            w,
            plans,
            plan,
            dims: 0,
        }
    }

    fn mismatch(&self, what: &str) -> WireError {
        WireError::OutOfRange {
            at: self.w.position(),
            expected: alloc::format!("{what}, which the plan has where it has {:?}", self.plan),
        }
    }

    fn require(&self, kinds: &[Primitive]) -> Result<(), WireError> {
        match self.plan {
            ValuePlan::Primitive(kind) if kinds.contains(kind) => Ok(()),
            _ => Err(self.mismatch("a primitive of the member's Rust type")),
        }
    }

    /// `boolean`.
    ///
    /// # Errors
    /// A plan that has another type here.
    pub fn boolean(self, value: bool) -> Result<(), WireError> {
        self.require(&[Primitive::Boolean])?;
        self.w.write_bool(value);
        Ok(())
    }

    /// `octet` or `uint8`.
    ///
    /// # Errors
    /// A plan that has another type here.
    pub fn uint8(self, value: u8) -> Result<(), WireError> {
        self.require(&[Primitive::Octet, Primitive::UInt8])?;
        self.w.write_u8(value);
        Ok(())
    }

    /// `int8`.
    ///
    /// # Errors
    /// A plan that has another type here.
    pub fn int8(self, value: i8) -> Result<(), WireError> {
        self.require(&[Primitive::Int8])?;
        self.w.write_i8(value);
        Ok(())
    }

    /// `short`.
    ///
    /// # Errors
    /// A plan that has another type here.
    pub fn int16(self, value: i16) -> Result<(), WireError> {
        self.require(&[Primitive::Int16])?;
        self.w.write_i16(value);
        Ok(())
    }

    /// `unsigned short`.
    ///
    /// # Errors
    /// A plan that has another type here.
    pub fn uint16(self, value: u16) -> Result<(), WireError> {
        self.require(&[Primitive::UInt16])?;
        self.w.write_u16(value);
        Ok(())
    }

    /// `long`.
    ///
    /// # Errors
    /// A plan that has another type here.
    pub fn int32(self, value: i32) -> Result<(), WireError> {
        self.require(&[Primitive::Int32])?;
        self.w.write_i32(value);
        Ok(())
    }

    /// `unsigned long`.
    ///
    /// # Errors
    /// A plan that has another type here.
    pub fn uint32(self, value: u32) -> Result<(), WireError> {
        self.require(&[Primitive::UInt32])?;
        self.w.write_u32(value);
        Ok(())
    }

    /// `long long`.
    ///
    /// # Errors
    /// A plan that has another type here.
    pub fn int64(self, value: i64) -> Result<(), WireError> {
        self.require(&[Primitive::Int64])?;
        self.w.write_i64(value);
        Ok(())
    }

    /// `unsigned long long`.
    ///
    /// # Errors
    /// A plan that has another type here.
    pub fn uint64(self, value: u64) -> Result<(), WireError> {
        self.require(&[Primitive::UInt64])?;
        self.w.write_u64(value);
        Ok(())
    }

    /// `float`.
    ///
    /// # Errors
    /// A plan that has another type here.
    pub fn float32(self, value: f32) -> Result<(), WireError> {
        self.require(&[Primitive::Float32])?;
        self.w.write_f32(value);
        Ok(())
    }

    /// `double`.
    ///
    /// # Errors
    /// A plan that has another type here.
    pub fn float64(self, value: f64) -> Result<(), WireError> {
        self.require(&[Primitive::Float64])?;
        self.w.write_f64(value);
        Ok(())
    }

    /// `char`.
    ///
    /// # Errors
    /// A plan that has another type here, and a character above U+00FF.
    pub fn character(self, value: char) -> Result<(), WireError> {
        self.require(&[Primitive::Char])?;
        self.w.write_char(value)
    }

    /// A `string`, with the bound and the NUL check of
    /// [`IdlOut::str`].
    ///
    /// # Errors
    /// A plan that has another type here, a text over the bound, or one
    /// with a NUL inside.
    pub fn string(self, value: &str) -> Result<(), WireError> {
        self.idl_str(|out| out.str(value))
    }

    /// A `string` a [`crate::WireRepr`] type writes through its
    /// [`IdlOut`].
    ///
    /// # Errors
    /// A plan that has another type here, and what `write` refuses.
    pub fn idl_str(
        self,
        write: impl FnOnce(IdlOut<'_, str>) -> Result<Written, WireError>,
    ) -> Result<(), WireError> {
        let ValuePlan::String { bound } = *self.plan else {
            return Err(self.mismatch("a string"));
        };
        let out = match bound {
            Some(bound) => IdlOut::bounded_string(self.w, bound),
            None => IdlOut::string(self.w),
        };
        write(out).map(drop)
    }

    /// A primitive a [`crate::WireRepr`] type writes through its
    /// [`IdlOut`].
    ///
    /// # Errors
    /// A plan that has another type here, and what `write` refuses.
    pub fn idl_primitive<P: IdlPrimitive>(
        self,
        write: impl FnOnce(IdlOut<'_, P>) -> Result<Written, WireError>,
    ) -> Result<(), WireError> {
        match self.plan {
            ValuePlan::Primitive(kind) if P::stands_for(*kind) => {}
            _ => return Err(self.mismatch("the primitive of the member's IDL type")),
        }
        write(IdlOut::over(self.w, None)).map(drop)
    }

    /// A `sequence`, its bound checked before an octet is written.
    ///
    /// # Errors
    /// A plan that has another type here, a sequence over its bound, and
    /// what an element refuses.
    pub fn sequence<T: DdsMember>(self, items: &[T]) -> Result<(), WireError> {
        let ValuePlan::Sequence {
            ref element,
            bound,
            delimited,
        } = *self.plan
        else {
            return Err(self.mismatch("a sequence"));
        };
        let plans = self.plans;
        let element: &ValuePlan = element;
        let write = |w: &mut Writer, value: &T| {
            value.serialize_member(ValueSerializer::new(w, plans, element))
        };
        match (bound, delimited) {
            (Some(bound), true) => self.w.write_delimited_sequence_bounded(items, bound, write),
            (Some(bound), false) => self.w.write_sequence_bounded(items, bound, write),
            (None, true) => self.w.write_delimited_sequence(items, write),
            (None, false) => self.w.write_sequence(items, write),
        }
    }

    /// One dimension of an array; an inner dimension is the element's.
    ///
    /// # Errors
    /// A plan that has another type or length here, and what an element
    /// refuses.
    pub fn array<T: DdsMember>(self, items: &[T]) -> Result<(), WireError> {
        let ValuePlan::Array {
            ref element,
            ref dims,
            delimited,
        } = *self.plan
        else {
            return Err(self.mismatch("an array"));
        };
        if dims.get(self.dims) != Some(&items.len()) {
            return Err(self.mismatch("an array of the member's length"));
        }
        let plans = self.plans;
        let whole = self.plan;
        let depth = self.dims + 1;
        let inner = depth < dims.len();
        let element: &ValuePlan = element;
        let write = |w: &mut Writer, value: &T| {
            let out = if inner {
                ValueSerializer {
                    w,
                    plans,
                    plan: whole,
                    dims: depth,
                }
            } else {
                ValueSerializer::new(w, plans, element)
            };
            value.serialize_member(out)
        };
        // One delimiter around the whole array, rule (9), and none around
        // an inner dimension: a multi-dimensional array is one run (G4).
        if delimited && self.dims == 0 {
            self.w.write_delimited_array(items, write)
        } else {
            self.w.write_array(items, write)
        }
    }

    /// An enumeration, by the index of its enumerator in declaration
    /// order; the plan has the wire value and the holder.
    ///
    /// # Errors
    /// A plan that has another type here, or an index past the
    /// enumerators.
    pub fn enumeration(self, index: usize) -> Result<(), WireError> {
        let ValuePlan::Enum {
            holder_bits,
            ref values,
        } = *self.plan
        else {
            return Err(self.mismatch("an enumeration"));
        };
        let Some(value) = values.get(index).copied() else {
            return Err(self.mismatch("an enumerator the type declares"));
        };
        let at = self.w.position();
        let refuse = |_| WireError::OutOfRange {
            at,
            expected: alloc::format!("an enumerator value its {holder_bits}-bit holder carries"),
        };
        // The holder the bit bound calls for, as the generated codec
        // writes it (`enum_holder` in `zerodds-lang-rules`).
        match holder_bits {
            0..=8 => self.w.write_u8(u8::try_from(value).map_err(refuse)?),
            9..=16 => self.w.write_u16(u16::try_from(value).map_err(refuse)?),
            17..=32 => self.w.write_u32(u32::try_from(value).map_err(refuse)?),
            _ => self.w.write_u64(u64::try_from(value).map_err(refuse)?),
        }
        Ok(())
    }

    /// A struct member: the value's own [`DdsSerialize`] under the member
    /// type's plan.
    ///
    /// # Errors
    /// A plan that has another type here, and what the value refuses.
    pub fn nested<T: DdsSerialize>(self, value: &T) -> Result<(), WireError> {
        let target = match *self.plan {
            ValuePlan::Struct(index) => Target::Struct(index),
            ValuePlan::Union(index) => Target::Union(index),
            ValuePlan::Holder(index) => Target::Holder(index),
            _ => return Err(self.mismatch("a struct or a union")),
        };
        value
            .serialize(DdsSerializer {
                w: self.w,
                plans: self.plans,
                target,
            })
            .map(drop)
    }

    /// A `sequence<octet>` from a byte slice, its bound checked before an
    /// octet is written.
    ///
    /// # Errors
    /// A plan that has another type here, and a slice over the bound.
    pub fn octets(self, bytes: &[u8]) -> Result<(), WireError> {
        self.sequence(bytes)
    }
}
