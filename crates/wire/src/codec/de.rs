// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The deserializer: a [`PlanSet`] executed against the received bytes.

use alloc::string::String;
use alloc::vec::Vec;
use core::marker::PhantomData;

use super::ser::Target;
use super::{
    DdsDeserialize, DdsMember, DdsMemberRead, DdsTypeInfo, DdsUnionInfo, Discriminator, Header,
    InOrder, MemberList, MemberListRead, Mutable, PlanSet, Primitive, Select, StructPlan, TypeKind,
    ValuePlan,
};
use crate::repr::IdlPrimitive;
use crate::{Narrowed, ParameterRole, Reader, TryConstruct, WireError};

/// A value that came out of a [`DdsDeserializer`] whole: only
/// [`Finish::with`] makes one.
#[derive(Debug)]
pub struct Decoded<T>(T);

impl<T> Decoded<T> {
    /// The value.
    #[must_use]
    pub fn into_inner(self) -> T {
        self.0
    }
}

/// Proof that a struct's members came off the stream in full.
#[derive(Debug)]
pub struct Finish {
    _private: (),
}

impl Finish {
    /// The value the members make.
    #[must_use]
    pub const fn with<T>(self, value: T) -> Decoded<T> {
        Decoded(value)
    }
}

/// Where the values come from.
enum Source<'r, 'de> {
    /// The received bytes.
    Reader(&'r mut Reader<'de>),
    /// Every member's default: the value of a struct a sample does not
    /// carry.
    Defaults,
}

/// The deserializer of one struct value.
pub struct DdsDeserializer<'r, 'de> {
    source: Source<'r, 'de>,
    plans: Option<&'r PlanSet>,
    target: Target,
}

impl core::fmt::Debug for DdsDeserializer<'_, '_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("DdsDeserializer")
            .field("target", &self.target)
            .finish_non_exhaustive()
    }
}

impl<'r, 'de> DdsDeserializer<'r, 'de> {
    /// The deserializer of a sample whose type `plans` was built for.
    ///
    /// `reader` carries the encapsulation's byte order and alignment cap.
    pub const fn new(reader: &'r mut Reader<'de>, plans: &'r PlanSet) -> Self {
        Self {
            source: Source::Reader(reader),
            plans: Some(plans),
            target: Target::root(plans),
        }
    }

    /// The deserializer of a key holder (XTypes 1.3 §7.6.8): the key
    /// members from the holder, every other member at its default; of a
    /// union keyed by its discriminator, the discriminator from the
    /// holder and the branch it selects at its default.
    pub const fn key_holder(reader: &'r mut Reader<'de>, plans: &'r PlanSet) -> Self {
        Self {
            source: Source::Reader(reader),
            plans: Some(plans),
            target: Target::key(plans),
        }
    }

    /// The deserializer that hands out every member's default.
    #[must_use]
    pub const fn defaults() -> Self {
        Self {
            source: Source::Defaults,
            plans: None,
            target: Target::Struct(0),
        }
    }

