#![allow(dead_code)]

use futures::{
    executor::{LocalPool, block_on},
    future::ready,
    stream::StreamExt as _,
    task::SpawnExt,
};
use std::{
    sync::{Arc, Mutex, mpsc::channel},
    thread,
};

/// This is the most classic implementation.
///
/// With 3 async tasks to receive from channel, there will be always at most 3 tasks
/// running at the same time.
fn classic() {
    let (tx, rx) = channel();
    let rx = Arc::new(Mutex::new(rx));
    let jh = thread::spawn(move || {
        let mut pool = LocalPool::new();
        let spawner = pool.spawner();
        (0..3/* spawn 3 workers*/).for_each(|i| {
            let rx = rx.clone();
            spawner
                .spawn(async move {
                    while let Ok(j) = {
                        rx.lock().unwrap().recv() /*scope is used to drop MutexGuard */
                    } {
                        println!("{i} start an async task {j}");
                        ready(()).await;
                        println!("{i} finish an async task {j}");
                    }
                })
                .unwrap();
        });
        pool.run();
    });

    for i in 0..10 {
        tx.send(i).unwrap();
    }
    drop(tx);
    jh.join().unwrap();
}

// Apart from that, you can also use tokio's `JoinSet` or `Semaphore`
// to limit the concurrency of the async tasks.
//
// But here, I'll introduce you a light method which may meet your demand -- `futures::stream::StreamExt`

fn light() {
    block_on(async {
        let mut buffered = futures::stream::iter(0..10)
            .map(|i| async move {
                println!("start an async task {i}");
                ready(()).await;
                println!("finish an async task {i}");
            })
            .buffer_unordered(3); // limit concurrency to 3
        while let Some(res) = buffered.next().await {
            dbg!(res);
        }
    });
}
