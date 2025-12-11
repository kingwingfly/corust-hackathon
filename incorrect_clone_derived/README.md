In the development of a observation data collector, we choose `tower` to form the pipeline.

```rust
let mut service = ServiceBuilder::new()
    .layer(JsonParser::<Value>::new())
    .layer(FilterLayer::new(
        |res: core::result::Result<Ir, crate::deserializer::error::Error<Value>>| res,
    ))
    .service(AsyncWriter::new(stdout()));
```

Where in `AsyncWriter`:

```rust
#[derive(Debug, Clone)]
pub struct AsyncWriter<W> {
    shared: Arc<Mutex<Shared>>,
    _phantom: PhantomData<W>,
}

impl<W> Service<Ir> for AsyncWriter<W>
where
    AsyncWriter<W>: AsyncWrite + Clone + Unpin + 'static, 
{
    ..
}
```

Pay attention to the `Clone` here.

Initially, we just use `derive(Clone)` to impl `Clone` for `AsyncWriter`.

Then, error raises:
```rust
error[E0599]: the method `call` exists for struct `JsonParserService<Filter<AsyncWriter<Stdout>, ...>, ...>`, but its trait bounds were not satisfied
  --> playground/tower/main.rs:41:33
   |
41 |         if let Err(e) = service.call(line).await {
   |                                 ^^^^ method cannot be called due to unsatisfied trait bounds
   |
  ::: playground/tower/deserializer/json.rs:28:1
   |
28 | pub struct JsonParserService<S, V> {
   | ---------------------------------- method `call` not found for this struct because it doesn't satisfy `_: Service<_>`
   |
  ::: /Users/louis/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tower-0.5.2/src/filter/mod.rs:45:1
   |
45 | pub struct Filter<T, U> {
   | ----------------------- doesn't satisfy `_: Service<Result<Ir, Error<Value>>>`
   |
note: trait bound `tower::filter::Filter<AsyncWriter<std::io::Stdout>, {closure@playground/tower/main.rs:29:13: 29:86}>: Service<Result<Ir, deserializer::error::Error<Value>>>` was not satisfied
  --> playground/tower/deserializer/json.rs:50:8
   |
44 | impl<S, L, V> Service<L> for JsonParserService<S, V>
   |               ----------     -----------------------
...
50 |     S: Service<core::result::Result<Ir, Error<V>>>,
   |        ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ unsatisfied trait bound introduced here
   = help: items from traits can only be used if the trait is implemented and in scope
   = note: the following traits define an item `call`, perhaps you need to implement one of them:
           candidate #1: `Fn`
           candidate #2: `Service`
           candidate #3: `futures_util::fns::Fn1`
   = note: the full name for the type has been written to '/Users/louis/target/debug/examples/tower-87f2c66abe8ef994.long-type-18250416949291364747.txt'
   = note: consider using `--verbose` to print the full type name to the console

For more information about this error, try `rustc --explain E0599`.
error: could not compile `playground` (example "tower") due to 1 previous error
```

The compiler complains about `JsonParserService` and `Filter` instead of `AsyncWriter`.

At first, I just replace `AsyncWriter` with a simple `ServiceFn`, and things works. So that
it's due to `AsyncWriter` does not impl `Service<String>`.

But with this change can we also figure out the reason:
```rust
if let Err(e) = Service::<String>::call(&mut service, line).await {
    eprintln!("{}", e);
}
```

The error message changes to:
```rust
error[E0277]: the trait bound `std::io::Stdout: Clone` is not satisfied
   --> playground/tower/main.rs:41:25
    |
 41 |         if let Err(e) = Service::<String>::call(&mut service, line).await {
    |                         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ the trait `Clone` is not implemented for `std::io::Stdout`
    |
    = help: the trait `Service<Request>` is implemented for `tower::filter::Filter<T, U>`
note: required for `AsyncWriter<std::io::Stdout>` to implement `Clone`
   --> playground/tower/sink/writer_sink.rs:29:17
    |
 29 | #[derive(Debug, Clone)]
    |                 ^^^^^ unsatisfied trait bound introduced in this `derive` macro
note: required for `AsyncWriter<std::io::Stdout>` to implement `Service<Ir>`
   --> playground/tower/sink/writer_sink.rs:156:9
    |
156 | impl<W> Service<Ir> for AsyncWriter<W>
    |         ^^^^^^^^^^^     ^^^^^^^^^^^^^^
157 | where
158 |     AsyncWriter<W>: AsyncWrite + Clone + Unpin + 'static,
    |                                  ----- unsatisfied trait bound introduced here
    = note: 1 redundant requirement hidden
    = note: required for `Filter<AsyncWriter<Stdout>, {closure@main.rs:29:13}>` to implement `Service<Result<Ir, deserializer::error::Error<Value>>>`
    = note: the full name for the type has been written to '/Users/louis/target/debug/examples/tower-87f2c66abe8ef994.long-type-14005499466391656696.txt'
    = note: consider using `--verbose` to print the full type name to the console

For more information about this error, try `rustc --explain E0277`.
error: could not compile `playground` (example "tower") due to 1 previous error
```

Wow, amazing, even through with `derive(Clone)`, `AsyncWrite<W>` is not `Clone`.

Let see what code `derive(Clone)` generated with `cargo-expand`:
```rust
#[automatically_derived]
impl<W: ::core::clone::Clone> ::core::clone::Clone for AsyncWriter<W> {
    #[inline]
    fn clone(&self) -> AsyncWriter<W> {
        AsyncWriter {
            shared: ::core::clone::Clone::clone(&self.shared),
            _phantom: ::core::clone::Clone::clone(&self._phantom),
        }
    }
}
```

Things worked out, it's due to `Stdout` is not `Clone` so that `AsyncWriter<W>` is not `Clone`,
so that `Service` is not implemented.

It's so strange that `W` in `PhantomData` will influence `Clone` proc macro. Anyway, `derive(Clone)`
is not clever enough.

To fix this:
```rust
impl<W> Clone for AsyncWriter<W> {
    fn clone(&self) -> Self {
        Self {
            shared: self.shared.clone(),
            _phantom: PhantomData,
        }
    }
}
```
Or one can try [derive-where](https://crates.io/crates/derive-where) crate.
