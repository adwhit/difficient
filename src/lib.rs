use std::{collections::HashMap, hash::Hash};

pub use difficient_macros::Diffable;

// *** Trait

pub trait Diffable: Sized {
    type Diff: Replace<Replaces = Self> + Apply<Parent = Self>;
    fn diff(&self, other: &Self) -> Self::Diff;
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
    fn get_replaced(self) -> Option<Self::Replaces>;
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
pub enum AtomicDiff<T> {
    Unchanged,
    Replaced(T),
}

impl<T> Replace for AtomicDiff<T> {
    type Replaces = T;

    fn is_unchanged(&self) -> bool {
        matches!(self, AtomicDiff::Unchanged)
    }

    fn is_replaced(&self) -> bool {
        matches!(self, AtomicDiff::Replaced(_))
    }

    fn get_replaced(self) -> Option<Self::Replaces> {
        if let Self::Replaced(it) = self {
            Some(it)
        } else {
            None
        }
    }
}

impl<T> Apply for AtomicDiff<T> {
    type Parent = T;
    fn apply(self, source: &mut Self::Parent, _: &mut Vec<ApplyError>) {
        match self {
            AtomicDiff::Unchanged => {}
            AtomicDiff::Replaced(r) => *source = r,
        };
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Diff<T, U> {
    Unchanged,
    Patched(U),
    Replaced(T),
}

impl<T, U> Replace for Diff<T, U> {
    type Replaces = T;

    fn is_unchanged(&self) -> bool {
        matches!(self, Diff::Unchanged)
    }

    fn is_replaced(&self) -> bool {
        matches!(self, Diff::Replaced(_))
    }

    fn get_replaced(self) -> Option<Self::Replaces> {
        if let Self::Replaced(it) = self {
            Some(it)
        } else {
            None
        }
    }
}

impl<T, U> Apply for Diff<T, U>
where
    T: Diffable,
    U: Apply<Parent = T>, // <<T as Diffable>::Diff as Apply>::Parent = T,
{
    type Parent = T;

    fn apply(self, source: &mut Self::Parent, errs: &mut Vec<ApplyError>) {
        match self {
            Diff::Unchanged => {}
            Diff::Patched(patch) => patch.apply(source, errs),
            Diff::Replaced(r) => *source = r,
        };
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum KvDiff<T: Diffable> {
    Removed,
    Inserted(T),
    Diff(T::Diff),
}

impl<T: Diffable> KvDiff<T> {
    fn diff(self) -> Option<T::Diff> {
        if let KvDiff::Diff(d) = self {
            Some(d)
        } else {
            None
        }
    }
}

// ** Common impls ***

macro_rules! impl_diffable_for_primitives {
    ($($typ: ty)*) => ($(
        impl Diffable for $typ {
            type Diff = AtomicDiff<Self>;

            fn diff(&self, other: &Self) -> Self::Diff {
                if self == other {
                    AtomicDiff::Unchanged
                } else {
                    AtomicDiff::Replaced(other.clone())
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

impl<T: Clone + PartialEq> Diffable for Vec<T> {
    type Diff = AtomicDiff<Vec<T>>;

    fn diff(&self, other: &Self) -> Self::Diff {
        if self.len() != other.len() {
            return AtomicDiff::Replaced(other.clone());
        }
        for (elem, other_elem) in self.iter().zip(other.iter()) {
            if elem != other_elem {
                return AtomicDiff::Replaced(other.clone());
            }
        }
        AtomicDiff::Unchanged
    }
}

impl<K: Hash + Eq + Clone, V: Diffable + Clone> Diffable for HashMap<K, V> {
    type Diff = Diff<Self, HashMap<K, KvDiff<V>>>;

    fn diff(&self, other: &Self) -> Self::Diff {
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
                diffs.insert(k.clone(), KvDiff::Inserted(v.clone()));
            }
        }
        if all_unchanged {
            Diff::Unchanged
        } else if all_replaced {
            let replace = diffs
                .into_iter()
                .map(|(k, v)| (k, v.diff().unwrap().get_replaced().unwrap()))
                .collect();
            Diff::Replaced(replace)
        } else {
            Diff::Patched(diffs)
        }
    }
}

impl<K: Eq + Hash, V: Diffable> Apply for HashMap<K, KvDiff<V>> {
    type Parent = HashMap<K, V>;

    fn apply(self, source: &mut Self::Parent, errs: &mut Vec<ApplyError>) {
        for (k, v) in self.into_iter() {
            match v {
                KvDiff::Removed => match source.remove(&k) {
                    Some(_) => {}
                    None => errs.push(ApplyError::MissingKey),
                },
                KvDiff::Inserted(val) => match source.insert(k, val) {
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

impl Diffable for () {
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

    fn get_replaced(self) -> Option<Self::Replaces> {
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

    impl Diffable for Parent {
        type Diff = Diff<Self, ParentDiff>;

        fn diff(&self, other: &Self) -> Self::Diff {
            let c1 = self.c1.diff(&other.c1);
            let c2 = self.c2.diff(&other.c2);
            let c3 = self.c3.diff(&other.c3);
            let val = self.val.diff(&other.val);
            if c1.is_unchanged() && c2.is_unchanged() && c3.is_unchanged() && val.is_unchanged() {
                Diff::Unchanged
            } else if c1.is_replaced() && c2.is_replaced() && c3.is_replaced() && val.is_replaced()
            {
                let c1 = c1.get_replaced().unwrap();
                let c2 = c2.get_replaced().unwrap();
                let c3 = c3.get_replaced().unwrap();
                let val = val.get_replaced().unwrap();
                Diff::Replaced(Self { c1, c2, c3, val })
            } else {
                Diff::Patched(ParentDiff { c1, c2, c3, val })
            }
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    struct ParentDiff {
        c1: <Child1 as Diffable>::Diff,
        c2: <Vec<Child1> as Diffable>::Diff,
        c3: <HashMap<i32, Child2> as Diffable>::Diff,
        val: <String as Diffable>::Diff,
    }

    impl Apply for ParentDiff {
        type Parent = Parent;

        fn apply(self, source: &mut Self::Parent, errs: &mut Vec<ApplyError>) {
            self.c1.apply(&mut source.c1, errs);
            self.c2.apply(&mut source.c2, errs);
            self.c3.apply(&mut source.c3, errs);
            self.val.apply(&mut source.val, errs);
        }
    }

    impl Diffable for Child1 {
        type Diff = Diff<Self, Child1Diff>;

        fn diff(&self, other: &Self) -> Self::Diff {
            let x = self.x.diff(&other.x);
            let y = self.y.diff(&other.y);
            if x.is_unchanged() && y.is_unchanged() {
                Diff::Unchanged
            } else if x.is_replaced() && y.is_replaced() {
                let x = x.get_replaced().unwrap();
                let y = y.get_replaced().unwrap();
                Diff::Replaced(Self { x, y })
            } else {
                Diff::Patched(Child1Diff { x, y })
            }
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    struct Child1Diff {
        x: <i32 as Diffable>::Diff,
        y: <String as Diffable>::Diff,
    }

    impl Apply for Child1Diff {
        type Parent = Child1;

        fn apply(self, source: &mut Self::Parent, errs: &mut Vec<ApplyError>) {
            self.x.apply(&mut source.x, errs);
            self.y.apply(&mut source.y, errs);
        }
    }

    impl Diffable for Child2 {
        type Diff = Diff<Self, Child2Diff>;
        fn diff(&self, other: &Self) -> Self::Diff {
            let a = self.a.diff(&other.a);
            let b = self.b.diff(&other.b);
            let c = self.c.diff(&other.c);
            if a.is_unchanged() && b.is_unchanged() {
                Diff::Unchanged
            } else if a.is_replaced() && b.is_replaced() {
                let a = a.get_replaced().unwrap();
                let b = b.get_replaced().unwrap();
                let c = c.get_replaced().unwrap();
                Diff::Replaced(Self { a, b, c })
            } else {
                Diff::Patched(Child2Diff { a, b, c })
            }
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    struct Child2Diff {
        a: <String as Diffable>::Diff,
        b: <SomeChild as Diffable>::Diff,
        c: <() as Diffable>::Diff,
    }

    impl Apply for Child2Diff {
        type Parent = Child2;

        fn apply(self, source: &mut Self::Parent, errs: &mut Vec<ApplyError>) {
            self.a.apply(&mut source.a, errs);
            self.b.apply(&mut source.b, errs);
            Apply::apply(self.c, &mut source.c, errs);
        }
    }

    impl Diffable for SomeChild {
        type Diff = Diff<SomeChild, SomeChildDiff>;
        fn diff(&self, other: &Self) -> Self::Diff {
            match (self, other) {
                (Self::C1(left), Self::C1(right)) => {
                    let this = left.diff(right);
                    if this.is_unchanged() {
                        Diff::Unchanged
                    } else if this.is_replaced() {
                        Diff::Replaced(Self::C1(this.get_replaced().unwrap()))
                    } else {
                        Diff::Patched(SomeChildDiff::C1(this))
                    }
                }
                (Self::C2(left), Self::C2(right)) => {
                    let this = left.diff(right);
                    if this.is_unchanged() {
                        Diff::Unchanged
                    } else if this.is_replaced() {
                        Diff::Replaced(Self::C2(Box::new(this.get_replaced().unwrap())))
                    } else {
                        Diff::Patched(SomeChildDiff::C2(Box::new(this)))
                    }
                }
                _ => Diff::Replaced(other.clone()),
            }
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    enum SomeChildDiff {
        C1(<Child1 as Diffable>::Diff),
        C2(Box<<Child2 as Diffable>::Diff>),
    }

    impl Apply for SomeChildDiff {
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
            let diff = p1.diff(&p1);
            assert!(matches!(diff, Diff::Unchanged));
            p1.apply(diff).unwrap();
            assert_eq!(p1, base);
        }

        {
            let mut p3 = base.clone();
            let mut p4 = p3.clone();
            p4.val = "mello".into();
            let diff = p3.diff(&p4);
            let expect = Diff::Patched(ParentDiff {
                c1: Diff::Unchanged,
                c2: AtomicDiff::Unchanged,
                c3: Diff::Unchanged,
                val: AtomicDiff::Replaced("mello".into()),
            });
            assert_eq!(diff, expect);
            p3.apply(diff).unwrap();
        }

        {
            let mut p5 = base.clone();
            let bad_patch = Diff::Patched(ParentDiff {
                c1: Diff::Unchanged,
                c2: AtomicDiff::Unchanged,
                c3: Diff::Patched(
                    [
                        (543, KvDiff::Removed),                  // key does not exist
                        (321, KvDiff::Inserted(dummy_child2())), // key already exists
                    ]
                    .into_iter()
                    .collect(),
                ),
                val: AtomicDiff::Replaced("mello".into()),
            });
            let mut err = p5.apply(bad_patch).unwrap_err();
            err.sort();
            assert_eq!(err, [ApplyError::MissingKey, ApplyError::UnexpectedKey]);
        }
    }

    #[test]
    fn test_derive_simple_struct() {
        #[derive(Diffable, PartialEq, Debug)]
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
        #[derive(Diffable, PartialEq, Debug)]
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
