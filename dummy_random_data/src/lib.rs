//! In many cases, we need to generate dummy data to mock data sources.
//!
//! And in some of them, without correct implementation,
//! the performance could be the bottleneck.
//!
//! In this example, we'll optimize the dummy data generator
//! and finally got a 18s -> 0.13s speeding up.

#![allow(dead_code)]

use std::time::Instant;

/// Run `f` 8 times, each `f` is expected to generate 60 [u8; 1920 * 1080 * 3].
/// Which is to say, generating 8s 60fps dummy video.
/// Print the time cost.
fn time_it(f: impl Fn()) {
    let now = Instant::now();
    for _ in 0..8 {
        f();
    }
    println!("Time cost: {:.2} s", now.elapsed().as_secs_f32());
}

/// for loop + ThreadRng.
///
/// `cargo test -p repeat_with_and_small_rng first_attempt --release -- --nocapture`
/// We got,
/// ```text ignore
/// Time cost: 18.65 s
/// ```
/// 18.65s > 8s, the performance is unacceptable.
#[test]
fn first_attempt() {
    use rand::Rng as _;
    use std::hint::black_box;

    fn generate() {
        let mut rng = rand::rng();
        for _ in 0..60 {
            let mut frame = Vec::with_capacity(1920 * 1080 * 3);
            for _ in 0..1920 * 1080 * 3 {
                frame.push(rng.random::<u8>());
            }
            black_box(&frame); // prevent frame from being optimized
        }
    }

    time_it(generate);
}

/// repeat_with + ThreadRng
///
/// `cargo test -p repeat_with_and_small_rng second_attempt --release -- --nocapture`
/// We got,
/// ```text ignore
/// Time cost: 15.18 s
/// ```
/// Much better, but still not enough.
#[test]
fn second_attempt() {
    use rand::Rng as _;
    use std::{hint::black_box, iter::repeat_with};

    fn generate() {
        let mut rng = rand::rng();
        for _ in 0..60 {
            let frame = repeat_with(|| rng.random::<u8>())
                .take(1920 * 1080 * 3)
                .collect::<Vec<_>>();
            black_box(&frame); // prevent frame from being optimized
        }
    }

    time_it(generate);
}

/// repeat_with + SmallRng
///
/// Notice that we do not need crypto secure, we can just use `SmallRng` instead of `ThreadRng`.
///
/// `cargo test -p repeat_with_and_small_rng third_attempt --release -- --nocapture`
/// We got,
/// ```text ignore
/// Time cost: 3.46 s
/// ```
#[test]
fn third_attempt() {
    use rand::{Rng as _, SeedableRng as _};
    use std::{hint::black_box, iter::repeat_with};

    fn generate() {
        // use SmallRng instead
        let mut rng = rand::rngs::SmallRng::from_os_rng();
        for _ in 0..60 {
            let frame = repeat_with(|| rng.random::<u8>())
                .take(1920 * 1080 * 3)
                .collect::<Vec<_>>();
            black_box(&frame); // prevent frame from being optimized
        }
    }

    time_it(generate);
}

/// for loop + SmallRng
///
/// We still notice `repeat_with` will hugely speed up dummy frames generating
/// according the first and second attempt.
/// Let's verity, `cargo test -p repeat_with_and_small_rng forth_attempt --release -- --nocapture`.
/// ```text ignore
/// Time cost: 3.57 s
/// ```
/// Wow, a little less efficient. `repeat_with` has magic to some extend.
/// Apart from that, if we remove `black_box(&frame)` in the third attempt,
/// the time cost will become 0s, since frame is entirely optimized,
/// which is say usage of `repeat_with` can help compiler generate better code.
#[test]
fn forth_attempt() {
    use rand::{Rng as _, SeedableRng as _};
    use std::hint::black_box;

    fn generate() {
        // use SmallRng instead
        let mut rng = rand::rngs::SmallRng::from_os_rng();
        for _ in 0..60 {
            let mut frame = Vec::with_capacity(1920 * 1080 * 3);
            for _ in 0..1920 * 1080 * 3 {
                frame.push(rng.random::<u8>());
            }
            black_box(&frame); // prevent frame from being optimized
        }
    }

    time_it(generate);
}

/// RngCore + SmallRng
///
/// `cargo test -p repeat_with_and_small_rng fifth_attempt --release -- --nocapture`.
/// ```text ignore
/// Time cost: 0.47 s
/// ```
/// This is due to SIMD.
#[test]
fn fifth_attempt() {
    use rand::{RngCore as _, SeedableRng as _};
    use std::hint::black_box;

    fn generate() {
        // use SmallRng instead
        let mut rng = rand::rngs::SmallRng::from_os_rng();
        for _ in 0..60 {
            let mut frame = vec![0u8; 1920 * 1080 * 3];
            rng.fill_bytes(&mut frame);
            black_box(&frame); // prevent frame from being optimized
        }
    }

    time_it(generate);
}

/// RngCore + SmallRng + **Rayon**
///
/// Let's use rayon.
/// `cargo test -p repeat_with_and_small_rng sixth_attempt --release -- --nocapture`.
/// ```text ignore
/// Time cost: 0.13 s
/// ```
/// Wow, amazing.
#[test]
fn sixth_attempt() {
    use rand::{RngCore as _, SeedableRng as _};
    use rayon::iter::{IntoParallelIterator as _, ParallelIterator as _};
    use std::hint::black_box;

    fn generate() {
        // use SmallRng instead
        (0..60).into_par_iter().for_each(|_| {
            let mut rng = rand::rngs::SmallRng::from_os_rng();
            let mut frame = vec![0u8; 1920 * 1080 * 3];
            rng.fill_bytes(&mut frame);
            black_box(&frame);
        });
    }

    time_it(generate);
}
