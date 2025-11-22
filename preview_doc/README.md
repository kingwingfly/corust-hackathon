We always use `doc_cfg` and `fake_variadic` to generate better documentation to be pulished onto docs.rs.

And docs.rs also officially suggests using a tool named `cargo-docs-rs` to simulate the environment of docs.rs runner.

However, when developing proc-macro crate which is used to simplify `fake_variadic` for users, 
the developers need to preview documentation of examples. 

Unluckily, this is not supported by `cargo-docs-rs` currenctly.
i.e. Running `cargo docs-rs -p preview_doc --example foo`, we got
```sh
cargo docs-rs -p preview_doc --example foo
    error: unexpected argument '--example' found
```

It's far away from enough for developers to use `cargo expand` to check if proc-macro works as expected.

One may found easily run `cargo doc` won't give expected result.

The correct solution is
```sh
RUSTDOCFLAGS="--cfg docsrs" cargo +nightly doc -p preview_doc --example foo
```
