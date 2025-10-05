#![allow(unused)]

/// Let's define a trait
trait Foo {}

/// Impl Trait for all `T`s
impl<T> Foo for T {}

/// Through this test, we can know:
///
/// T is not only solved as simple usize, isize, A, &A,
/// but also as tuple of them, e.g. (usize, isize, A, &A)
#[test]
fn test1() {
    /// This is how we check the trait
    fn check<T: Foo>() {}

    check::<usize>();
    check::<isize>();
    struct A;
    check::<A>();
    check::<&A>();
    check::<(usize, isize, A, &A)>();
}

/// Now, we have more complicated demand:
///
/// This trait is expected to be enhanced std::borrow::Borrow,
/// which can not only borrow T as &T, but also can borrow (T1, T2, ...) as (&T1, &T2, ...).
///
/// p.s this is just an example, `&(T1, T2)` can be borrowed as `(&T1, &T2)` through pat-match.
trait Borrow<'a> {
    type Ref;

    fn borrow(&'a self) -> Self::Ref;
}

/// This is easy, but we notice that T can be (usize, isize), i.e. we will borrow (usize, isize) as &(usize, isize),
/// it's probably not what we want.
impl<'a, T> Borrow<'a> for T
where
    T: 'a,
{
    type Ref = &'a T;

    fn borrow(&'a self) -> Self::Ref {
        self
    }
}

/// Let `cargo check -F err`:
/// ```
///error[E0119]: conflicting implementations of trait `Borrow` for type `(_, _)`
///   --> src/lib.rs:61:1
///    |
/// 36 | / impl<T> Borrow for T
/// 37 | | where
/// 38 | |     for<'a> T: 'a,
///    | |__________________- first implementation here
/// ...
/// 61 | / impl<T1, T2> Borrow for (T1, T2)
/// 62 | | where
/// 63 | |     for<'a> T1: 'a,
/// 64 | |     for<'a> T2: 'a,
///    | |___________________^ conflicting implementation for `(_, _)`
/// ```
/// This is due to that we have implemented Borrow for T,
/// which can be solved as (T1, T2), so it is duplicated.
#[cfg(feature = "err")]
impl<'a, T1, T2> Borrow<'a> for (T1, T2)
where
    T1: 'a,
    T2: 'a,
{
    type Ref = (&'a T1, &'a T2);

    fn borrow(&'a self) -> Self::Ref {
        (&self.0, &self.1)
    }
}

/// Let's defined Baz to solve this issue
trait Baz<'a, M /*M is short for Marker*/> {
    type Ref;

    // `1` to differ with trait `Bar`.
    fn borrow1(&'a self) -> Self::Ref;
}

/// impl `Baz<()>` for all `T`s first
impl<'a, T> Baz<'a, ()> for T
where
    T: 'a,
{
    type Ref = &'a T;

    fn borrow1(&'a self) -> Self::Ref {
        self
    }
}

/// impl `Baz<((),)>` for all tuples of `T`s.
///
/// Due to `Baz<()>` and `Baz<((),)>` is not the same, so there's no confict.
///
/// ps: It is `((),)` considered as tuple, while `(())` is considered the same as `()`.
/// So, we use `((),)` here to diff with `()`.
#[cfg(not(feature = "variadics_please"))]
impl<'a, T0, T1> Baz<'a, ((),)> for (T0, T1)
where
    T0: Baz<'a, ()>,
    T1: Baz<'a, ()>,
{
    type Ref = (T0::Ref, T1::Ref);

    fn borrow1(&'a self) -> Self::Ref {
        (self.0.borrow1(), self.1.borrow1())
    }
}

#[cfg(not(feature = "variadics_please"))]
#[test]
fn test2() {
    fn check<'a, T: Baz<'a, M>, M>() {}
    check::<usize, _>();
    check::<isize, _>();
    // Here, M cannot be inferred, due to `Baz<()>` and `Baz<(),>` are both implemented for (usize, isize),
    // the compiler cannot simply decide which `borrow1` to use.
    check::<(usize, isize), ((),)>();
}

/// Last, it's not a good idea to impl Baz<((),)> for all tuples of `T`s manually.
/// So, macro is going to be used.
///
/// What is helpful to implement trait for tuples is to use the crate named `paste`,
/// it can magically turn `[<a b $i>]` into `ab$i` where `$i` is matched by the macro.
#[cfg(not(feature = "variadics_please"))]
macro_rules! impl_baz {
    ($($i: literal),* $(,)?) => {
        paste::paste! {
            impl<'a, $([<T $i>]),*> Baz<'a, ((),)> for ($([<T $i>]),*,)
            where
                $([<T $i>]: Baz<'a, ()>),*
            {
                type Ref = ($([<T $i>]::Ref),*,);

                fn borrow1(&'a self) -> Self::Ref {
                    (
                        $(self.$i.borrow1()),*,
                    )
                }
            }
        }
    };
}

/// Let's just begin from 3-elem-tuple, since 2-elem-tuple has been implemented before
#[cfg(not(feature = "variadics_please"))]
impl_baz!(0, 1, 2);
#[cfg(not(feature = "variadics_please"))]
impl_baz!(0, 1, 2, 3);
#[cfg(not(feature = "variadics_please"))]
impl_baz!(0, 1, 2, 3, 4);

/// Nice, we have just implemented `Baz<((),)>` for 3/4/5-elem-tuple, but
/// ```
/// impl_baz!(0, 1, 2);
/// impl_baz!(0, 1, 2, 3);
/// impl_baz!(0, 1, 2, 3, 4);
/// ```
/// It seems too tedious...
///
/// A crate developed by bevy team is born for this scenario, which is named `variadics_please`,
/// let's try it!
#[cfg(feature = "variadics_please")]
macro_rules! impl_baz2 {
    ($(($i: tt, $t: ident)),* $(,)?) => {
        impl<'a, $($t),*> Baz<'a,((),)> for ($($t),*,)
        where
            $($t: Baz<'a, ()>),*
        {
            type Ref = ($($t::Ref),*,);

            fn borrow1(&'a self) -> Self::Ref {
                (
                    $(self.$i.borrow1()),*,
                )
            }
        }
    };
}

// This is turned into:
// ```
// impl_baz2!((0, T0));
// impl_baz2!((0, T0), (1, T1));
// ...
// impl_baz2!((0, T0), .. (9, T9));
// ```
// This makes life easier!
#[cfg(feature = "variadics_please")]
variadics_please::all_tuples_enumerated!(impl_baz2, 1, 10, T);

// To draw a conclusion, we use a generic param `M` to differ Trait `Baz`, so that
// we can implemented it for all T and tuple of Ts separately.
//
// The example in this article may not be useful enough, however, you'll probably meet and even
// write your own such kind of code in the future learning especially in `tower` and `bevy`.
//
// Useful example can be found in
// - [curriculum](https://github.com/kingwingfly/curriculum/blob/dev/curriculum/src/query.rs)
// - [bevy_quadtree](https://github.com/kingwingfly/bevy_quadtree/blob/dev/bevy_quadtree/src/tree/query.rs)
//
// Some discussion in Rust user forum:
// - [How to define trait used for enhancing tuples](https://users.rust-lang.org/t/how-to-define-trait-used-for-enhancing-tuples/114886)
// - [Macros: How to make impl Trait for tuple better](https://users.rust-lang.org/t/macros-how-to-make-impl-trait-for-tuple-better/133577)
