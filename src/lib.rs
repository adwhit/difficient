use std::{collections::HashMap, hash::Hash};

// *** Trait

pub trait Diffable: Sized {
    type Diff: Replace<Replaces = Self>;

    fn diff(&self, other: &Self) -> Self::Diff;
}

pub trait Replace {
    type Replaces;

    fn is_unchanged(&self) -> bool;
    fn is_replaced(&self) -> bool;
    fn get_replaced(self) -> Option<Self::Replaces>;
}

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

#[derive(Debug, Clone, PartialEq)]
pub enum KvDiff<T, U> {
    Unchanged,
    Removed,
    Patch(U),
    Replace(T),
}

// ** Common impls ***

impl<K: Hash + Eq, V: Diffable> Diffable for HashMap<K, V> {
    type Diff = Diff<Self, HashMap<K, KvDiff<V, V::Diff>>>;

    fn diff(&self, other: &Self) -> Self::Diff {
        let mut rtn = HashMap::new();
        for (k, v) in self.iter() {
            if let Some(other) = other.get(k) {
                let diff = v.diff(other);
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
    type Diff = AtomicDiff<Self>;

    fn diff(&self, other: &Self) -> Self::Diff {
        if self == other {
            AtomicDiff::Unchanged
        } else {
            AtomicDiff::Replaced(other.clone())
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

impl Diffable for i32 {
    type Diff = AtomicDiff<Self>;

    fn diff(&self, other: &Self) -> Self::Diff {
        if self == other {
            AtomicDiff::Unchanged
        } else {
            AtomicDiff::Replaced(other.clone())
        }
    }
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
        } else if c1.is_replaced() && c2.is_replaced() && c3.is_replaced() && val.is_replaced() {
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
    x: AtomicDiff<i32>,
    y: AtomicDiff<String>,
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

#[test]
fn smoke_test() {
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
                c: (),
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
        let expect = Diff::Patched(ParentDiff {
            c1: Diff::Unchanged,
            c2: AtomicDiff::Unchanged,
            c3: Diff::Unchanged,
            val: AtomicDiff::Replaced("mello".into()),
        });
        assert_eq!(diff, expect);
    }
}
