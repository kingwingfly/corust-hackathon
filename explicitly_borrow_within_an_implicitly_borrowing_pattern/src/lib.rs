#![allow(dead_code)]

#[derive(Debug)]
struct Foo {
    a: String,
    b: String,
}

fn func(f: &mut Foo) {
    // Here,
    // a: &mut String
    // a: &mut String
    let Foo { a, b } = f;
    // However, we actually only need
    // a: &String instead of &mut String
    *b = a.clone();
}

// It just works. However, if we just let it go,
// we'll meet lifetime problem some day in the future.

#[cfg(feature = "error")]
fn failed_fix(f: &mut Foo) {
    // We natually get used to try `ref` key word
    // to tell Rust we only want immutable reference for `a`.
    let Foo { ref a, b } = f;
    // It almost succeed.
    // At least Rust Analyzer inferred the type we want.
    // However, compiler compains:
    // ```text
    // error: cannot explicitly borrow within an implicitly-borrowing pattern
    //   --> explicitly_borrow_within_an_implicitly_borrowing_pattern/src/lib.rs:25:15
    //    |
    // 25 |     let Foo { ref a, b } = f;
    //    |               ^^^ explicit `ref` binding modifier not allowed when implicitly borrowing
    //    |
    //    = note: for more information, see <https://doc.rust-lang.org/reference/patterns.html#binding-modes>
    // note: matching on a reference type with a non-reference pattern implicitly borrows the contents
    //   --> explicitly_borrow_within_an_implicitly_borrowing_pattern/src/lib.rs:25:9
    //    |
    // 25 |     let Foo { ref a, b } = f;
    //    |         ^^^^^^^^^^^^^^^^ this non-reference pattern matches on a reference type `&mut _`
    // help: match on the reference with a reference pattern and borrow explicitly using a variable binding mode
    //    |
    // 25 |     let &mut Foo { ref a, ref mut b } = f;
    //    |         ++++              +++++++
    //
    // error: could not compile `explicitly_borrow_within_an_implicitly_borrowing_pattern` (lib) due to 1 previous error
    // ```
    *b = a.clone();
}

fn anyway_worked_fix(f: &mut Foo) {
    // We follow what compiler told us
    let &mut Foo { ref a, ref mut b } = f;
    *b = a.clone();
}

// I've got shocked that the compiler is so smart to give me such a complex solution
// which works correctly.
//
// But it's not graceful at all.
//
// Remember we can use `ref` in pattern match
// ```rust
// let a = Some("hello, world".to_string());
// match a {
//     Some(ref s) => ...
//     None => ...
// }
// ```
// What is the difference here?
// The answer is `a` owns `Option<String>`.
//
// Then why `&mut Foo { ref a, ref mut b } = f;` above works?
// The answer is when matching `f: &mut Foo` with `&mut Foo { .. }`,
// the ownership of right `Foo` is tried to be moved left.
//
// But we cannot move things behind a mut reference, so `ref` and `ref mut`
// are used to tell compiler we only want references.
//
// Then things worked.
//
// However, `&mut Foo { ref a, ref mut b } = f;` looks tedious...

fn best_fix(f: &mut Foo) {
    // explicitly dereference f,
    // and use `ref` and `ref mut` to express that we only want reference of fields
    let Foo { ref a, ref mut b } = *f; // this loos better then
    *b = a.clone();
}
