#![allow(dead_code)]

use std::{
    borrow::Borrow,
    collections::{HashMap, HashSet},
    hash::{Hash, Hasher},
    rc::Rc,
};

// Sometimes, we need a HashMap, whose key is a reference to certain field of value.
//
// e.g. HashMap<&str, Foo>, whose key `&str` is reference to `key` in `Foo { key: String }`.

/// Define Foo
struct Foo {
    key: String,
}

/// This is a failed attempt.
/// ```ignore
/// error[E0505]: cannot move out of `foo1` because it is borrowed
///   --> hashmap_but_key_ref_to_value/src/lib.rs:28:36
///    |
/// 21 |     let foo1 = Foo {
///    |         ---- binding `foo1` declared here
/// ...
/// 28 |     foos.insert(foo1.key.as_str(), foo1);
///    |          ------ --------           ^^^^ move out of `foo1` occurs here
///    |          |      |
///    |          |      borrow of `foo1.key` occurs here
///    |          borrow later used by call
///    |
/// help: consider cloning the value if the performance cost is acceptable
///    |
/// 28 |     foos.insert(foo1.key.clone().as_str(), foo1);
///    |                         ++++++++
///
/// error[E0505]: cannot move out of `foo2` because it is borrowed
///   --> hashmap_but_key_ref_to_value/src/lib.rs:29:36
///    |
/// 24 |     let foo2 = Foo {
///    |         ---- binding `foo2` declared here
/// ...
/// 29 |     foos.insert(foo2.key.as_str(), foo2);
///    |          ------ --------           ^^^^ move out of `foo2` occurs here
///    |          |      |
///    |          |      borrow of `foo2.key` occurs here
///    |          borrow later used by call
///    |
/// help: consider cloning the value if the performance cost is acceptable
///    |
/// 29 |     foos.insert(foo2.key.clone().as_str(), foo2);
///    |                         ++++++++
///
/// For more information about this error, try `rustc --explain E0505`.
/// error: could not compile `hashmap_but_key_ref_to_value` (lib test) due to 2 previous errors
/// ```
#[cfg(feature = "err")]
#[test]
fn test1() {
    let foo1 = Foo {
        key: "hello".to_string(),
    };
    let foo2 = Foo {
        key: "world".to_string(),
    };
    let mut foos = HashMap::new();
    foos.insert(foo1.key.as_str(), foo1);
    foos.insert(foo2.key.as_str(), foo2);
}

// As the compiler figures out, foo.key is referred while it's also move into HashMap.
//
// But it suggest cloning the key, which is not what we want.

struct Bar {
    key: Rc<String>,
}

/// Then we can naturally solve it by making the clone operation cheaper
/// with `Rc` or `Arc`.
fn test2() {
    let bar1 = Bar {
        key: Rc::new("hello".to_string()),
    };
    let bar2 = Bar {
        key: Rc::new("world".to_string()),
    };
    let mut bars = HashMap::new();
    bars.insert(bar1.key.clone(), bar1);
    bars.insert(bar2.key.clone(), bar2);
    // error[E0277]: the trait bound `Rc<String>: Borrow<str>` is not satisfied
    //    --> hashmap_but_key_ref_to_value/src/lib.rs:99:14
    //     |
    //  99 |     bars.get("hello").unwrap();
    //     |          --- ^^^^^^^ the trait `Borrow<str>` is not implemented for `Rc<String>`
    //     |          |
    //     |          required by a bound introduced by this call
    //     |
    //     = help: the trait `Borrow<str>` is not implemented for `Rc<String>`
    //             but trait `Borrow<String>` is implemented for it
    //     = help: for that trait implementation, expected `String`, found `str`
    #[cfg(feature = "err")]
    bars.get("hello").unwrap();
    // unnecessary use of to_string
    // for further information visit https://rust-lang.github.io/rust-clippy/master/index.html#unnecessary_to_owned
    // #[warn(clippy::unnecessary_to_owned)] on by default (clippy unnecessary_to_owned)
    #[cfg(feature = "err")]
    bars.get(&"hello".to_string()).unwrap();
    // `HashMap::<K, _>::get(&Q)` needs `K: Borrow<Q>`,
    // here, needs `Rc<String>: Borrow<str>`, but `Rc<String>` only `Borrow<String>`,
    // and there's no `impl<T: Borrow<U>, U: Borrow<V>, V> Borrow<V> for T` in std,
    // > In particular Eq, Ord and Hash must be equivalent for borrowed and owned values:
    // > x.borrow() == y.borrow() should give the same result as x == y.
}

// We all believe that smart pointer can be replaced with unsafe Rust for better performance,
// and Rust pros like you may already try to solve this with unsafe.
//
// However, you can simply solve this with Hash trait and by using HashSet instead.
//
// With these tips, you will definitely know how to do it.
// But we all know, The simplest way is not always the easiest to come by.

struct Baz {
    key: String,
    // other fields are omitted
}

impl Borrow<str> for Baz {
    fn borrow(&self) -> &str {
        self.key.as_str()
    }
}

// > In particular Eq, Ord and Hash must be equivalent for borrowed and owned values:
// > x.borrow() == y.borrow() should give the same result as x == y.
//
// You can also use my published crate named `borrow_key` to simplify this.

impl Hash for Baz {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.key.hash(state);
    }
}

impl PartialEq for Baz {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

impl Eq for Baz {}

impl PartialOrd for Baz {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Baz {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.key.cmp(&other.key)
    }
}

fn test3() {
    let baz1 = Baz {
        key: "hello".to_string(),
    };
    let baz2 = Baz {
        key: "world".to_string(),
    };
    let mut bazs = HashSet::new();
    bazs.insert(baz1);
    bazs.insert(baz2);
    bazs.get("hello").unwrap();
}
