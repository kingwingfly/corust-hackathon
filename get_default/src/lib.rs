#![allow(clippy::needless_lifetimes, unused)]

use std::{collections::HashMap, hash::Hash};

/// # Error message
/// ```
/// error[E0499]: cannot borrow `*map` as mutable more than once at a time
///   --> src/lib.rs:11:13
///    |
///  3 |   fn wrong_get_default<'m, K, V>(map: &'m mut HashMap<K, V>, key: K) -> &'m mut V
///    |                        -- lifetime `'m` defined here
/// ...
///  8 |       match map.get_mut(&key) {
///    |       -     --- first mutable borrow occurs here
///    |  _____|
///    | |
///  9 | |         Some(value) => value, // return 'm, `&mut map: 'm`
/// 10 | |         None => {
/// 11 | |             map.insert(key.clone(), V::default());
///    | |             ^^^ second mutable borrow occurs here
/// ...  |
/// 14 | |     }
///    | |_____- returning this value requires that `*map` is borrowed for `'m`
///
/// error[E0499]: cannot borrow `*map` as mutable more than once at a time
///   --> src/lib.rs:12:13
///    |
///  3 |   fn wrong_get_default<'m, K, V>(map: &'m mut HashMap<K, V>, key: K) -> &'m mut V
///    |                        -- lifetime `'m` defined here
/// ...
///  8 |       match map.get_mut(&key) {
///    |       -     --- first mutable borrow occurs here
///    |  _____|
///    | |
///  9 | |         Some(value) => value, // return 'm, `&mut map: 'm`
/// 10 | |         None => {
/// 11 | |             map.insert(key.clone(), V::default());
/// 12 | |             map.get_mut(&key).unwrap() // return 'm, `&mut map: 'm`
///    | |             ^^^ second mutable borrow occurs here
/// 13 | |         }
/// 14 | |     }
///    | |_____- returning this value requires that `*map` is borrowed for `'m`
///
/// For more information about this error, try `rustc --explain E0499`.
/// ```
///
/// # Explain
///
/// Below is desugarred function signature of `HashMap::get_mut`:
/// ```
/// fn get_mut<'a>(&'a mut self, k: &Q) -> Option<&'a mut V>;
/// ```
///
/// In branch `Some(value) => value` where `value: &'m mut V`, the life time of `value` is inferred as `'m`,
/// so that the the `map.get_mut<'a>(&key)` in `match map.get_mut(&key)` is actully inffered as `map.get_mut<'m>()`.
/// That is to say, we borrow `*map` as mutable from `match map.get_mut(&key)` to return.
///
/// Inside branch `None`, there's also a `map.get_mut<'a>(&key)`, however, we have borrowed `*map` as mutable before
/// in `match map.get_mut(&key)`, the compiler is not smart enough, so error raises.
///
/// # More Info
///
/// You can also fix by `RUSTFLAGS="-Zpolonius=next" cargo +nightly check -F err`.
///
/// - [Polonius update](https://blog.rust-lang.org/inside-rust/2023/10/06/polonius-update/)
/// - [An alias-based formulation of the borrow checker](https://smallcultfollowing.com/babysteps/blog/2018/04/27/an-alias-based-formulation-of-the-borrow-checker/)
#[cfg(feature = "err")]
fn wrong_get_default<'m, K, V>(map: &'m mut HashMap<K, V>, key: K) -> &'m mut V
where
    K: Clone + Eq + Hash,
    V: Default,
{
    match map.get_mut(&key) {
        Some(value) => value,
        None => {
            map.insert(key.clone(), V::default());
            map.get_mut(&key).unwrap()
        }
    }
}

/// `HashMap::entry` is all you need
fn correct_get_default<'m, K, V>(map: &'m mut HashMap<K, V>, key: K) -> &'m mut V
where
    K: Clone + Eq + Hash,
    V: Default,
{
    map.entry(key).or_default()
}
