#![doc = include_str!("../README.md")]
// this feature should be enabled for using `fake_variadic`
#![cfg_attr(docsrs, feature(rustdoc_internals))]

pub trait Foo {}

#[cfg_attr(docsrs, doc(fake_variadic))]
/// implemented for up to 1 F
impl<F> Foo for (F,) {}

// With fake_variadic, we can got
// ```text
// impl<F> Foo for (F₁, F₂, …, Fₙ)
// ```
//
// Check it with `cargo docs-rs -p fake_variadic --open`
