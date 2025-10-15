#![allow(dead_code)]

//! In Tauri, we can use parameter injection to retrieve state in invoke handlers. However, due to Tauri's poor implementation,
//! the states in app won't get dropped when exiting due to `std::process::exit(0)` is called directly.
//!
//! In this example, I'll show you the tricky to hack this bug, and you can solve your own in your daily practice.

use std::{
    any::{Any, TypeId},
    collections::HashMap,
};

struct State;

impl Drop for State {
    fn drop(&mut self) {
        println!("Dropped")
    }
}

type StateMap = HashMap<TypeId, Box<dyn Any>>;

/// Dummy Tauri app.
struct DummyTauriApp {
    /// The states can be retrieved by parameter injection, e.g. invoke handler in Tauri
    states: StateMap,
    /// The callback when app exiting
    on_exit: Option<Box<dyn FnOnce()>>,
}

/// According to [Rust reference: destructors](https://doc.rust-lang.org/reference/destructors.html),
/// > Trait objects run the destructor of the underlying type.
///
/// So, run `test1`, we get `Dropped` printed.
#[test]
fn test1() {
    let _app = DummyTauriApp {
        states: HashMap::from([(TypeId::of::<State>(), Box::new(State) as _)]),
        on_exit: Some(Box::new(|| {})),
    };
}

impl DummyTauriApp {
    /// When users close the window, Tauri will send WindowDestroy event through IPC channel,
    /// callback is invoked, and directly `std::process::exit(0)`, leading states not dropped.
    fn exit(&mut self) {
        (self.on_exit.take().unwrap())();
        // App::exit call std::process::exit(0) without dropping self.states
        std::process::exit(0);
    }
}

/// i.e. Run `test2`, we won't get `Dropped` printed.
#[test]
fn test2() {
    let mut app = DummyTauriApp {
        states: HashMap::from([(TypeId::of::<State>(), Box::new(State) as _)]),
        on_exit: Some(Box::new(|| {})),
    };
    app.exit();
}

/// Luckily, tauri provides [on_window_event](https://docs.rs/tauri/2.8.5/tauri/struct.Builder.html#method.on_window_event)
/// method to register callback.
///
/// ```ignore
/// tauri::Builder::default()
///     .on_window_event(|_, ev| {
///         if matches!(ev, WindowEvent::Destroyed) {
///             ...
///         }
///     })
/// ```
///
/// However, `on_window_event::<F: Fn(&Window<R>, &WindowEvent) + Send + Sync + 'static>`,
/// according to the trait bound of F, we can know that it's impossible to get `State` dropped
/// with useless `&Window<R>` and `&WindowEvent`.
///
/// So trick is needed to make `State` dropped in the callback.
#[test]
fn test3() {
    use std::sync::Arc;

    // We use Arc to wrap State, so that we can drop its inner by decreasing its counter.
    let state = Arc::new(State); // counter: 1

    let mut app = DummyTauriApp {
        states: HashMap::from([(TypeId::of::<Arc<State>>(), Box::new(state.clone()) as _)]), // counter: 2
        on_exit: Some(Box::new(move || {
            let state = Arc::into_raw(state);
            unsafe {
                Arc::decrement_strong_count(state); // counter: 2 - 1 = 1
                let _ = Arc::from_raw(state); // counter: 1 - 1 = 0
            }
            // We successfully get `State` dropped without access `app.states`
        })),
    };

    app.exit();
    // process is killed, no use after free bug.
}

// Wait, so `Arc` is used and `State` is made immutable, how can we mutate state with tauri invoke handlers?
//
// Well, use internal mutability instead. By the way, due to Tauri's poor implementation, tauri DOES NOT support mutable state at all.
// So, use Arc tricky won't increase the complexity using Tauri, you always need internal mutability.

/// Inspired by `test3`, we can event achieve with raw pointer.
#[test]
fn test4() {
    use std::sync::Arc;

    let mut state = State;
    let state_ptr = &mut state as *mut _;

    let mut app = DummyTauriApp {
        states: HashMap::from([(TypeId::of::<Arc<State>>(), Box::new(state) as _)]),
        on_exit: Some(Box::new(move || unsafe {
            let _ = Box::from_raw(state_ptr);
            // We successfully get `State` dropped without access `app.states`
        })),
    };

    app.exit();
    // process is killed, no use after free bug.
}
