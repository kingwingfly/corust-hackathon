Tailing file is an important log source in an observability tool.

At first, we wrap `tokio::fs::File`, and use `notify` crate to drive `AsyncRead`.

So that `TailingFile` treat `EOF` as `WouldBlock`, and things work as expected.

See [modified_only.rs](src/modified_only.rs).

However, in real system, the log file may be created after collector starting, leading us supposed to treat `NotFound` as `WouldBlock` as well, and then things become tricky.

See [created_as_well.rs](src/created_as_well.rs).
