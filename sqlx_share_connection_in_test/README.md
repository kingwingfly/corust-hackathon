# Steps
1. Start postgresql
2. Test a few times
```sh
cargo test bug_test
```
It randomly fails with the following error `PoolTimedOut`:
```sh
running 2 tests
INIT ModelManager
test tests::test2 ... ok
thread 'tests::test1' panicked at src/lib.rs:39:33:
called `Result::unwrap()` on an `Err` value: PoolTimedOut
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
test tests::test1 ... FAILED
```

# Reason
This is because every tokio::test runs in a separate runtime, but share the same OnceCell initialized by one of them.

If the runtime which initialized the OnceCell exits and the IO resources just stop working, the left runtimes can never get the OnceCell.

# Fix
Fix in this way:
```rust
/// This creates a static runtime for the connection.
pub fn test_rt() -> &'static tokio::runtime::Runtime {
    static RT: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(init_test());
        rt
    });
    &RT
}

/// All the test about database should be run in this function.
/// Because we need to share the same database connection,
/// which cannot keep alive between runtimes.
/// In other words, if the database connection is closed
/// due to the end of the runtime which initialized it, the other runtimes will fail.
pub fn run_test<F: std::future::Future>(f: F) -> F::Output {
    test_rt().block_on(f)
}

#[test]
fn test() {
    // run all test in run_test()
    run_test(async {
        let _mm = init_test().await;
    })
}
```
This can ensure all the tests share the same runtime.

You can try:
```
cargo test fixed_test
```
