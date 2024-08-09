// ** Test structs

use std::collections::HashMap;

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
#[derive(difficient::Diffable, PartialEq, Debug, Clone)]
struct SimpleStruct {
    x: String,
    y: i32,
}

#[derive(difficient::Diffable, PartialEq, Debug, Clone)]
struct StrangeStruct {
    r#try: Option<Box<(u32, (&'static str, Box<u64>))>>,
}

#[derive(difficient::Diffable, PartialEq, Debug, Clone)]
struct Unit;

#[derive(difficient::Diffable, PartialEq, Debug, Clone)]
struct Tuple(Vec<&'static str>, i32);

#[derive(difficient::Diffable, PartialEq, Debug, Clone)]
enum SimpleEnum {
    First,
    Second(i32),
    Third { x: String, y: () },
}

// **** Derive tests
mod tests {
    use super::*;

    use difficient::Diffable;

    #[test]
    fn test_simple_struct() {
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
    fn test_less_simple_struct() {
        let mut it1 = StrangeStruct {
            r#try: Some(Box::new((123, ("ick", Box::new(543))))),
        };
        let it2 = StrangeStruct {
            r#try: Some(Box::new((123, ("flick", Box::new(543))))),
        };
        let diff = it1.diff(&it2);
        it1.apply(diff).unwrap();
        assert_eq!(it1, it2);
    }

    #[test]
    fn test_unit_struct() {
        let mut it1 = Unit;
        let it2 = Unit;
        let diff = it1.diff(&it2);
        it1.apply(diff).unwrap();
        assert_eq!(it1, it2);
    }

    #[test]
    fn test_tuple_struct() {
        let mut it1 = Tuple(vec!["first", "second"], 123);
        let it2 = Tuple(vec!["second", "third"], 123);
        let diff = it1.diff(&it2);
        it1.apply(diff).unwrap();
        assert_eq!(it1, it2);
    }

    #[test]
    fn test_simple_enum() {
        let mut it1 = SimpleEnum::First;
        let mut it2 = SimpleEnum::Second(123);
        let mut it3 = SimpleEnum::Third {
            x: "work work".into(),
            y: (),
        };
        let it4 = SimpleEnum::Third {
            x: "twork".into(),
            y: (),
        };

        {
            let diff = it1.diff(&it2);
            it1.apply(diff).unwrap();
            assert_eq!(it1, it2);
        }

        {
            let diff = it2.diff(&it3);
            it2.apply(diff).unwrap();
            assert_eq!(it2, it3);
        }

        {
            let diff = it3.diff(&it4);
            it3.apply(diff).unwrap();
            assert_eq!(it3, it4);
        }
    }
}
