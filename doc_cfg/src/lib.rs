#![cfg_attr(docsrs, feature(doc_cfg))]

#[cfg(feature = "foo")]
pub struct Foo;

#[cfg(feature = "bar")]
pub trait Bar {}
