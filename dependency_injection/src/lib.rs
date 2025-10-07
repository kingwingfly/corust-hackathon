//! This case is highly inspired by this tutorial
//! [dependency_injection_like_bevy_from_scratch](https://promethia-27.github.io/dependency_injection_like_bevy_from_scratch/introductions.html)
//!
//! Instead of provide parameters when calling a function, dependency injection is to retrieve parameters from somewhere.
//!
//! e.g. `iterable.map(|x| { ... })`, x is retrieved from the iterable and injected into the closure.
//!
//! You can see this trick in many famous crates, including teloxide, axum and bevy.
//!
//! e.g. In bevy, we give `|mut hp: Single<&mut HP, With<Player>>| **hp += 1` to the scheduler, `HP` is automatically retrieved from somewhere and passed to the closure.
//! In axum, we define `async fn(Json(payload): Json<Payload>) -> Result<impl IntoResponse>` and give it to MethodRouter, the payload is then deserialized and provided to the closure.
//!
//! In this article, I will show you how dependency injection works and let your life with axum, bevy, etc. less confused.

#![allow(unused)]

use std::{
    any::{Any, TypeId},
    collections::HashMap,
    marker::PhantomData,
    vec,
};

/// System: in bevy ecs context, system is similar to function.
///
/// Here, we use a struct to represent a function/system,
/// aiming to avoid trait like `System<I>`, which is not convenient to store in `Vec<Box<dyn System<??>>`.
struct FunctionSystem<F, I /* I is short for Input */> {
    f: F,
    /// PhantomData is used since `I` must be used.
    ///
    /// `fn() -> I`, the fn pointer, is chosen because it doesn't mean the ownership of `I`.
    /// So, whether I is Send/Sync/Drop won't influence `FunctionSystem`.
    ///
    /// And `fn() -> I` is also covariant, the same as `I`.
    _phantom: PhantomData<fn() -> I>,
}

/// This trait is defined to convert `FnMut` into `FunctionSystem`.
trait IntoSystem<F, I> {
    fn into_system(self) -> FunctionSystem<F, I>;
}

/// The magic happens in this trait. Resources/parameters will be retrieved from HashMap,
/// and be passed to `&mut self` which is the FunctionSystem.
trait System {
    fn run(&mut self, resources: &HashMap<TypeId, Box<dyn Any>>);
}

/// This is how I impl system with macro and it differ from
/// dependency_injection_like_bevy_from_scratch tutorial we mentioned before.
macro_rules! impl_system {
    ($(($I: ident, $i: ident)),*) => {
        impl<F, $($I),*> IntoSystem<F, ($($I,)*)> for F
        where
            F: FnMut($($I),*),
        {
            fn into_system(self) -> FunctionSystem<F, ($($I,)*)> {
                FunctionSystem {
                    f: self,
                    _phantom: PhantomData,
                }
            }
        }

        impl<F, $($I),*> System for FunctionSystem<F, ($($I,)*)>
        where
            F: FnMut($($I),*),
            $($I: Any + Clone),*
        {
            /// Here, `I` is cloned from HashMap, which is to say,
            /// in actual usage, `I` is always smart pointer and cheap to be cloned.
            fn run(&mut self, resources: &HashMap<TypeId, Box<dyn Any>>) {
                $(
                    let Some($i) = resources
                        .get(&TypeId::of::<$I>())
                        .and_then(|r| r.downcast_ref::<$I>().cloned()) else {
                            return;
                        };
                )*
                (self.f)($($i),*);
            }
        }
    };
}

/// This macro expanded to:
/// ```
/// impl_append!();
/// impl_append!((I0, i0));
/// ..
/// impl_append!((I0, i0) .. (I4, i4));
/// ```
variadics_please::all_tuples!(impl_system, 0, 5, I, i);

/// Let's define a struct to store the state.
#[derive(Default)]
struct Scheduler {
    systems: Vec<Box<dyn System>>,
    resources: HashMap<TypeId, Box<dyn Any>>,
}

impl Scheduler {
    /// Iterate over systems and run them one by one.
    fn run(&mut self) {
        for s in self.systems.iter_mut() {
            s.run(&self.resources);
        }
    }

    fn add_resource<R: Any>(&mut self, res: R) {
        self.resources.insert(TypeId::of::<R>(), Box::new(res));
    }

    fn add_system<F, I>(&mut self, f: F)
    where
        F: IntoSystem<F, I> + 'static,
        I: Any,
        FunctionSystem<F, I>: System,
    {
        self.systems.push(Box::new(f.into_system()));
    }
}

/// Let's test it!
#[test]
fn test() {
    let mut scheduler = Scheduler::default();

    // Add resources
    scheduler.add_resource(1u8);
    scheduler.add_resource(2u32);
    scheduler.add_resource(3i32);

    // Add systems to run
    scheduler.add_system(|x: u8| println!("{x}"));
    scheduler.add_system(|x: u8, y: u32| println!("{x} {y}"));

    // The args are auto passed, manually passing args will never happen.
    scheduler.run();
    // Dependency injection design let users to concentrate on what needed/todo/returned,
    // instead of caring about providing/retrieving resources.

    // Able to be run again and again...
    // Oh, nice for game logic, what a bevy!
    scheduler.run();
}

// Now we implemented basic dependency injection, however, compared to axum or bevy,
// we lack things like `Json`, `Path`, `Query`, `Res`, etc.

/// Now, we need to define how we extract resources from HashMap.
trait SystemParam: Sized {
    fn retrieve(resources: &HashMap<TypeId, Box<dyn Any>>) -> Option<Self>;
}

/// Similar to Json in axum which deserialize body when extracting/retrieving,
/// `FromU32` convert u32 in resources into T when extracting.
struct FromU32<T>(T);

impl SystemParam for FromU32<u64> {
    fn retrieve(resources: &HashMap<TypeId, Box<dyn Any>>) -> Option<Self> {
        resources
            .get(&TypeId::of::<u32>())
            .and_then(|r| r.downcast_ref::<u32>())
            .map(|r| FromU32(*r as u64))
    }
}

/// To avoid conflict implementation with `Sysyem`
trait System2 {
    fn run2(&mut self, resources: &HashMap<TypeId, Box<dyn Any>>);
}

macro_rules! impl_system2 {
    ($(($I: ident, $i: ident)),*) => {
        impl<F, $($I),*> System2 for FunctionSystem<F, ($($I,)*)>
        where
            F: FnMut($($I),*),
            $($I: SystemParam),*
        {
            /// Now, `I` is retrieved by `SystemParam::retrieve`
            fn run2(&mut self, resources: &HashMap<TypeId, Box<dyn Any>>) {
                $(
                    let Some($i) = <$I>::retrieve(resources) else {
                        return;
                    };
                )*
                (self.f)($($i),*);
            }
        }
    };
}

variadics_please::all_tuples!(impl_system2, 0, 5, I, i);

// We can simply imagine, instead of clone from HashMap, now the resources will be retrieved
// according to the logic in `SystemParam::retrieve`, just like `Json` in axum does.
// For those clever enough, I think stopping here is Ok, there's then nothing more about dependency injection.
// Have a nice day!
