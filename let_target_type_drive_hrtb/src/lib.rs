#![allow(dead_code)]

use std::{pin::Pin, sync::Arc};

/// What an amazing type!
///
/// However, this is generally similar to [`TransactionFunction`](https://docs.rs/goose/latest/goose/goose/type.TransactionFunction.html)
/// in `goose`, a famous Rust load test crate.
///
/// This is known HRTB (higher rank trait bound).
///
/// It's a Box of fn, which returns pinned future, whose lifetime is expected to be at least
/// any possible lifetime of `&str`.
type FF =
    Box<dyn for<'a> Fn(&'a str) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> + Send + Sync>;

fn consume(_f: FF) {}

#[trait_variant::make(Send)]
trait AsF {
    async fn f(&self, _s: &str);
}

/// ```text ignore
/// error[E0282]: type annotations needed
///   --> amazing_explicit_type_coerces_needed/src/lib.rs:27:9
///    |
/// 27 |         move |s: &str| {
///    |         ^^^^^^^^^^^^^^
/// ...
/// 30 |             Box::pin(async move { t.f(s).await }) as _
///    |                                                      - type must be known at this point
///    |
/// help: try giving this closure an explicit return type
///    |
/// 27 |         move |s: &str| -> /* Type */ {
///    |                        +++++++++++++
/// ```
/// In order to make users not to write such tedious code, we defined `AsF` trait
/// and make full use of closure to capture things like T, so that the closure is FF.
#[cfg(feature = "error")]
fn bad1<T>(t: T)
where
    T: AsF + Send + Sync + 'static,
{
    let t = Arc::new(t);
    let f = Box::new({
        move |s: &str| {
            // Clone first, so that `f: Fn` instead of `f: FnOnce()`
            let t = t.clone();
            // error happens here:
            // type hint is needed, or `Pin<Box<impl Future>>`
            // cannot be coerced
            Box::pin(async move { t.f(s).await }) as _
        }
    }) as _;
    consume(f);
}

/// After add type hint, error changed to
/// ```text ignore
/// error: lifetime may not live long enough
///   --> amazing_explicit_type_coerces_needed/src/lib.rs:58:13
///    |
/// 55 |         move |s: &str| -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
///    |                  -        --------------------------------------------- return type of closure is Pin<Box<(dyn Future<Output = ()> + Send + '2)>>
///    |                  |
///    |                  let's call the lifetime of this reference `'1`
/// ...
/// 58 |             Box::pin(async move { t.f(&s).await })
///    |             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ returning this value requires that `'1` must outlive `'2`
///
/// error[E0308]: mismatched types
///   --> amazing_explicit_type_coerces_needed/src/lib.rs:54:13
///    |
/// 54 |       let f = Box::new({
///    |  _____________^
/// 55 | |         move |s: &str| -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
/// 56 | |             // Clone first, so that `f: Fn` instead of `f: FnOnce()`
/// 57 | |             let t = t.clone();
/// ...  |
/// 60 | |     }) as FF;
///    | |____________^ one type is more general than the other
///    |
///    = note: expected struct `Pin<Box<dyn Future<Output = ()> + Send>>`
///               found struct `Pin<Box<(dyn Future<Output = ()> + Send + 'a)>>`
/// ```
///
/// The key point here is `one type is more general than the other`.
/// `f` is expected to return Future which can be used with any possiable lifetime,
/// but with `as` coerced, the lifetime of `f` is inferred as some concrete `'a`.
///
/// In other words, `f` is expected to be more general than that have been coerced.
#[cfg(feature = "error")]
fn bad2<T>(t: T)
where
    T: AsF + Send + Sync + 'static,
{
    let t = Arc::new(t);
    let f = Box::new({
        move |s: &str| -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
            // Clone first, so that `f: Fn` instead of `f: FnOnce()`
            let t = t.clone();
            Box::pin(async move { t.f(&s).await })
        }
    }) as FF; // coerced here
    consume(f);
}

/// This is correct solution.
fn good<T>(t: T)
where
    T: AsF + Send + Sync + 'static,
{
    let t = Arc::new(t);
    // The key point is here, instead use `as` to coerce things,
    // we claim the target type to let it drive HRTB.
    let f: FF = Box::new({
        move |s: &str| {
            // Clone first, so that `f: Fn` instead of `f: FnOnce()`
            let t = t.clone();
            Box::pin(async move { t.f(s).await })
        }
    });
    consume(f);
}

// Compare `bad1` and `bad2`, it is `as` cannot be handled correctly,
// which is not the `real` error.
//
// Compare `bad2` and `good`, we can draw a conclusion:
// - `t as T` means coercing `t` to `T` based on `t`'s lifetime
// - `t: T = ...` where `T` has HRTB means that `...` is claimed to be `T`,
// as to whether it's actually `T` or not, the compiler would judge it.
//
// So, when working with HRTB, it's better to use target type to drive things.
