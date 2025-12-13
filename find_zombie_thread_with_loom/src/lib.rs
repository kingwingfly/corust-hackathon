#![allow(dead_code)]

#[cfg(not(loom))]
use std::{
    sync::{Arc, Condvar, Mutex},
    thread,
};

#[cfg(loom)]
use loom::{
    sync::{Arc, Condvar, Mutex},
    thread,
};

#[derive(Debug, Default)]
struct Shared {
    #[cfg(feature = "error")]
    state: Mutex<()>,
    #[cfg(not(feature = "error"))]
    state: Mutex<bool>,
    cvar: Condvar,
}

#[derive(Debug)]
struct Foo {
    shared: Arc<Shared>,
    jh: Option<thread::JoinHandle<()>>,
}

impl Foo {
    #[cfg(feature = "error")]
    fn new() -> Self {
        let shared = Arc::new(Shared::default());
        let jh = thread::spawn({
            let shared = shared.clone();
            move || {
                while Arc::strong_count(&shared) > 1 {
                    let Shared { state, cvar } = &*shared;
                    let state = state.lock().unwrap();
                    let _state = cvar.wait(state).unwrap();
                    // do something else
                }
            }
        });
        Self {
            shared,
            jh: Some(jh),
        }
    }

    #[cfg(not(feature = "error"))]
    fn new() -> Self {
        let shared = Arc::new(Shared::default());
        let jh = thread::spawn({
            let shared = shared.clone();
            move || {
                let Shared { state, cvar } = &*shared;
                let mut stop = state.lock().unwrap();
                while !*stop {
                    stop = cvar.wait(stop).unwrap();
                    // do something else
                }
            }
        });
        Self {
            shared,
            jh: Some(jh),
        }
    }

    fn take_jh(&mut self) -> thread::JoinHandle<()> {
        self.jh.take().unwrap()
    }
}

#[cfg(feature = "error")]
impl Drop for Foo {
    fn drop(&mut self) {
        self.shared.cvar.notify_one();
    }
}

#[cfg(not(feature = "error"))]
impl Drop for Foo {
    fn drop(&mut self) {
        let Shared { state, cvar } = &*self.shared;
        let mut stop = state.lock().unwrap();
        *stop = true;
        cvar.notify_one();
    }
}

/// cargo test -p find_zombie_thread_with_loom -F error`
#[cfg(not(loom))]
#[test]
fn test() {
    for _ in 0..10 {
        let mut foo = Foo::new();
        let jh = foo.take_jh();
        drop(foo);
        jh.join().unwrap();
    }
    // None hangs probably,
    // but is it true that all foo's background threads 100% ended?
    // If `Arc::strong_count(&shared)` before `foo.shared` getting dropped,
    // the background thread will be hung.
    // How can we find this situation in test?
    // One solution is add iter times from 10 to 10000,
    // so that the test will be more likely to hang.
}

/// `LOOM_LOG=trace RUSTFLAGS="--cfg loom" cargo test -p find_zombie_thread_with_loom -F error --release`
/// And got
/// ```
/// deadlock; threads = [(Id(0), Blocked(Location(None))), (Id(1), Blocked(Location(None)))]
/// ```
/// try
/// `LOOM_LOG=trace RUSTFLAGS="--cfg loom" cargo test -p find_zombie_thread_with_loom --release`
/// to get fixed.
#[cfg(loom)]
#[test]
fn test() {
    loom::model(|| {
        let mut foo = Foo::new();
        let jh = foo.take_jh();
        drop(foo);
        jh.join().unwrap();
    });
}