    /// The members of a struct's key holder, read into their slots.
    fn key_slots<T: DdsTypeInfo>(
        reader: &mut Reader<'de>,
        plans: &'r PlanSet,
        holder: usize,
    ) -> Result<<T::Members as MemberList>::Slots, WireError>
    where
        T::Members: MemberListRead<'de>,
    {
        let holder = plans.holders.get(holder).ok_or_else(no_plan)?;
        if holder.name != T::LAYOUT.scoped_name() {
            return Err(WireError::refused(
                reader.position(),
                "the plan of the type being deserialized",
            ));
        }
        let mut slots = <T::Members as MemberList>::empty_slots();
        for (index, frame) in &holder.members {
            <T::Members as MemberListRead<'de>>::read_frame_slot(
                &mut slots, *index, reader, plans, frame,
            )?;
        }
        Ok(slots)
    }

    /// The members of a `@final` or `@appendable` `T`, in declaration
    /// order.
    ///
    /// # Errors
    /// A plan that is not `T`'s, and what the stream refuses.
    pub fn deserialize_struct<T: DdsTypeInfo>(
        self,
    ) -> Result<StructDeserializer<'r, 'de, T::Members>, WireError>
    where
        T::Extensibility: InOrder,
        T::Members: MemberListRead<'de>,
    {
        let () = T::CHECK;
        let mut state = State {
            plan: None,
            end: None,
            slots: None,
            next: 0,
        };
        let plans = self.plans;
        let source = match (self.source, self.target) {
            (Source::Defaults, _) => Source::Defaults,
            (Source::Reader(reader), Target::Struct(index)) => {
                let plan = plan_of::<T>(plans, index)?;
                if plan.needs_dheader {
                    let length = reader.read_dheader()?;
                    state.end = Some(reader.position() + length);
                }
                state.plan = Some(plan);
                Source::Reader(reader)
            }
            (Source::Reader(reader), Target::Holder(holder)) => {
                let slots = Self::key_slots::<T>(reader, plans.ok_or_else(no_plan)?, holder)?;
                state.slots = Some(slots);
                Source::Reader(reader)
            }
            (Source::Reader(reader), Target::NoKey) => {
                return Err(super::ser::no_key(reader.position()));
            }
            (Source::Reader(reader), Target::Union(_)) => {
                return Err(WireError::refused(
                    reader.position(),
                    "a struct where the plan has a union",
                ));
            }
        };
        Ok(StructDeserializer {
            source,
            plans,
            state,
        })
    }

    /// The members of a `@mutable` `T`, one typed slot per member id,
    /// filled from the headers the stream carries in whatever order it
    /// carries them (XTypes 1.3 §7.4.1.2.3, §7.4.3.4.2).
    ///
    /// # Errors
    /// A plan that is not `T`'s, a second header for one member, a
    /// must-understand member `T` does not declare, and what the stream
    /// refuses.
    pub fn deserialize_mutable<T: DdsTypeInfo<Extensibility = Mutable>>(
        self,
    ) -> Result<MutableSlots<T::Members>, WireError>
    where
        T::Members: MemberListRead<'de>,
    {
        let () = T::CHECK;
        let plans = self.plans;
        let slots = match (self.source, self.target) {
            (Source::Defaults, _) => T::Members::empty_slots(),
            (Source::Reader(reader), Target::Holder(holder)) => {
                Self::key_slots::<T>(reader, plans.ok_or_else(no_plan)?, holder)?
            }
            (Source::Reader(reader), Target::NoKey) => {
                return Err(super::ser::no_key(reader.position()));
            }
            (Source::Reader(reader), Target::Struct(index)) => {
                let plan = plan_of::<T>(plans, index)?;
                let plans = plans.ok_or_else(no_plan)?;
                if plan.needs_sentinel {
                    parameter_list::<T::Members>(reader, plans, plan)?
                } else {
                    member_headers::<T::Members>(reader, plans, plan)?
                }
            }
            (Source::Reader(reader), Target::Union(_)) => {
                return Err(WireError::refused(
                    reader.position(),
                    "a struct where the plan has a union",
                ));
            }
        };
        Ok(MutableSlots {
            slots,
            members: PhantomData,
        })
    }

    /// A `T` that is a union: its framing and its discriminator, read;
    /// [`UnionDeserializer::selected`] says which branch it selects, and
    /// [`UnionDeserializer::branch`] reads that branch.
    ///
    /// # Errors
    /// A plan that is not `T`'s, a discriminator header with another
    /// member id than 0, and what the stream refuses.
    pub fn deserialize_union<T: DdsUnionInfo>(
        self,
    ) -> Result<UnionDeserializer<'r, 'de, T::Members, T::Discriminator>, WireError>
    where
        T::Discriminator: DdsMemberRead<'de>,
    {
        let () = T::CHECK;
        let () = T::IS_UNION;
        match (self.source, self.target) {
            // Every member at its default (XTypes 1.3 Table 9): the
            // discriminator's default, and the branch it selects at its
            // own default.
            (Source::Defaults, _) => {
                let discriminator = <T::Discriminator as DdsMember>::default_member()?;
                let selected = select_by_description(&T::LAYOUT, discriminator.label());
                Ok(UnionDeserializer {
                    source: Source::Defaults,
                    plans: None,
                    index: 0,
                    end: None,
                    discriminator,
                    selected,
                    branches: PhantomData,
                })
            }
            (Source::Reader(reader), Target::Union(index)) => {
                let plans = self.plans.ok_or_else(no_plan)?;
                let plan = plans.unions.get(index).ok_or_else(no_plan)?;
                if plan.name != T::LAYOUT.scoped_name() {
                    return Err(WireError::refused(
                        reader.position(),
                        "the plan of the type being deserialized",
                    ));
                }
                let end = if plan.needs_dheader {
                    let length = reader.read_dheader()?;
                    Some(reader.position() + length)
                } else {
                    None
                };
                let discriminator: T::Discriminator =
                    read_union_frame(reader, plans, &plan.discriminator)?;
                let selected = plan.select(discriminator.label());
                // The key holder of a union keyed by its discriminator
                // ends here (XTypes 1.3 §7.6.8 step 1); the branch it
                // selects takes its default, as a struct's members outside
                // its holder do.
                if plan.discriminator_only {
                    return Ok(UnionDeserializer {
                        source: Source::Defaults,
                        plans: None,
                        index,
                        end: None,
                        discriminator,
                        selected,
                        branches: PhantomData,
                    });
                }
                Ok(UnionDeserializer {
                    source: Source::Reader(reader),
                    plans: Some(plans),
                    index,
                    end,
                    discriminator,
                    selected,
                    branches: PhantomData,
                })
            }
            (Source::Reader(reader), Target::NoKey) => Err(super::ser::no_key(reader.position())),
            (Source::Reader(reader), _) => Err(WireError::refused(
                reader.position(),
                "a union where the plan has a struct",
            )),
        }
    }
}

