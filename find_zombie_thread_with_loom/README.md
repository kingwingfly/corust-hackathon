When wrapping block `R: Read` into `AsyncReader<R>: futures_io::AsyncRead`, 
I try to spawn a thread to pull data into some limited buffer, waiting `CondVar` if capacity meeted, 
and `AsyncRead::poll_read` to pull from the buffer and wake the brack ground thread 
through `CondVar` if needed.

However, if `AsyncReader<R>` get dropped, the back ground thread will become a zombie thread.
In my first attempt, I check `Arc::strong_count` of the shared state, and wake up the thread with `notify_one` in Drop trait suggested by Ai.

With `cargo run -p find_zombie_thread_with_loom`, you can probably find test passed.

However, if you increase the loop iter number to 10000, it probably hang up, and never exit.

I suddenly realized there's a bug, and the best way to figure it out in Rust world is `loom`.

I make changes to the original code, and got `loom` works. You can try

```sh
LOOM_LOG=trace LOOM_LOCATION=1 RUSTFLAGS="--cfg loom" cargo test -p find_zombie_thread_with_loom --release
```

And got:
```sh
thread 'test' (1244040) panicked at /Users/louis/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/loom-0.7.2/src/rt/execution.rs:216:13:
deadlock; threads = [(Id(0), Blocked(Location(Some(Location { file: "find_zombie_thread_with_loom/src/lib.rs", line: 83, column: 12 })))), (Id(1), Blocked(Location(Some(Location { file: "find_zombie_thread_with_loom/src/lib.rs", line: 36, column: 39 }))))]
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    test

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p find_zombie_thread_with_loom --lib
```

Nice, the deadlock is found.

And logs are as follows:
```sh
failures:

---- test stdout ----
TRACE iter{1}: ..
..
TRACE iter{2}: loom::rt::mutex: Mutex::new state=Ref<loom::rt::mutex::State>(0) seq_cst=true
TRACE iter{2}: loom::rt::condvar: Condvar::new state=Ref<loom::rt::condvar::State>(1)
```
Create `CondVar1` for `shared`.
```sh
TRACE iter{2}: loom::rt::arc: Arc::new state=Ref<loom::rt::arc::State>(2) location=find_zombie_thread_with_loom/src/lib.rs:29:22
TRACE iter{2}: loom::rt::object: Object::branch_action obj=Ref<loom::rt::arc::State>(2) action=RefInc
TRACE iter{2}:thread{id=0}: loom::rt: branch switch=false
TRACE iter{2}:thread{id=0}: loom::rt::arc: Arc::ref_inc state=Ref<loom::rt::arc::State>(2) ref_cnt=2 location=find_zombie_thread_with_loom/src/lib.rs:31:33
TRACE iter{2}:thread{id=0}: loom::rt::notify: Notify::new state=Ref<loom::rt::notify::State>(3) seq_cst=true spurious=false
TRACE iter{2}:thread{id=0}: loom::rt: spawn thread=Id(1)
```
Create `CondVar3` for `thread{id=1}`. Thread spawned, and then
```sh
TRACE iter{2}:thread{id=0}: loom::rt::object: Object::branch_action obj=Ref<loom::rt::condvar::State>(1) action=Opaque
TRACE iter{2}:thread{id=0}: loom::rt: branch switch=false
TRACE iter{2}:thread{id=0}: loom::rt::condvar: Condvar::notify_one state=Ref<loom::rt::condvar::State>(1) thread=None
```
`CondVar1::notify_one` is called in Drop in `thread{id=0}` (main thread) before `CondVar1::wait` in thread 1.
```sh
TRACE iter{2}:thread{id=0}: loom::rt::object: Object::branch_action obj=Ref<loom::rt::arc::State>(2) action=RefDec
 INFO iter{2}:thread{id=1}: loom::rt::execution: ~~~~~~~~ THREAD 1 ~~~~~~~~
TRACE iter{2}:thread{id=1}: loom::rt: branch switch=true
TRACE iter{2}:thread{id=1}: loom::rt::object: Object::branch_action obj=Ref<loom::rt::arc::State>(2) action=Inspect
TRACE iter{2}:thread{id=1}: loom::rt: branch switch=false
TRACE iter{2}:thread{id=1}: loom::rt::mutex: Mutex::is_locked state=Ref<loom::rt::mutex::State>(0) is_locked=false
TRACE iter{2}:thread{id=1}: loom::rt::object: Object::branch_acquire obj=Ref<loom::rt::mutex::State>(0) is_locked=false
TRACE iter{2}:thread{id=1}: loom::rt: branch switch=false
TRACE iter{2}:thread{id=1}: loom::rt::object: Object::branch_action obj=Ref<loom::rt::condvar::State>(1) action=Opaque
TRACE iter{2}:thread{id=1}: loom::rt: branch switch=false
TRACE iter{2}:thread{id=1}: loom::rt::condvar: Condvar::wait state=Ref<loom::rt::condvar::State>(1) mutex=Mutex { state: Ref<loom::rt::mutex::State>(0) }
TRACE iter{2}:thread{id=1}: loom::rt: park thread=Id(1) active.state=Runnable { unparked: false }
```
`CondVar1::wait` is called in `thread{id=1}`, and `thread{id=1}` will be parked forever.
```sh
 INFO iter{2}:thread{id=0}: loom::rt::execution: ~~~~~~~~ THREAD 0 ~~~~~~~~
TRACE iter{2}:thread{id=0}: loom::rt::arc: Arc::ref_dec state=Ref<loom::rt::arc::State>(2) ref_cnt=1 location=/Users/louis/.rustup/toolchains/nightly-aarch64-apple-darwin/lib/rustlib/src/rust/library/core/src/ptr/mod.rs:805:1
```
`foo` is dropped here, and strong count of the inner shared decreased to 1.
```sh
TRACE iter{2}:thread{id=0}: loom::rt::notify: Notify::wait 1 state=Ref<loom::rt::notify::State>(3) notified=false spurious=false
TRACE iter{2}:thread{id=0}: loom::rt::object: Object::branch_acquire obj=Ref<loom::rt::notify::State>(3) is_locked=true
```
`thread{id=0}` wait `CondVar3` which is for `thread{id=1}`.

In loom's implementation of `JoinHandle`, the `join()` mechanism uses an internal Notify primitive.
```rust
pub fn join(self) -> std::thread::Result<T> {
    self.notify.wait(location!());
    self.result.lock().unwrap().take().unwrap()
}
```

loom runtime then found all threads blocked, which is treated as a dead lock.

To fix this bug, I easily add a flag. Anyway, it's the method to find the bug that matters.
