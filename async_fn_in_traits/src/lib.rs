//! https://blog.rust-lang.org/2023/12/21/async-fn-rpit-in-traits/

#![allow(dead_code)]
#![feature(return_type_notation)]

use std::{marker::PhantomData, pin::Pin};

fn assert_send<T: Send>(_: T) {}

trait Foo {
    // equal to `foo(&self) -> impl Future<..>`
    // not `impl Future<..> + Send`
    async fn foo(&self) {}
}

// A is Send
struct A(u8);

impl Foo for A {}

fn handle_a(a: A) {
    // no err!!!
    // ??? Why a.foo() is Send ???
    assert_send(a.foo());
}

// B is !Send
struct B(PhantomData<*mut u8>);

impl Foo for B {}

#[cfg(feature = "err")]
fn handle_b(b: B) {
    // err
    // b.foo() is not Send, correct
    assert_send(b.foo());
}

#[cfg(feature = "err")]
fn handle_foo<F>(f: F)
where
    F: Foo + Send,
{
    // err
    // although F is Send, f.foo() is not Send, correct
    assert_send(f.foo());
}

// In `handle_b`, `b.foo()` is not `Send`, in `handle_foo`, `f.foo()` is not `Send`, this is expected.
// No matter `b`/`f` is `Send` or not, the future is not `Send`.
//
// But in `handle_a`, `a.foo()` is surprisingly `Send`, this confuses me a lot.
// Compared `A` and `B`, I can assume that `a.foo()` is `Send` since `A` is `Send`.
// But `f.foo()` is not `Send` even `f` is `Send`, so the assumption is definitely wrong.
//
// What is the key point here, what makes `a.foo()` Send?
//
// The answer can be found in this Rust Forum thread: https://users.rust-lang.org/t/question-about-async-fn-in-traits-return-type-is-send-or-not/134613
//
// The compiler is more smart than we think, it can infer future evaluated by `A::foo()` is Send,
// while that of `B::foo()` is not.
// And `f: Send` cannot ensure `f.foo()` is Send, since Rc/Cell may be used in `f.foo`.
//
// Here are many solutions to fix `handle_foo`, let me show you.

/// Solution 1, explicitly mark return type of async fn in trait `Send`.
trait Bar {
    fn bar(&self) -> impl Future<Output = ()> + Send {
        async {}
    }
}

fn handle_bar<B>(b: B)
where
    B: Bar,
{
    assert_send(b.bar());
}

/// Solution 2, use trait-variant crate to simplify method 1.
///
/// This proc-macro, `make(Baz: Send)`,
/// generates a new trait named `Baz`, whose all return types of async fn in `LocalBaz`
/// are `impl Future<..> + Send`.
///
/// The drawback of this solution:
/// - it makes **all** return types of fn in trait `Send`
#[trait_variant::make(Baz: Send)]
trait LocalBaz {
    async fn baz1(&self);

    /// If with default implementation, manually desugar it
    fn baz2(&self) -> impl Future<Output = ()> {
        async {}
    }
}

fn handle_baz<B>(b: B)
where
    B: Baz,
{
    assert_send(b.baz1());
    assert_send(b.baz2());
}

/// Solution 3, use async-trait
///
/// This proc-macro, `async_trait`,
/// generates a new trait named `Qux`, whose all return types of async fn in it
/// are `Pin<Box<dyn Future<Output = ()> + Send + '_>>`.
///
/// p.s. Pin is used since `Future::poll`'s first arg is `Pin<&mut Self>`.
///
/// The drawback of this solution are:
/// - dyn Future, leads vtable consumption
/// - it makes **all** return types of fn in trait `Send`, and we may not need this
///
/// By the way, it generate trait which is dyn-compatible, which means you can use `dyn Qux`.
#[async_trait::async_trait]
trait Qux {
    async fn qux1(&self);

    /// If with default implementation, manually use `Pin<Box<dyn Future<Output = ()> + Send>>` as return type,
    /// this is exactly what async_trait proc-macro does.
    fn qux2(&self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(async {})
    }
}

fn handle_qux<Q>(b: Q)
where
    Q: Qux,
{
    assert_send(b.qux1());
    assert_send(b.qux2());
}

fn handle_dyn_qux(b: &dyn Qux) {
    assert_send(b.qux1());
    assert_send(b.qux2());
}

/// Solution 4, use dynosaur crate.
///
/// In many cases, we do need function like `handle_qux` above,
/// but it generate code for all possible `Q: Qux`.
/// And `handle_dyn_qux` become more useful and cheap.
///
/// But as mentioned above, async fn desugar to `impl Future<..>` which is not dyn-compatible,
/// while async-trait proc-macro solution has vtable consumption.
///
/// If `async_trait` is used, the dyn consumption is unavoidable.
/// Then `dynosaur` crate is used to overcome these drawbacks.
#[trait_variant::make(Corge: Send)]
#[dynosaur::dynosaur(DynLocalcorge = dyn(box) LocalCorge, bridge(dyn))]
#[dynosaur::dynosaur(Dyncorge = dyn(box) Corge, bridge(dyn))]
trait LocalCorge {
    async fn corge(&self);
}

/// It generate code for all possible `C: Corge`, including DynCorge.
///
/// Compared with `async_trait` solution, it won't include things like `Pin<Box<dyn ..>>`.
fn handle_corge<C>(c: C)
where
    C: Corge,
{
    assert_send(c.corge());
}

/// Has vtable consumption.
///
/// Compared with `trait_variant::make` solution,
/// it supports dyn dispatch to some extent by generated `Dyncorge` (wrapper of `dyn ErasedCorge`).
fn handle_dyn_corge(c: &Dyncorge) {
    assert_send(c.corge());
}

/// Solution 5, nightly feature RTN.
///
/// The RFC of RTN is https://rust-lang.github.io/rfcs/3654-return-type-notation.html
fn handle_nightly_foo<F>(f: F)
where
    F: Foo<foo(..): Send>,
{
    assert_send(f.foo());
}

#[test]
fn test() {
    handle_nightly_foo(A(0));
    #[cfg(feature = "err")]
    handle_nightly_foo(B(PhantomData::default()));
    // With nightly feature `RTN`, we can talk something like `F::foo(..): Send` in trait bound.
}
