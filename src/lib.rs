use std::{collections::HashMap, hash::Hash};

// *** Trait

type Result<T> = std::result::Result<T, ApplyError>;

pub trait Diffable: Sized {
    type Diff: Replace<Replaces = Self> + Apply<Parent = Self>;
    fn diff(&self, other: &Self) -> Self::Diff;
    fn apply(self, diff: Self::Diff) -> Result<Self> {
        diff.apply(self)
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
    fn apply(self, source: Self::Parent) -> Result<Self::Parent>;
}

pub enum ApplyError {
    EnumDidNotApply,
}

impl std::fmt::Debug for ApplyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl std::fmt::Display for ApplyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        todo!()
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
    fn apply(self, source: Self::Parent) -> Result<Self::Parent> {
        Ok(match self {
            AtomicDiff::Unchanged => source,
            AtomicDiff::Replaced(r) => r,
        })
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

    fn apply(self, source: Self::Parent) -> Result<Self::Parent> {
        let patch = match self {
            Diff::Unchanged => return Ok(source),
            Diff::Patched(p) => p,
            Diff::Replaced(r) => return Ok(r),
        };
        patch.apply(source)
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

impl<K, V: Diffable> Apply for HashMap<K, KvDiff<V>> {
    type Parent = HashMap<K, V>;

    fn apply(self, source: Self::Parent) -> Result<Self::Parent> {
        todo!()
    }
}

impl Diffable for () {
    type Diff = ();

    fn diff(&self, _: &Self) -> Self::Diff {
        ()
    }

    fn apply(self, (): Self::Diff) -> Result<Self> {
        Ok(())
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

    fn apply(self, _: Self::Parent) -> Result<Self::Parent> {
        Ok(())
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

impl Apply for ParentDiff {
    type Parent = Parent;

    fn apply(self, source: Self::Parent) -> Result<Self::Parent> {
        Ok(Self::Parent {
            c1: self.c1.apply(source.c1)?,
            c2: self.c2.apply(source.c2)?,
            c3: self.c3.apply(source.c3)?,
            val: self.val.apply(source.val)?,
        })
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
    x: AtomicDiff<i32>,
    y: AtomicDiff<String>,
}

impl Apply for Child1Diff {
    type Parent = Child1;

    fn apply(self, source: Self::Parent) -> Result<Self::Parent> {
        Ok(Self::Parent {
            x: self.x.apply(source.x)?,
            y: self.y.apply(source.y)?,
        })
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

    fn apply(self, source: Self::Parent) -> Result<Self::Parent> {
        Ok(Self::Parent {
            a: self.a.apply(source.a)?,
            b: self.b.apply(source.b)?,
            c: Apply::apply(self.c, source.c)?,
        })
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

    fn apply(self, source: Self::Parent) -> Result<Self::Parent> {
        todo!()
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
        assert_eq!(p1.clone().apply(diff).unwrap(), p2);
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
        assert_eq!(p1.clone().apply(diff).unwrap(), p3);
    }
}