/// The branch `label` selects by the description's labels, for a union
/// read from its defaults (no plan).
fn select_by_description(desc: &super::TypeDesc, label: i128) -> Option<usize> {
    let TypeKind::Union { cases, .. } = desc.kind else {
        return None;
    };
    cases
        .iter()
        .position(|case| (case.labels)().contains(&label))
        .or_else(|| cases.iter().position(|case| case.default))
}

/// A union's discriminator or branch, read in order behind the header the
/// plan gives it: none, a `@mutable` union's EMHEADER (XTypes 1.3
/// §7.4.3.5.3 rule (28)) or its PL_CDR1 parameter header (rule (27)), each
/// checked against the frame's member id.
fn read_union_frame<'de, H: DdsMemberRead<'de>>(
    reader: &mut Reader<'de>,
    plans: &PlanSet,
    frame: &super::MemberFrame,
) -> Result<H, WireError> {
    let plan = &frame.value;
    match frame.header {
        Header::EmHeader { .. } => {
            let header = reader.read_emheader()?;
            if header.member_id != frame.member_id {
                return Err(WireError::refused(
                    reader.position(),
                    "the member id of the union member this position holds",
                ));
            }
            let ends = reader.position() + header.extent;
            let value = H::deserialize_member(ValueDeserializer::new(reader, plans, plan))?;
            if reader.position() != ends {
                return Err(WireError::refused(
                    reader.position(),
                    "a member filling the extent its EMHEADER declared",
                ));
            }
            Ok(value)
        }
        Header::Pid { role, .. } => {
            let pid = reader.read_pid_as(role)?;
            if !pid.is_user_member() || pid.pid != frame.member_id {
                return Err(WireError::refused(
                    reader.position(),
                    "the member id of the union member this position holds",
                ));
            }
            let value = H::deserialize_member(ValueDeserializer::new(reader, plans, plan))?;
            reader.finish_pid(pid)?;
            Ok(value)
        }
        Header::None | Header::OptionalFlag => {
            H::deserialize_member(ValueDeserializer::new(reader, plans, plan))
        }
    }
}

/// A union whose discriminator is read: `M` lists the branches' Rust
/// types, `D` is the discriminator's.
pub struct UnionDeserializer<'r, 'de, M, D> {
    source: Source<'r, 'de>,
    plans: Option<&'r PlanSet>,
    index: usize,
    /// Where an `@appendable` or `@mutable` union ends (its DHEADER).
    end: Option<usize>,
    discriminator: D,
    selected: Option<usize>,
    branches: PhantomData<fn() -> M>,
}

impl<M, D: core::fmt::Debug> core::fmt::Debug for UnionDeserializer<'_, '_, M, D> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("UnionDeserializer")
            .field("discriminator", &self.discriminator)
            .field("selected", &self.selected)
            .finish_non_exhaustive()
    }
}

impl<'r, 'de, M, D: Discriminator> UnionDeserializer<'r, 'de, M, D> {
    /// The discriminator read.
    #[must_use]
    pub const fn discriminator(&self) -> &D {
        &self.discriminator
    }

    /// The branch the discriminator selects, by its position in the
    /// union's declaration; `None` for a discriminator no case and no
    /// `default:` takes.
    #[must_use]
    pub const fn selected(&self) -> Option<usize> {
        self.selected
    }

