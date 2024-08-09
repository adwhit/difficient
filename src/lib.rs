//! # Difficient
//!
//! Efficient, type-safe, (almost) zero-allocation diffing.
//!
//!
//! # Example
//!
//! ```
//! use difficient::{Diffable, DeepDiff, AtomicDiff, Id};
//!
//! #[derive(Diffable, PartialEq, Debug, Clone)]
//! enum SimpleEnum {
//!     First,
//!     Second { x: &'static str, y: (), z: SimpleStruct },
//! }
//!
//! #[derive(Diffable, PartialEq, Debug, Clone)]
//! struct SimpleStruct {
//!     a: String,
//!     b: i32,
//! }
//!
//! let mut first = SimpleEnum::First;
//!
//! let diff1 = first.diff(&first);
//! assert_eq!(diff1, DeepDiff::Unchanged);
//!
//! let mut second1 = SimpleEnum::Second {
//!     x: "hello",
//!     y: (),
//!     z: SimpleStruct { a: "aaa".into(), b: 123 }
//! };
//!
//! let diff2 = first.diff(&second1);
//! let expect = DeepDiff::Replaced(&second1);
//! assert_eq!(diff2, expect);
//! first.apply(diff2);
//! assert_eq!(first, second1);
//!
//! let second2 = SimpleEnum::Second {
//!     x: "goodbye",
//!     y: (),
//!     z: SimpleStruct { a: "aaa".into(), b: 234 }
//! };
//!
//! let diff3 = second1.diff(&second2);
//! let expect = DeepDiff::Patched(SimpleEnumDiff::Second {
//!     x: AtomicDiff::Replaced(&"goodbye"),
//!     y: Id::new(),
//!     z: DeepDiff::Patched(
//!         SimpleStructDiff { a: AtomicDiff::Unchanged, b: AtomicDiff::Replaced(&234) }
//!     )
//! });
//! assert_eq!(diff3, expect);
//! second1.apply(diff3);
//! assert_eq!(second1, second2);
//! ```

#![deny(warnings)]

use std::{
    collections::{BTreeMap, HashMap},
    hash::Hash,
    marker::PhantomData,
    ops::Deref,
};

pub use difficient_macros::Diffable;

#[cfg(feature = "chrono")]
mod chrono;
#[cfg(feature = "uuid")]
mod uuid;

// *** Trait

pub trait Diffable<'a>: Sized {
    type Diff: Replace<Replaces = Self> + Apply<Parent = Self>;
    fn diff(&self, other: &'a Self) -> Self::Diff;
    fn apply(&mut self, diff: Self::Diff) -> Result<(), Vec<ApplyError>> {
        let mut errs = Vec::new();
        diff.apply_to_base(self, &mut errs);
        if errs.is_empty() {
            Ok(())
        } else {
            Err(errs)
        }
    }
}

pub trait Replace {
    type Replaces;
    fn is_unchanged(&self) -> bool;
    fn is_replaced(&self) -> bool;
}

pub trait Apply {
    type Parent;
    fn apply_to_base(&self, source: &mut Self::Parent, errs: &mut Vec<ApplyError>);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ApplyError {
    MismatchingEnum,
    MissingKey,
    UnexpectedKey,
}

impl std::fmt::Display for ApplyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApplyError::MismatchingEnum => {
                write!(f, "enum mismatch")
            }
            ApplyError::MissingKey => {
                write!(f, "missing key")
            }
            ApplyError::UnexpectedKey => {
                write!(f, "unexpected key")
            }
        }
    }
}

impl std::error::Error for ApplyError {}

// *** Helper structs

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Id<T>(PhantomData<T>);

impl<T> Id<T> {
    pub fn new() -> Id<T> {
        Id(PhantomData)
    }
}

impl<T> Replace for Id<T> {
    type Replaces = T;

    fn is_unchanged(&self) -> bool {
        true
    }

    fn is_replaced(&self) -> bool {
        false
    }
}

impl<T> Apply for Id<T> {
    type Parent = T;
    fn apply_to_base(&self, _: &mut Self::Parent, _: &mut Vec<ApplyError>) {}
}

#[derive(Debug, Clone, PartialEq)]
pub enum AtomicDiff<'a, T> {
    Unchanged,
    Replaced(&'a T),
}

impl<'a, T> Replace for AtomicDiff<'a, T> {
    type Replaces = T;

    fn is_unchanged(&self) -> bool {
        matches!(self, AtomicDiff::Unchanged)
    }

