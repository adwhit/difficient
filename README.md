
# Difficient

Efficient type-safe diffing.


```rust
#[derive(difficient::Diffable, PartialEq, Debug, Clone)]
enum SimpleEnum {
    First,
    Second(i32),
    Third { x: String, y: (), z: SimpleStruct },
}

#[derive(difficient::Diffable, PartialEq, Debug, Clone)]
struct SimpleStruct {
    x: String,
    y: i32,
}
```