    /// The discriminator and the value of the branch at the type-level
    /// index `I`, which must be the selected one.
    ///
    /// # Errors
    /// Another branch selected, and what the stream or the branch's type
    /// refuses.
    #[allow(clippy::type_complexity)] // the three parts a union value is made of
    pub fn branch<I>(mut self) -> Result<(D, <M as Select<I>>::Item, Finish), WireError>
    where
        M: Select<I>,
        <M as Select<I>>::Item: DdsMemberRead<'de>,
    {
        if self.selected != Some(<M as Select<I>>::INDEX) {
            return Err(WireError::refused(
                self.position(),
                "the branch the discriminator selects",
            ));
        }
        let value = match &mut self.source {
            Source::Defaults => <<M as Select<I>>::Item as DdsMember>::default_member()?,
            Source::Reader(reader) => {
                let plans = self.plans.ok_or_else(no_plan)?;
                let plan = plans.unions.get(self.index).ok_or_else(no_plan)?;
                let branch = plan
                    .branches
                    .get(<M as Select<I>>::INDEX)
                    .ok_or_else(no_plan)?;
                read_union_frame(reader, plans, &branch.frame)?
            }
        };
        let (discriminator, finish) = self.close()?;
        Ok((discriminator, value, finish))
    }

    /// The discriminator of a union that selects no member, and the close.
    ///
    /// # Errors
    /// A discriminator that selects a branch.
    pub fn no_member(self) -> Result<(D, Finish), WireError> {
        if self.selected.is_some() {
            return Err(WireError::refused(
                self.position(),
                "a discriminator that selects no member",
            ));
        }
        self.close()
    }

    fn position(&self) -> usize {
        match &self.source {
            Source::Reader(reader) => reader.position(),
            Source::Defaults => 0,
        }
    }

    /// The PL_CDR1 sentinel where the plan has one, and past the DHEADER's
    /// extent, where a newer writer may have put what this type does not
    /// declare.
    fn close(self) -> Result<(D, Finish), WireError> {
        if let (Source::Reader(reader), Some(plans)) = (self.source, self.plans) {
            let plan = plans.unions.get(self.index).ok_or_else(no_plan)?;
            if plan.needs_sentinel {
                let pid = reader.read_pid()?;
                if !pid.end {
                    return Err(WireError::refused(
                        reader.position(),
                        "the PID_SENTINEL that closes a union's parameter list",
                    ));
                }
            }
            if let Some(end) = self.end {
                reader.skip_to(end)?;
            }
        }
        Ok((self.discriminator, Finish { _private: () }))
    }
}

/// The plan of a `T`, by its index.
fn plan_of<T: DdsTypeInfo>(
    plans: Option<&PlanSet>,
    index: usize,
) -> Result<&StructPlan, WireError> {
    let plan = plans
        .and_then(|plans| plans.structs.get(index))
        .ok_or_else(no_plan)?;
    if plan.name != T::LAYOUT.scoped_name() {
        return Err(WireError::refused(
            0,
            "the plan of the type being deserialized",
        ));
    }
    Ok(plan)
}

/// Rule (21): the members a DHEADER's extent holds, each behind its
/// EMHEADER.
fn member_headers<'de, M: MemberListRead<'de>>(
    reader: &mut Reader<'de>,
    plans: &PlanSet,
    plan: &StructPlan,
) -> Result<M::Slots, WireError> {
    let mut slots = M::empty_slots();
    let length = reader.read_dheader()?;
    let end = reader.position() + length;
    while reader.position() < end {
        let header = reader.read_emheader()?;
        let ends = reader.position() + header.extent;
        match plan
            .members
            .iter()
            .position(|frame| frame.member_id == header.member_id)
        {
            Some(index) => {
                M::read_slot(
                    &mut slots,
                    index,
                    ValueDeserializer::new(reader, plans, &plan.members[index].value),
                )?;
                if reader.position() != ends {
                    return Err(WireError::refused(
                        reader.position(),
                        "a member filling the extent its EMHEADER declared",
                    ));
                }
            }
            None if header.must_understand => {
                return Err(WireError::refused(
                    reader.position(),
                    "a must-understand member this type does not declare, per §7.2.2.4.4.4.6",
                ));
            }
            None => reader.skip_to(ends)?,
        }
    }
    reader.skip_to(end)?;
    Ok(slots)
}