    fn is_replaced(&self) -> bool {
        matches!(self, AtomicDiff::Replaced(_))
    }
}

impl<'a, T> Apply for AtomicDiff<'a, T>
where
    T: Clone,
{
    type Parent = T;
    fn apply_to_base(&self, source: &mut Self::Parent, _: &mut Vec<ApplyError>) {
        match self {
            AtomicDiff::Unchanged => {}
            AtomicDiff::Replaced(r) => *source = (*r).clone(),
        };
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum DeepDiff<'a, Full, Patch> {
    Unchanged,
    Patched(Patch),
    Replaced(&'a Full),
}

impl<'a, T, U> Replace for DeepDiff<'a, T, U> {
    type Replaces = T;

    fn is_unchanged(&self) -> bool {
        matches!(self, DeepDiff::Unchanged)
    }

    fn is_replaced(&self) -> bool {
        matches!(self, DeepDiff::Replaced(_))
    }
}

impl<'a, T, U> Apply for DeepDiff<'a, T, U>
where
    T: Diffable<'a> + Clone,
    U: Apply<Parent = T>, // <<T as Diffable>::Diff as Apply>::Parent = T,
{
    type Parent = T;

    fn apply_to_base(&self, source: &mut Self::Parent, errs: &mut Vec<ApplyError>) {
        match self {
            DeepDiff::Unchanged => {}
            DeepDiff::Patched(patch) => patch.apply_to_base(source, errs),
            DeepDiff::Replaced(r) => *source = (*r).clone(),
        };
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum KvDiff<'a, T: Diffable<'a>> {
    Removed,
    Inserted(&'a T),
    Diff(T::Diff),
}

// ** Common impls ***

macro_rules! impl_diffable_for_primitives {
    ($($typ: ty)*) => ($(
        impl<'a> Diffable<'a> for $typ {
            type Diff = AtomicDiff<'a, Self>;

            fn diff(&self, other: &'a Self) -> Self::Diff {
                if self == other {
                    AtomicDiff::Unchanged
                } else {
                    AtomicDiff::Replaced(&other)
                }
            }
        }
    )*);
}

impl_diffable_for_primitives! {
    i8 i16 i32 i64
    u8 u16 u32 u64
    f32 f64
    bool
    &'static str
    String
}

impl<'a, T: Clone + PartialEq + 'a> Diffable<'a> for Vec<T> {
    type Diff = AtomicDiff<'a, Vec<T>>;

    fn diff(&self, other: &'a Self) -> Self::Diff {
        if self.len() != other.len() {
            return AtomicDiff::Replaced(&other);
        }
        for (elem, other_elem) in self.iter().zip(other.iter()) {
            if elem != other_elem {
                return AtomicDiff::Replaced(&other);
            }
        }
        AtomicDiff::Unchanged
    }
}

macro_rules! kv_map_impl {
    ($typ: ident, $bounds: ident) => {
        impl<'a, K, V> Diffable<'a> for $typ<K, V>
        where
            K: $bounds + Eq + Clone + 'a,
            V: Diffable<'a> + Clone + 'a,
        {
            type Diff = DeepDiff<'a, Self, $typ<K, KvDiff<'a, V>>>;

            fn diff(&self, other: &'a Self) -> Self::Diff {
                let mut diffs: $typ<K, KvDiff<V>> = $typ::new();
                let mut all_unchanged = true;
                let mut all_replaced = true;
                for (k, v) in self.iter() {
                    let Some(other) = other.get(k) else {
                        all_replaced = false;
                        all_unchanged = false;
                        diffs.insert(k.clone(), KvDiff::Removed);
                        continue;
                    };
                    let diff = v.diff(other);
                    if diff.is_unchanged() {
                        // do 'nothing'
                        all_replaced = false;
                        continue;
                    } else {
                        all_replaced &= diff.is_replaced();
                        all_unchanged = false;
                        diffs.insert(k.clone(), KvDiff::Diff(diff));
                    }
                }
                for (k, v) in other.iter() {
                    if !other.contains_key(k) {
                        all_unchanged = false;
                        all_replaced = false;
                        diffs.insert(k.clone(), KvDiff::Inserted(v));
                    }
                }
                if all_unchanged {
                    DeepDiff::Unchanged
                } else if all_replaced {
                    DeepDiff::Replaced(other)
                } else {
                    DeepDiff::Patched(diffs)
                }
            }
        }

        impl<'a, K, V> Apply for $typ<K, KvDiff<'a, V>>
        where
            K: $bounds + Eq + Clone,
            V: Diffable<'a> + Clone,
        {
            type Parent = $typ<K, V>;

            fn apply_to_base(&self, source: &mut Self::Parent, errs: &mut Vec<ApplyError>) {
                for (k, v) in self.into_iter() {
                    match v {
                        KvDiff::Removed => match source.remove(&k) {
                            Some(_) => {}
                            None => errs.push(ApplyError::MissingKey),
                        },
                        KvDiff::Inserted(val) => {
                            match source.insert((*k).clone(), (*val).clone()) {
                                Some(_) => errs.push(ApplyError::UnexpectedKey),
                                None => {}
                            }
                        }
                        KvDiff::Diff(diff) => match source.get_mut(&k) {
                            Some(val) => diff.apply_to_base(val, errs),
                            None => errs.push(ApplyError::MissingKey),
                        },
                    }
                }
            }
        }
    };
}

kv_map_impl!(HashMap, Hash);
kv_map_impl!(BTreeMap, Ord);

impl<'a> Diffable<'a> for () {
    type Diff = Id<Self>;

    fn diff(&self, _: &Self) -> Self::Diff {
        Id::new()
    }
}

impl<'a, T> Diffable<'a> for Box<T>
where
    T: Diffable<'a>,
{
    type Diff = Box<T::Diff>;

    fn diff(&self, other: &'a Self) -> Self::Diff {
        Box::new(self.deref().diff(other.deref()))
    }
}

impl<T> Replace for Box<T>
where
    T: Replace,
{
    type Replaces = Box<T::Replaces>;

    fn is_unchanged(&self) -> bool {
        self.deref().is_unchanged()
    }

    fn is_replaced(&self) -> bool {
        self.deref().is_replaced()
    }
}

impl<T> Apply for Box<T>
where
    T: Apply,
{
    type Parent = Box<T::Parent>;

    fn apply_to_base(&self, source: &mut Self::Parent, errs: &mut Vec<ApplyError>) {
        self.deref().apply_to_base(source, errs)
    }
}

impl<'a, T> Diffable<'a> for Option<T>
where
    T: Diffable<'a> + Clone + 'a,
{
    type Diff = DeepDiff<'a, Self, Option<T::Diff>>;

    fn diff(&self, other: &'a Self) -> Self::Diff {
        match (self, other) {
            (None, None) => DeepDiff::Unchanged,
            (None, Some(_)) | (Some(_), None) => DeepDiff::Replaced(other),
            (Some(l), Some(r)) => {
                let diff = l.diff(r);
                if diff.is_unchanged() {
                    DeepDiff::Unchanged
                } else if diff.is_replaced() {
                    DeepDiff::Replaced(other)
                } else {
                    DeepDiff::Patched(Some(diff))
                }
            }
        }
    }
}

impl<T> Apply for Option<T>
where
    T: Apply,
{
    type Parent = Option<T::Parent>;

    fn apply_to_base(&self, source: &mut Self::Parent, errs: &mut Vec<ApplyError>) {
        match (self, source) {
            (Some(diff), Some(src)) => diff.apply_to_base(src, errs),
            _ => errs.push(ApplyError::MismatchingEnum),
        }
    }
}

macro_rules! tuple_impl {
    ( $( $tup:ident $ix:tt ),* ) => {
        impl<'a, $( $tup ),*> Diffable<'a> for ( $( $tup, )* )
        where
            $( $tup: Diffable<'a> ),*
        {
            type Diff = ( $( $tup::Diff,)* );

            fn diff(&self, other: &'a Self) -> Self::Diff {
                (
                    $(
                        self.$ix.diff(&other.$ix),
                    )*
                )
            }
        }

        impl< $( $tup ),*> Replace for ($( $tup, )*)
        where
            $( $tup: Replace ),*
        {
            type Replaces = ( $( $tup::Replaces, )* );

            fn is_unchanged(&self) -> bool {
                    $(
                        self.$ix.is_unchanged() &&
                    )*
                    true
            }

            fn is_replaced(&self) -> bool {
                    $(
                        self.$ix.is_replaced() &&
                    )*
                    true
            }
        }

        impl< $( $tup ),*> Apply for ( $( $tup, )*)
        where
            $( $tup: Apply ),*
        {
            type Parent = ( $( $tup::Parent, )* );

            fn apply_to_base(&self, source: &mut Self::Parent, errs: &mut Vec<ApplyError>) {
                    $(
                        self.$ix.apply_to_base(&mut source.$ix, errs);
                    )*
            }
        }
    };
}

tuple_impl!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8);
tuple_impl!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7);
tuple_impl!(A 0, B 1, C 2, D 3, E 4, F 5, G 6);
tuple_impl!(A 0, B 1, C 2, D 3, E 4, F 5);
tuple_impl!(A 0, B 1, C 2, D 3, E 4);
tuple_impl!(A 0, B 1, C 2, D 3);
tuple_impl!(A 0, B 1, C 2);
tuple_impl!(A 0, B 1);
tuple_impl!(A 0);

#[cfg(test)]
mod tests {
    use super::*;

    // ** Test structs

    #[derive(Debug, Clone, PartialEq)]
    struct Parent {
        c1: Child1,
        c2: Vec<Child1>,
        c3: HashMap<i32, Child2>,
        val: String,
    }

    #[derive(Debug, Clone, PartialEq)]
    struct Child1 {
        x: i32,
        y: String,
    }

    #[derive(Debug, Clone, PartialEq)]
    struct Child2 {
        a: String,
        b: SomeChild,
        c: (),
    }

    #[derive(Debug, Clone, PartialEq)]
    enum SomeChild {
        C1(Child1),
        C2(Box<Child2>),
    }

    // *** Impls ***

    impl<'a> Diffable<'a> for Parent {
        type Diff = DeepDiff<'a, Self, ParentDiff<'a>>;

        fn diff(&self, other: &'a Self) -> Self::Diff {
            let c1 = self.c1.diff(&other.c1);
            let c2 = self.c2.diff(&other.c2);
            let c3 = self.c3.diff(&other.c3);
            let val = self.val.diff(&other.val);
            if c1.is_unchanged() && c2.is_unchanged() && c3.is_unchanged() && val.is_unchanged() {
                DeepDiff::Unchanged
            } else if c1.is_replaced() && c2.is_replaced() && c3.is_replaced() && val.is_replaced()
            {
                DeepDiff::Replaced(other)
            } else {
                DeepDiff::Patched(ParentDiff { c1, c2, c3, val })
            }
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    struct ParentDiff<'a> {
        c1: <Child1 as Diffable<'a>>::Diff,
        c2: <Vec<Child1> as Diffable<'a>>::Diff,
        c3: <HashMap<i32, Child2> as Diffable<'a>>::Diff,
        val: <String as Diffable<'a>>::Diff,
    }

    impl<'a> Apply for ParentDiff<'a> {
        type Parent = Parent;

        fn apply_to_base(&self, source: &mut Self::Parent, errs: &mut Vec<ApplyError>) {
            self.c1.apply_to_base(&mut source.c1, errs);
            self.c2.apply_to_base(&mut source.c2, errs);
            self.c3.apply_to_base(&mut source.c3, errs);
            self.val.apply_to_base(&mut source.val, errs);
        }
    }

    impl<'a> Diffable<'a> for Child1 {
        type Diff = DeepDiff<'a, Self, Child1Diff<'a>>;

        fn diff(&self, other: &'a Self) -> Self::Diff {
            let x = self.x.diff(&other.x);
            let y = self.y.diff(&other.y);
            if x.is_unchanged() && y.is_unchanged() {
                DeepDiff::Unchanged
            } else if x.is_replaced() && y.is_replaced() {
                DeepDiff::Replaced(other)
            } else {
                DeepDiff::Patched(Child1Diff { x, y })
            }
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    struct Child1Diff<'a> {
        x: <i32 as Diffable<'a>>::Diff,
        y: <String as Diffable<'a>>::Diff,
    }

    impl<'a> Apply for Child1Diff<'a> {
        type Parent = Child1;

        fn apply_to_base(&self, source: &mut Self::Parent, errs: &mut Vec<ApplyError>) {
            self.x.apply_to_base(&mut source.x, errs);
            self.y.apply_to_base(&mut source.y, errs);
        }
    }

    impl<'a> Diffable<'a> for Child2 {
        type Diff = DeepDiff<'a, Self, Child2Diff<'a>>;
        fn diff(&self, other: &'a Self) -> Self::Diff {
            let a = self.a.diff(&other.a);
            let b = self.b.diff(&other.b);
            let c = self.c.diff(&other.c);
            if a.is_unchanged() && b.is_unchanged() && c.is_unchanged() {
                DeepDiff::Unchanged
            } else if a.is_replaced() && b.is_replaced() && c.is_replaced() {
                DeepDiff::Replaced(other)
            } else {
                DeepDiff::Patched(Child2Diff { a, b, c })
            }
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    struct Child2Diff<'a> {
        a: <String as Diffable<'a>>::Diff,
        b: <SomeChild as Diffable<'a>>::Diff,
        c: <() as Diffable<'a>>::Diff,
    }

    impl<'a> Apply for Child2Diff<'a> {
        type Parent = Child2;

        fn apply_to_base(&self, source: &mut Self::Parent, errs: &mut Vec<ApplyError>) {
            self.a.apply_to_base(&mut source.a, errs);
            self.b.apply_to_base(&mut source.b, errs);
            self.c.apply_to_base(&mut source.c, errs);
        }
    }

    impl<'a> Diffable<'a> for SomeChild {
        type Diff = DeepDiff<'a, SomeChild, SomeChildDiff<'a>>;
        fn diff(&self, other: &'a Self) -> Self::Diff {
            match (self, other) {
                (Self::C1(left), Self::C1(right)) => {
                    let this = left.diff(right);
                    if this.is_unchanged() {
                        DeepDiff::Unchanged
                    } else if this.is_replaced() {
                        DeepDiff::Replaced(other)
                    } else {
                        DeepDiff::Patched(SomeChildDiff::C1(this))
                    }
                }
                (Self::C2(left), Self::C2(right)) => {
                    let this = left.diff(right);
                    if this.is_unchanged() {
                        DeepDiff::Unchanged
                    } else if this.is_replaced() {
                        DeepDiff::Replaced(other)
                    } else {
                        DeepDiff::Patched(SomeChildDiff::C2(this))
                    }
                }
                _ => DeepDiff::Replaced(other),
            }
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    enum SomeChildDiff<'a> {
        C1(<Child1 as Diffable<'a>>::Diff),
        C2(Box<<Child2 as Diffable<'a>>::Diff>),
    }

    impl<'a> Apply for SomeChildDiff<'a> {
        type Parent = SomeChild;

        fn apply_to_base(&self, source: &mut Self::Parent, errs: &mut Vec<ApplyError>) {
            match (self, source) {
                (SomeChildDiff::C1(diff), SomeChild::C1(src)) => diff.apply_to_base(src, errs),
                (SomeChildDiff::C2(diff), SomeChild::C2(src)) => diff.apply_to_base(src, errs),
                _ => errs.push(ApplyError::MismatchingEnum),
            }
        }
    }

    #[test]
    fn smoke_test() {
        fn dummy_child2() -> Child2 {
            Child2 {
                a: "ayeaye".into(),
                b: SomeChild::C1(Child1 {
                    x: 222,
                    y: "uuu".into(),
                }),
                c: (),
            }
        }

        let base = Parent {
            c1: Child1 {
                x: 123,
                y: "me".into(),
            },
            c2: vec![Child1 {
                x: 234,
                y: "yazoo".into(),
            }],
            c3: [(
                321,
                Child2 {
                    a: "ayeaye".into(),
                    b: SomeChild::C2(Box::new(dummy_child2())),
                    c: (),
                },
            )]
            .into_iter()
            .collect(),
            val: "hello".into(),
        };

        {
            let mut p1 = base.clone();
            let p2 = base.clone();
            let diff = p1.diff(&p2);
            assert!(matches!(diff, DeepDiff::Unchanged));
            p1.apply(diff).unwrap();
            assert_eq!(p1, base);
        }

        let mello = "mello".to_string();

        {
            let mut p3 = base.clone();
            let mut p4 = p3.clone();
            p4.val = mello.clone();
            let diff = p3.diff(&p4);
            let expect = DeepDiff::Patched(ParentDiff {
                c1: DeepDiff::Unchanged,
                c2: AtomicDiff::Unchanged,
                c3: DeepDiff::Unchanged,
                val: AtomicDiff::Replaced(&mello),
            });
            assert_eq!(diff, expect);
            p3.apply(diff).unwrap();
        }

        {
            let mut p5 = base.clone();
            let dummy = dummy_child2();
            let bad_patch = DeepDiff::Patched(ParentDiff {
                c1: DeepDiff::Unchanged,
                c2: AtomicDiff::Unchanged,
                c3: DeepDiff::Patched(
                    [
                        (543, KvDiff::Removed),          // key does not exist
                        (321, KvDiff::Inserted(&dummy)), // key already exists
                    ]
                    .into_iter()
                    .collect(),
                ),
                val: AtomicDiff::Replaced(&mello),
            });
            let mut err = p5.apply(bad_patch).unwrap_err();
            err.sort();
            assert_eq!(err, [ApplyError::MissingKey, ApplyError::UnexpectedKey]);
        }
    }
}
