//! In many cases, we need proc-macros to simpify the code.
//!
//! e.g.
//! - Serialize and Deserialize in serde
//! - Debug in std
//! - Pod in bytemuck
//! - FromBytes in zerocopy
//! - Component in bevy
//! - Zeroize and ZeroizeOnDrop in zeroize
//!
//! They are all widely used and core foundation of Rust crate ecosystem.
//!
//! However, just knowing how to use them is always far away from enough.
//! Without the ability to write your own, one could never fully master Rust and make maintainable Rust project.
//!
//! In this example, we aiming to implemente a proc-macro named `BorrowKey`. (And this example has been published as `borrow_key` on cratesio)
//! This macro should allow a struct to be borrowed as a reference to one of its fields, called `key`, based on `core::borrow::Borrow`.
//! And it should also ensure that Eq, Ord, and Hash are implemented correctly according to the documentation requirements below,
//! so that this struct can be used in `HashSet` correctly:
//!
//! > Further, when providing implementations for additional traits,
//! > it needs to be considered whether they should behave
//! > identically to those of the underlying type as a consequence of acting as a representation of that underlying type.
//! > Generic code typically uses `Borrow<T>` when it relies on the identical behavior of these additional trait implementations.
//! > These traits will likely appear as additional trait bounds.
//! >
//! > In particular Eq, Ord and Hash must be equivalent for borrowed and owned values: x.borrow() == y.borrow() should give the same result as x == y.
//! >
//! > If generic code merely needs to work for all types that can provide a reference to related type T,
//! > it is often better to use `AsRef<T>` as more types can safely implement it.
//!
//! For more information about why this is needed to be used in HashSet, see <https://github.com/kingwingfly/corust-hackathon/blob/dev/hashmap_but_key_ref_to_value/src/lib.rs>

#![allow(dead_code)]

use proc_macro::TokenStream;
// like format!, but quote! is used to format TokenStream
use quote::quote;
// Rust TokenStream parser based on proc_macro2
use syn::{Data, DeriveInput, Error, Ident, Type, parse_macro_input, spanned::Spanned};

/// # Example
/// ```
/// use proc_macro_example::BorrowKey;
///
/// #[derive(BorrowKey)]
/// struct Foo {
///     #[key(str)]
///     key: String,
/// }
///
/// #[derive(BorrowKey)]
/// struct Bar {
///     #[key]
///     key: String,
/// }
/// ```
#[proc_macro_derive(BorrowKey, attributes(key /* used to mark key field */))] // this is need to define the proc-macro-derive
pub fn derive_borrow_key(input: TokenStream) -> TokenStream {
    // `parse_macro_input` can parse the token stream into syn::DeriveInput struct.
    let input = parse_macro_input!(input as DeriveInput);
    // split generics into things implemented ToTokens
    let (
        impl_generics, /* generic after Token![impl] */
        ty_generics,   /* generic after ident */
        where_clause,  /* where */
    ) = input.generics.split_for_impl();
    // the ident of the struct to be implemented
    let ident = input.ident;

    // the key field ident, leaving it None initially
    let mut key_ident = None::<Ident>;
    // the type to be borrowed as
    let mut key_type = None::<Type>;

    match input.data /* the field in the struct */ {
        Data::Struct(data_struct) => {
            // iterate over all fields
            for field in data_struct.fields {
                // only handle key attribute
                if let Some(attr) = field.attrs.iter().find(|a| a.meta.path().is_ident("key")) {
                    if key_ident.is_some() || key_type.is_some() {
                        // key has been set, refuse to set again
                        return Error::new(
                            attr.span(),
                            "`BorrowKey`: expect exact 1 key to be specified",
                        )
                        .to_compile_error() // this is proc_macro2::TokenStream
                        .into(); // into proc_macro::TokenStream
                    }
                    if field.ident.is_none() {
                        // this means its tuple struct, which does not need our proc-macro
                        return Error::new(
                            field.span(),
                            "`BorrowKey`: tuple struct does not need this proc-macro",
                        )
                        .to_compile_error()
                        .into();
                    }
                    key_ident = field.ident;    // set key ident
                    key_type = match attr.parse_args() {
                        Ok(r#type) => Some(r#type),
                        Err(_) => Some(field.ty),   // if type to be borrowed as is not set, use field type directly
                    }
                }
            }
        }
        // return Error if not supported
        Data::Enum(data_enum) => {
            return Error::new(
                data_enum.enum_token.span(),
                "`BorrowKey`: enum type is not supported",
            )
            .to_compile_error()
            .into();
        }
        // return Error if not supported
        Data::Union(data_union) => {
            return Error::new(
                data_union.union_token.span(),
                "`BorrowKey`: union type is not supported",
            )
            .to_compile_error()
            .into();
        }
    }

    // check if the key is set
    if key_ident.is_none() || key_type.is_none() {
        return Error::new(
            ident.span(),
            "`BorrowKey`: expect exact 1 key to be specified with #[key($type?: ty)]",
        )
        .to_compile_error()
        .into();
    }

    // use quote to format the TokenStream.
    // The reason to format into this can be found in <https://github.com/kingwingfly/corust-hackathon/blob/dev/hashmap_but_key_ref_to_value/src/lib.rs>
    let expanded = quote! {
        impl #impl_generics ::core::borrow::Borrow<#key_type> for #ident #ty_generics #where_clause {
            fn borrow(&self) -> &#key_type {
                &self.#key_ident
            }
        }

        impl #impl_generics ::core::hash::Hash for #ident #ty_generics #where_clause {
            fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
                self.#key_ident.hash(state);
            }
        }

        impl #impl_generics ::core::cmp::PartialEq for #ident #ty_generics #where_clause {
            fn eq(&self, other: &Self) -> bool {
                self.key == other.key
            }
        }

        impl #impl_generics ::core::cmp::Eq for #ident #ty_generics #where_clause { }

        impl #impl_generics ::core::cmp::PartialOrd for #ident #ty_generics #where_clause {
            fn partial_cmp(&self, other: &Self) -> Option<::core::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }

        impl #impl_generics ::core::cmp::Ord for #ident #ty_generics #where_clause {
            fn cmp(&self, other: &Self) -> ::core::cmp::Ordering {
                self.key.cmp(&other.key)
            }
        }

    };

    // convert from proc_macro2::TokenStream into proc_macro::TokenStream
    TokenStream::from(expanded)
}