/// Rule (23): the members of a PL_CDR1 parameter list, up to its
/// sentinel.
fn parameter_list<'de, M: MemberListRead<'de>>(
    reader: &mut Reader<'de>,
    plans: &PlanSet,
    plan: &StructPlan,
) -> Result<M::Slots, WireError> {
    let mut slots = M::empty_slots();
    loop {
        let pid = reader.read_pid()?;
        if pid.end {
            return Ok(slots);
        }
        if pid.ignore {
            reader.skip_parameter(pid)?;
            continue;
        }
        let index = if pid.is_user_member() {
            plan.members
                .iter()
                .position(|frame| frame.member_id == pid.pid)
        } else {
            None
        };
        match index {
            Some(index) => {
                M::read_slot(
                    &mut slots,
                    index,
                    ValueDeserializer::new(reader, plans, &plan.members[index].value),
                )?;
                reader.finish_pid(pid)?;
            }
            None if pid.must_understand => {
                return Err(WireError::refused(
                    reader.position(),
                    "a must-understand parameter this type does not declare, per §7.2.2.4.4.4.6",
                ));
            }
            None => reader.skip_parameter(pid)?,
        }
    }
}

/// What a [`StructDeserializer`] carries from member to member.
struct State<'r, M: MemberList> {
    plan: Option<&'r StructPlan>,
    /// Where an `@appendable` writer's members end (its DHEADER).
    end: Option<usize>,
    /// The key members, read ahead from a key holder.
    slots: Option<M::Slots>,
    next: usize,
}

/// The members of one struct value still to come.
pub struct StructDeserializer<'r, 'de, M: MemberList> {
    source: Source<'r, 'de>,
    plans: Option<&'r PlanSet>,
    state: State<'r, M>,
}

impl<M: MemberList> core::fmt::Debug for StructDeserializer<'_, '_, M> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("StructDeserializer")
            .field("next", &self.state.next)
            .finish_non_exhaustive()
    }
}

impl<'r, 'de, H: DdsMemberRead<'de>, T: MemberList> StructDeserializer<'r, 'de, (H, T)> {
    /// The next member's value.
    ///
    /// A member an `@appendable` writer left out takes its default (XTypes
    /// 1.3 Table 19).
    ///
    /// # Errors
    /// What the stream or the member's type refuses.
    pub fn member(self) -> Result<(H, StructDeserializer<'r, 'de, T>), WireError> {
        let Self {
            mut source,
            plans,
            state,
        } = self;
        let (value, rest_slots) = match (&mut source, state.slots) {
            (Source::Defaults, _) => (H::default_member()?, None),
            (Source::Reader(_), Some((slot, rest))) => {
                let value = match slot {
                    Some(value) => value,
                    None => H::default_member()?,
                };
                (value, Some(rest))
            }
            (Source::Reader(reader), None) => {
                let plan = state.plan.ok_or_else(no_plan)?;
                let plans = plans.ok_or_else(no_plan)?;
                let frame = plan.members.get(state.next).ok_or_else(no_plan)?;
                let reached = match state.end {
                    Some(end) => reader.position() >= end,
                    // XCDR1 has no DHEADER for an `@appendable` type
                    // (§7.4.2): the payload's end is where the writer's
                    // members stop, as Cyclone, RTI and Fast DDS read it.
                    None => plan.appendable && reader.remaining() == 0,
                };
                let value = if plan.appendable && reached {
                    H::default_member()?
                } else {
                    read_frame::<H>(reader, plans, frame)?
                };
                (value, None)
            }
        };
        Ok((
            value,
            StructDeserializer {
                source,
                plans,
                state: State {
                    plan: state.plan,
                    end: state.end,
                    slots: rest_slots,
                    next: state.next + 1,
                },
            },
        ))
    }
}

impl<'r, 'de, A, V, T> StructDeserializer<'r, 'de, (super::With<A, V>, T)>
where
    T: MemberList,
    super::With<A, V>: DdsMemberRead<'de>,
{
    /// The next member's value, through the adapter its
    /// [`super::With`] names.
    ///
    /// # Errors
    /// What the stream or the adapter refuses.
    pub fn member_with(self) -> Result<(V, StructDeserializer<'r, 'de, T>), WireError> {
        let (with, rest) = self.member()?;
        Ok((with.into_inner(), rest))
    }
}

impl StructDeserializer<'_, '_, ()> {
    /// Close the struct: past the DHEADER's extent, where a newer writer
    /// may have appended members this type does not declare.
    ///
    /// # Errors
    /// An extent that ends behind what was read.
    pub fn end(self) -> Result<Finish, WireError> {
        if let (Source::Reader(reader), Some(end)) = (self.source, self.state.end) {
            reader.skip_to(end)?;
        }
        Ok(Finish { _private: () })
    }
}

