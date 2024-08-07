use std::{collections::HashMap, hash::Hash};

pub use difficient_macros::Diffable;

// *** Trait

pub trait Diffable<'a>: Sized {
    type Diff: Replace<Replaces = Self> + Apply<Parent = Self>;
    fn diff(&self, other: &'a Self) -> Self::Diff;
    fn apply(&mut self, diff: Self::Diff) -> Result<(), Vec<ApplyError>> {
        let mut errs = Vec::new();
        diff.apply(self, &mut errs);
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
    fn get_replaced(&self) -> Option<&Self::Replaces>;
}

pub trait Apply {
    type Parent;
    fn apply(self, source: &mut Self::Parent, errs: &mut Vec<ApplyError>);
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

    fn get_replaced(&self) -> Option<&Self::Replaces> {
        if let Self::Replaced(it) = self {
            Some(&it)
        } else {
            None
        }
    }
}

impl<'a, T> Apply for AtomicDiff<'a, T>
where
    T: Clone,
{
    type Parent = T;
    fn apply(self, source: &mut Self::Parent, _: &mut Vec<ApplyError>) {
        match self {
            AtomicDiff::Unchanged => {}
            AtomicDiff::Replaced(r) => *source = r.clone(),
        };
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum DeepDiff<'a, T, U> {
    Unchanged,
    Patched(U),
    Replaced(&'a T),
}

impl<'a, T, U> Replace for DeepDiff<'a, T, U> {
    type Replaces = T;

    fn is_unchanged(&self) -> bool {
        matches!(self, DeepDiff::Unchanged)
    }

    fn is_replaced(&self) -> bool {
        matches!(self, DeepDiff::Replaced(_))
    }

    fn get_replaced(&self) -> Option<&Self::Replaces> {
        if let Self::Replaced(it) = self {
            Some(it)
        } else {
            None
        }
    }
}

impl<'a, T, U> Apply for DeepDiff<'a, T, U>
where
    T: Diffable<'a> + Clone,
    U: Apply<Parent = T>, // <<T as Diffable>::Diff as Apply>::Parent = T,
{
    type Parent = T;

    fn apply(self, source: &mut Self::Parent, errs: &mut Vec<ApplyError>) {
        match self {
            DeepDiff::Unchanged => {}
            DeepDiff::Patched(patch) => patch.apply(source, errs),
            DeepDiff::Replaced(r) => *source = r.clone(),
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

impl<'a, K, V> Diffable<'a> for HashMap<K, V>
where
    K: Hash + Eq + Clone + 'a,
    V: Diffable<'a> + Clone + 'a,
{
    type Diff = DeepDiff<'a, Self, HashMap<K, KvDiff<'a, V>>>;

    fn diff(&self, other: &'a Self) -> Self::Diff {
        let mut diffs: HashMap<K, KvDiff<V>> = HashMap::new();
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

impl<'a, K, V> Apply for HashMap<K, KvDiff<'a, V>>
where
    K: Eq + Hash,
    V: Diffable<'a> + Clone,
{
    type Parent = HashMap<K, V>;

    fn apply(self, source: &mut Self::Parent, errs: &mut Vec<ApplyError>) {
        for (k, v) in self.into_iter() {
            match v {
                KvDiff::Removed => match source.remove(&k) {
                    Some(_) => {}
                    None => errs.push(ApplyError::MissingKey),
                },
                KvDiff::Inserted(val) => match source.insert(k, val.clone()) {
                    Some(_) => errs.push(ApplyError::UnexpectedKey),
                    None => {}
                },
                KvDiff::Diff(diff) => match source.get_mut(&k) {
                    Some(val) => diff.apply(val, errs),
                    None => errs.push(ApplyError::MissingKey),
                },
            }
        }
    }
}

impl<'a> Diffable<'a> for () {
    type Diff = ();

    fn diff(&self, _: &Self) -> Self::Diff {
        ()
    }
}

impl Replace for () {
    type Replaces = ();

    fn is_unchanged(&self) -> bool {
        true
    }

    fn is_replaced(&self) -> bool {
        false
    }

    fn get_replaced(&self) -> Option<&Self::Replaces> {
        None
    }
}

impl Apply for () {
    type Parent = ();

    fn apply(self, _: &mut Self::Parent, _: &mut Vec<ApplyError>) {}
}

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

        fn apply(self, source: &mut Self::Parent, errs: &mut Vec<ApplyError>) {
            self.c1.apply(&mut source.c1, errs);
            self.c2.apply(&mut source.c2, errs);
            self.c3.apply(&mut source.c3, errs);
            self.val.apply(&mut source.val, errs);
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

        fn apply(self, source: &mut Self::Parent, errs: &mut Vec<ApplyError>) {
            self.x.apply(&mut source.x, errs);
            self.y.apply(&mut source.y, errs);
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

        fn apply(self, source: &mut Self::Parent, errs: &mut Vec<ApplyError>) {
            self.a.apply(&mut source.a, errs);
            self.b.apply(&mut source.b, errs);
            Apply::apply(self.c, &mut source.c, errs);
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
                        DeepDiff::Patched(SomeChildDiff::C2(Box::new(this)))
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

        fn apply(self, source: &mut Self::Parent, errs: &mut Vec<ApplyError>) {
            match (self, source) {
                (SomeChildDiff::C1(diff), SomeChild::C1(src)) => diff.apply(src, errs),
                (SomeChildDiff::C2(diff), SomeChild::C2(src)) => diff.apply(src, errs),
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
                    b: SomeChild::C1(Child1 {
                        x: 222,
                        y: "uuu".into(),
                    }),
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

    #[test]
    fn test_derive_simple_struct() {
        #[derive(Diffable, PartialEq, Debug, Clone)]
        struct SimpleStruct {
            x: String,
            y: i32,
        }

        let mut it1 = SimpleStruct {
            x: "hello".into(),
            y: 123,
        };
        let it2 = SimpleStruct {
            x: "bye".into(),
            y: 123,
        };
        let diff = it1.diff(&it2);
        it1.apply(diff).unwrap();
        assert_eq!(it1, it2);
    }

    #[test]
    fn test_simple_enum() {
        #[derive(Diffable, PartialEq, Debug, Clone)]
        enum SimpleEnum {
            First,
            Second(i32),
            Third { x: String, y: () },
        }

        let mut it1 = SimpleEnum::First;
        let it2 = SimpleEnum::Second(123);
        let diff = it1.diff(&it2);
        it1.apply(diff).unwrap();
        assert_eq!(it1, it2);
    }
}
