use std::{collections::HashMap, hash::Hash};

// when we diff, 3 alternative -

// *** Trait

pub trait Diffable: Sized {
    type Diff;

    fn diff(&self, other: &Self) -> Diff<Self>;
}

#[derive(Debug, Clone, PartialEq)]
pub enum Diff<T: Diffable> {
    Unchanged,
    Patch(T::Diff),
    Replace(T),
}

#[derive(Debug, Clone, PartialEq)]
pub enum KvDiff<T: Diffable> {
    Unchanged,
    Removed,
    Patch(T::Diff),
    Replace(T),
}

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
}

#[derive(Debug, Clone, PartialEq)]
enum SomeChild {
    C1(Child1),
    C2(Box<Child2>),
}

// *** Impls ***

impl Diffable for Parent {
    type Diff = ParentDiff;

    fn diff(&self, other: &Self) -> Diff<Self> {
        let c1 = self.c1.diff(&other.c1);
        let c2 = self.c2.diff(&other.c2);
        let c3 = self.c3.diff(&other.c3);
        let val = self.val.diff(&other.val);
        use Diff::*;
        match (c1, c2, c3, val) {
            (Unchanged, Unchanged, Unchanged, Unchanged) => Diff::Unchanged,
            (Replace(c1), Replace(c2), Replace(c3), Replace(val)) => {
                Diff::Replace(Self { c1, c2, c3, val })
            }
            (c1, c2, c3, val) => Diff::Patch(Self::Diff { c1, c2, c3, val }),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct ParentDiff {
    c1: Diff<Child1>,
    c2: Diff<Vec<Child1>>,
    c3: Diff<HashMap<i32, Child2>>,
    val: Diff<String>,
}

impl Diffable for Child1 {
    type Diff = Child1Diff;

    fn diff(&self, other: &Self) -> Diff<Self> {
        let x = self.x.diff(&other.x);
        let y = self.y.diff(&other.y);
        use Diff::*;
        match (x, y) {
            (Unchanged, Unchanged) => Diff::Unchanged,
            (Replace(x), Replace(y)) => Diff::Replace(Self { x, y }),
            (x, y) => Diff::Patch(Self::Diff { x, y }),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Child1Diff {
    x: Diff<i32>,
    y: Diff<String>,
}

impl Diffable for Child2 {
    type Diff = Child2Diff;
    fn diff(&self, other: &Self) -> Diff<Self> {
        todo!()
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Child2Diff {
    a: Diff<String>,
    b: Diff<SomeChild>,
}

impl Diffable for SomeChild {
    type Diff = MaybeChildDiff;
    fn diff(&self, other: &Self) -> Diff<Self> {
        todo!()
    }
}

#[derive(Debug, Clone, PartialEq)]
enum MaybeChildDiff {
    C1(<Child1 as Diffable>::Diff),
    C2(Box<<Child2 as Diffable>::Diff>),
}

impl<T> Diffable for Vec<T> {
    type Diff = ();

    fn diff(&self, other: &Self) -> Diff<Self> {
        Diff::Unchanged
    }
}

impl<K: Hash + Eq, V: Diffable> Diffable for HashMap<K, V> {
    type Diff = HashMap<K, Diff<V>>;

    fn diff(&self, other: &Self) -> Diff<Self> {
        let mut rtn = HashMap::new();
        for (k, v) in self.iter() {
            if let Some(other) = other.get(k) {
                match v.diff(other) {
                    Diff::Unchanged => {}
                    other => {
                        rtn.insert(k, other);
                    }
                }
            } else {
                rtn.insert(k, todo!());
            }
        }
        for (k, v) in other.iter() {
            if let Some(other) = other.get(k) {
                todo!()
            } else {
                todo!()
            }
        }
        todo!()
    }
}

impl Diffable for String {
    type Diff = Self;

    fn diff(&self, other: &Self) -> Diff<Self> {
        if self == other {
            Diff::Unchanged
        } else {
            Diff::Replace(other.clone())
        }
    }
}

impl Diffable for () {
    type Diff = ();

    fn diff(&self, _: &Self) -> Diff<Self> {
        Diff::Unchanged
    }
}

impl Diffable for i32 {
    type Diff = i32;

    fn diff(&self, other: &Self) -> Diff<Self> {
        if self == other {
            Diff::Unchanged
        } else {
            Diff::Replace(other.clone())
        }
    }
}

#[test]
fn smoke() {
    let p1 = Parent {
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
            },
        )]
        .into_iter()
        .collect(),
        val: "hello".into(),
    };

    {
        let p2 = p1.clone();
        let diff = p1.diff(&p2);
        assert!(matches!(diff, Diff::Unchanged));
    }

    {
        let mut p3 = p1.clone();
        p3.val = "mello".into();
        let diff = p1.diff(&p3);
        let expect = Diff::Patch(ParentDiff {
            c1: Diff::Unchanged,
            c2: Diff::Unchanged,
            c3: Diff::Unchanged,
            val: Diff::Replace("mello".into()),
        });
        assert_eq!(diff, expect);
    }
}