/// A `@mutable` struct's members, one slot per member id.
pub struct MutableSlots<M: MemberList> {
    slots: M::Slots,
    members: PhantomData<fn() -> M>,
}

impl<M: MemberList> core::fmt::Debug for MutableSlots<M> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("MutableSlots").finish_non_exhaustive()
    }
}

impl<M: MemberList> MutableSlots<M> {
    /// The slots as the stream left them: `(Option<u32>, (Option<String>, ()))`.
    #[must_use]
    pub fn into_slots(self) -> (M::Slots, Finish) {
        (self.slots, Finish { _private: () })
    }

    /// The values, a member no header named taking its default (XTypes 1.3
    /// §7.4.1.2.3).
    ///
    /// # Errors
    /// A member whose default its Rust type refuses.
    pub fn values(self) -> Result<(M::Values, Finish), WireError> {
        Ok((M::into_values(self.slots)?, Finish { _private: () }))
    }
}

/// A member's framing, then its value, for a type read in order.
pub(crate) fn read_frame<'de, H: DdsMemberRead<'de>>(
    reader: &mut Reader<'de>,
    plans: &PlanSet,
    frame: &super::MemberFrame,
) -> Result<H, WireError> {
    let plan = &frame.value;
    match frame.header {
        Header::None => H::deserialize_member(ValueDeserializer::new(reader, plans, plan)),
        Header::OptionalFlag => {
            if reader.read_bool()? {
                H::deserialize_member(ValueDeserializer::new(reader, plans, plan))
            } else {
                H::absent()
            }
        }
        Header::Pid {
            role: ParameterRole::Optional,
            ..
        } => {
            let pid = reader.read_pid_as(ParameterRole::Optional)?;
            let value = if pid.length == 0 {
                H::absent()?
            } else {
                H::deserialize_member(ValueDeserializer::new(reader, plans, plan))?
            };
            reader.finish_pid(pid)?;
            Ok(value)
        }
        Header::Pid { .. } | Header::EmHeader { .. } => Err(WireError::refused(
            reader.position(),
            "a member read in order; a @mutable type's members are read by id",
        )),
    }
}

fn no_plan() -> WireError {
    WireError::refused(0, "a layout plan for every member the type declares")
}

/// Where one value comes from, and the plan it comes in under.
pub struct ValueDeserializer<'r, 'de> {
    r: &'r mut Reader<'de>,
    plans: &'r PlanSet,
    plan: &'r ValuePlan,
    /// How many of an array's dimensions the enclosing calls have taken.
    dims: usize,
}

impl core::fmt::Debug for ValueDeserializer<'_, '_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ValueDeserializer")
            .field("plan", self.plan)
            .field("dims", &self.dims)
            .finish_non_exhaustive()
    }
}

impl<'r, 'de> ValueDeserializer<'r, 'de> {
    pub(crate) const fn new(
        r: &'r mut Reader<'de>,
        plans: &'r PlanSet,
        plan: &'r ValuePlan,
    ) -> Self {
        Self {
            r,
            plans,
            plan,
            dims: 0,
        }
    }

    /// Where the value begins.
    #[must_use]
    pub const fn position(&self) -> usize {
        self.r.position()
    }

