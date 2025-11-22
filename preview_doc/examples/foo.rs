//! This is an example waiting to be checked if its documentation is generated correctly for docsrs

#![cfg_attr(docsrs, feature(rustdoc_internals))]

fn main() {}

pub trait Foo {}

#[cfg_attr(docsrs, doc(fake_variadic))]
/// implemented for up to 1 F
impl<F> Foo for (F,) {}
