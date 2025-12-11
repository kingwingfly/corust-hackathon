#![allow(dead_code)]

use std::marker::PhantomData;

/// `cargo expand -p incorrect_clone_derived -F incorrect`:
/// ```rust
/// #[automatically_derived]
/// impl<T: ::core::clone::Clone> ::core::clone::Clone for Foo<T> {
///     #[inline]
///     fn clone(&self) -> Foo<T> {
///         Foo {
///             _phantom: ::core::clone::Clone::clone(&self._phantom),
///         }
///     }
/// }
/// ```
///
/// It needs `T: Clone` so that `Foo<T>: Clone`, while `T` is actually
/// in `PhantomData`. `Foo<T>` should be `Clone` even though `T` is not.
#[derive(Debug)]
#[cfg_attr(feature = "incorrect", derive(Clone))]
struct Foo<T> {
    _phantom: PhantomData<T>,
}

/// Instead of using proc macro, we shall implemente `Clone` by our own.
#[cfg(not(feature = "incorrect"))]
impl<T> Clone for Foo<T> {
    fn clone(&self) -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}