    fn mismatch(&self, what: &str) -> WireError {
        WireError::OutOfRange {
            at: self.r.position(),
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
    /// A plan that has another type here, and what the stream refuses.
    pub fn boolean(self) -> Result<bool, WireError> {
        self.require(&[Primitive::Boolean])?;
        self.r.read_bool()
    }

    /// `octet` or `uint8`.
    ///
    /// # Errors
    /// As [`Self::boolean`].
    pub fn uint8(self) -> Result<u8, WireError> {
        self.require(&[Primitive::Octet, Primitive::UInt8])?;
        self.r.read_u8()
    }

    /// `int8`.
    ///
    /// # Errors
    /// As [`Self::boolean`].
    pub fn int8(self) -> Result<i8, WireError> {
        self.require(&[Primitive::Int8])?;
        self.r.read_i8()
    }

    /// `short`.
    ///
    /// # Errors
    /// As [`Self::boolean`].
    pub fn int16(self) -> Result<i16, WireError> {
        self.require(&[Primitive::Int16])?;
        self.r.read_i16()
    }

    /// `unsigned short`.
    ///
    /// # Errors
    /// As [`Self::boolean`].
    pub fn uint16(self) -> Result<u16, WireError> {
        self.require(&[Primitive::UInt16])?;
        self.r.read_u16()
    }

    /// `long`.
    ///
    /// # Errors
    /// As [`Self::boolean`].
    pub fn int32(self) -> Result<i32, WireError> {
        self.require(&[Primitive::Int32])?;
        self.r.read_i32()
    }

    /// `unsigned long`.
    ///
    /// # Errors
    /// As [`Self::boolean`].
    pub fn uint32(self) -> Result<u32, WireError> {
        self.require(&[Primitive::UInt32])?;
        self.r.read_u32()
    }

    /// `long long`.
    ///
    /// # Errors
    /// As [`Self::boolean`].
    pub fn int64(self) -> Result<i64, WireError> {
        self.require(&[Primitive::Int64])?;
        self.r.read_i64()
    }

    /// `unsigned long long`.
    ///
    /// # Errors
    /// As [`Self::boolean`].
    pub fn uint64(self) -> Result<u64, WireError> {
        self.require(&[Primitive::UInt64])?;
        self.r.read_u64()
    }

    /// `float`.
    ///
    /// # Errors
    /// As [`Self::boolean`].
    pub fn float32(self) -> Result<f32, WireError> {
        self.require(&[Primitive::Float32])?;
        self.r.read_f32()
    }

    /// `double`.
    ///
    /// # Errors
    /// As [`Self::boolean`].
    pub fn float64(self) -> Result<f64, WireError> {
        self.require(&[Primitive::Float64])?;
        self.r.read_f64()
    }

    /// `char`.
    ///
    /// # Errors
    /// As [`Self::boolean`].
    pub fn character(self) -> Result<char, WireError> {
        self.require(&[Primitive::Char])?;
        self.r.read_char()
    }

    /// A `string`, copied into a `String`.
    ///
    /// # Errors
    /// As [`Self::idl_str`].
    pub fn string(self) -> Result<String, WireError> {
        self.idl_str(|text| Ok(String::from(text)))
    }

    /// A `string` lent out of the received bytes, for a
    /// [`crate::WireRepr`] type to read from.
    ///
    /// A value over the bound of a `string<N>` discards the sample, which
    /// is what an absent `@try_construct` asks for (XTypes 1.3 §7.2.2.7).
    ///
    /// # Errors
    /// A plan that has another type here, what the stream refuses, and
    /// what `read` refuses.
    pub fn idl_str<R>(
        self,
        read: impl FnOnce(&'de str) -> Result<R, WireError>,
    ) -> Result<R, WireError> {
        let ValuePlan::String { bound } = *self.plan else {
            return Err(self.mismatch("a string"));
        };
        let text = self.r.read_str()?;
        let text = match bound {
            Some(bound) => match self.r.fit_str(text, bound, TryConstruct::Discard)? {
                Narrowed::Value(text) => text,
                Narrowed::Default => {
                    return Err(WireError::refused(
                        self.r.position(),
                        "a narrowing that asked for no default and got one",
                    ));
                }
            },
            None => text,
        };
        read(text)
    }

    /// A primitive, for a [`crate::WireRepr`] type to read from.
    ///
    /// # Errors
    /// A plan that has another type here, what the stream refuses, and
    /// what `read` refuses.
    pub fn idl_primitive<P: IdlPrimitive, R>(
        self,
        read: impl FnOnce(&P) -> Result<R, WireError>,
    ) -> Result<R, WireError> {
        match self.plan {
            ValuePlan::Primitive(kind) if P::stands_for(*kind) => {}
            _ => return Err(self.mismatch("the primitive of the member's IDL type")),
        }
        let value = P::read(self.r)?;
        read(&value)
    }

    /// A `sequence`. One over the bound discards the sample (XTypes 1.3
    /// §7.2.2.7, Table 17).
    ///
    /// # Errors
    /// A plan that has another type here, and what the stream or an
    /// element refuses.
    pub fn sequence<T: DdsMemberRead<'de>>(self) -> Result<Vec<T>, WireError> {
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
        let read =
            |r: &mut Reader<'de>| T::deserialize_member(ValueDeserializer::new(r, plans, element));
        let values = if delimited {
            self.r.read_delimited_sequence(read)?
        } else {
            self.r.read_sequence(read)?
        };
        match bound {
            Some(bound) => match self.r.fit_sequence(values, bound, TryConstruct::Discard)? {
                Narrowed::Value(values) => Ok(values),
                Narrowed::Default => Err(WireError::refused(
                    self.r.position(),
                    "a narrowing that asked for no default and got one",
                )),
            },
            None => Ok(values),
        }
    }

    /// One dimension of an array; an inner dimension is the element's.
    ///
    /// # Errors
    /// A plan that has another type or length here, and what the stream
    /// or an element refuses.
    pub fn array<T: DdsMemberRead<'de>, const N: usize>(self) -> Result<[T; N], WireError> {
        let ValuePlan::Array {
            ref element,
            ref dims,
            delimited,
        } = *self.plan
        else {
            return Err(self.mismatch("an array"));
        };
        if dims.get(self.dims) != Some(&N) {
            return Err(self.mismatch("an array of the member's length"));
        }
        let plans = self.plans;
        let whole = self.plan;
        let depth = self.dims + 1;
        let inner = depth < dims.len();
        let element: &ValuePlan = element;
        let read = |r: &mut Reader<'de>| {
            let input = if inner {
                ValueDeserializer {
                    r,
                    plans,
                    plan: whole,
                    dims: depth,
                }
            } else {
                ValueDeserializer::new(r, plans, element)
            };
            T::deserialize_member(input)
        };
        if delimited && self.dims == 0 {
            self.r.read_delimited_array_n(read)
        } else {
            self.r.read_array_n(read)
        }
    }

