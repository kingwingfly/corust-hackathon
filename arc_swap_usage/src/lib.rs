//! [arc_swap](https://docs.rs/arc-swap/latest/arc_swap/)
//! > There are many situations in which one might want to have some data structure that is often read and seldom updated.
//! > Some examples might be a configuration of a service, routing tables, snapshot of some data that is renewed every few minutes, etc.

#![allow(unused)]

use std::{
    fmt::DebugStruct,
    sync::{Arc, RwLock},
};

use arc_swap::ArcSwap;

/// Here is an easy example: maybe sometime the config would be updated, but before that, the config String
/// is shared through `Arc::clone`, and after that, new config starts its show.
#[derive(Default)]
#[cfg(feature = "not_so_good")]
struct App1 {
    config: Arc<RwLock<Arc<String>>>,
}

#[cfg(feature = "not_so_good")]
impl App1 {
    /// Clone the config pointer, and do something with it.
    fn handle(&self, data: usize) {
        // NOTICE: in real life, it is always async, here we use thread::spawn to simplify
        std::thread::spawn({
            let config = self.config.clone();
            move || {
                // Read config from many threads
                let content = config.read().unwrap().clone();
                println!("{data} do the job with config: {content}");
            }
        });
    }

    /// Update the inner config pointer.
    fn update_config(&self, new: impl AsRef<str>) {
        *self.config.write().unwrap() = Arc::new(new.as_ref().to_string());
    }
}

#[cfg(feature = "not_so_good")]
#[test]
fn app1_test() {
    let app = App1::default();
    for i in 0..100 {
        // Seldom updated
        if i % 10 == 0 {
            app.update_config(i.to_string());
        }
        app.handle(i);
    }
}

// Above is totally Ok for daily use, however, according to [this report](https://www.conviva.com/resource/the-concurrency-trap-how-an-atomic-counter-stalled-a-pipeline/).
// The `ReadGuard` may delay the system and atomic counter inside may become the bottleneck.
//
// Moreover, if `ReadGuard` always exists, `WriteGuard` is prevented so that update will be blocked.
//
// Then, `arc-swap` become the solution.

#[derive(Default)]
struct App2 {
    config: Arc<ArcSwap<String>>,
}

impl App2 {
    /// Clone the config pointer, and do something with it.
    fn handle(&self, data: usize) {
        // NOTICE: in real life, it is always async, here we use thread::spawn to simplify
        std::thread::spawn({
            let config = self.config.clone();
            move || {
                // Read config from many threads
                let content = config.load_full();
                println!("{data} do the job with config: {content}");
            }
        });
    }

    /// Update the inner config pointer.
    fn update_config(&self, new: impl AsRef<str>) {
        self.config.store(Arc::new(new.as_ref().to_string()));
    }
}

#[test]
fn app2_test() {
    let app = App2::default();
    for i in 0..100 {
        // Seldom updated
        if i % 10 == 0 {
            app.update_config(i.to_string());
        }
        app.handle(i);
    }
}