    /// An enumeration, as the index of its enumerator in declaration
    /// order.
    ///
    /// # Errors
    /// A plan that has another type here, what the stream refuses, and a
    /// value that names no enumerator.
    pub fn enumeration(self) -> Result<usize, WireError> {
        let ValuePlan::Enum {
            holder_bits,
            ref values,
        } = *self.plan
        else {
            return Err(self.mismatch("an enumeration"));
        };
        let value = match holder_bits {
            0..=8 => i128::from(self.r.read_u8()?),
            9..=16 => i128::from(self.r.read_u16()?),
            17..=32 => i128::from(self.r.read_u32()?),
            _ => i128::from(self.r.read_u64()?),
        };
        values
            .iter()
            .position(|declared| *declared == value)
            .ok_or_else(|| {
                WireError::refused(self.r.position(), "a value that names an enumerator")
            })
    }

    /// A struct member: the type's own [`DdsDeserialize`] under the member
    /// type's plan.
    ///
    /// # Errors
    /// A plan that has another type here, and what the value refuses.
    pub fn nested<T: DdsDeserialize<'de>>(self) -> Result<T, WireError> {
        let target = match *self.plan {
            ValuePlan::Struct(index) => Target::Struct(index),
            ValuePlan::Union(index) => Target::Union(index),
            ValuePlan::Holder(index) => Target::Holder(index),
            _ => return Err(self.mismatch("a struct or a union")),
        };
        T::deserialize(DdsDeserializer {
            source: Source::Reader(self.r),
            plans: Some(self.plans),
            target,
        })
        .map(Decoded::into_inner)
    }

    /// A `string` lent out of the received bytes, as a `&'de str` member
    /// holds it. A value over the bound of a `string<N>` discards the
    /// sample.
    ///
    /// # Errors
    /// As [`Self::idl_str`].
    pub fn borrowed_str(self) -> Result<&'de str, WireError> {
        self.idl_str(Ok)
    }

    /// A `sequence<octet>` lent out of the received bytes, as a
    /// `&'de [u8]` member holds it. One over its bound discards the sample
    /// (XTypes 1.3 §7.2.2.7, Table 17).
    ///
    /// # Errors
    /// A plan that has another type here, and what the stream refuses.
    pub fn borrowed_octets(self) -> Result<&'de [u8], WireError> {
        let ValuePlan::Sequence {
            ref element,
            bound,
            delimited: false,
        } = *self.plan
        else {
            return Err(self.mismatch("a sequence of octets"));
        };
        if !matches!(
            **element,
            ValuePlan::Primitive(Primitive::Octet | Primitive::UInt8)
        ) {
            return Err(self.mismatch("a sequence of octets"));
        }
        self.r.align_to(4)?;
        let at = self.r.position();
        let length = usize::try_from(self.r.read_u32()?)
            .map_err(|_| WireError::refused(at, "a sequence length this host's usize holds"))?;
        if let Some(bound) = bound
            && length > bound
        {
            return Err(WireError::Discarded {
                at,
                expected: alloc::format!("a sequence<octet, {bound}>"),
                found: alloc::format!("{length} octets"),
            });
        }
        self.r.read_bytes(length)
    }
}
