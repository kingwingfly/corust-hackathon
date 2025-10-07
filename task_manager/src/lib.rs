//! In asynchronous or multithreaded Rust projects, it’s common to spawn background tasks and let them run concurrently.
//! However, effective task management is equally important — we often need to monitor, control, and track the progress of these tasks.
//! A well-designed API should facilitate this by providing a robust task manager abstraction,
//! enabling control, progress, parallel limitation, and improved developer experience — ultimately leading to cleaner code and happier programmers.

#![allow(dead_code)]

use std::{
    collections::HashMap,
    fmt::Display,
    hash::Hash,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use anyhow::{Result, anyhow};
use crossbeam_channel::{Sender, unbounded};
use parking_lot::RwLock;
use tokio::{runtime::Builder, sync::Semaphore, time::sleep};
use tokio_util::sync::CancellationToken;

/// Below is the simplest implementation, which is rewritten based on tokio documentation, but lacks a lot of functions we mentioned above.
#[derive(Debug)]
struct BasicTaskManager {
    /// The sender of `Duration`.
    /// `Duration` is used to create `Sleep`, which is used to simulate async tasks like fs-ops, net-ops, etc.
    tx: Sender<Duration>,
}

impl BasicTaskManager {
    fn new() -> Self {
        let (tx, rx) = unbounded::<Duration>();
        // spawn a new thread
        thread::spawn(move || {
            // build an async runtime in the new thread
            Builder::new_multi_thread()
                .enable_time()
                .build()
                .unwrap()
                .block_on(async move {
                    // wait new task
                    while let Ok(duration) = rx.recv() {
                        // spawn it in tokio runtime
                        tokio::spawn(async move {
                            println!("task {} started", duration.as_secs_f32());
                            sleep(duration).await;
                            println!("task {} finished", duration.as_secs_f32());
                        });
                    }
                });
        });
        Self { tx }
    }

    /// Send `Duration` to the tokio runtime.
    fn spawn_sleep_task(&self, duration: Duration) -> Result<()> {
        self.tx.send(duration).map_err(Into::into)
    }
}

/// ```ignore
/// task 1 started
/// task 3 started
/// task 2 started
/// task 1 finished
/// task 2 finished
/// task 3 finished
/// ```
#[cfg(feature = "basic")]
#[test]
fn test1() -> Result<()> {
    let task_manager = BasicTaskManager::new();

    task_manager.spawn_sleep_task(Duration::from_secs(1))?;
    task_manager.spawn_sleep_task(Duration::from_secs(3))?;
    task_manager.spawn_sleep_task(Duration::from_secs(2))?;

    thread::sleep(Duration::from_secs(4));

    Ok(())
}

// Now, the `TaskManager` users can easily pass `Duration` to `spawn_sleep_task`,
// and the task will be created and asynchronously spawned.
//
// However, this is far from enough. Then, let's make the task manager:
// - cancellable, `tokio_util::CancellationToken` and `tokio::select` macro.
// - more generic API, `async_trait` and `IntoFuture`.
// - parallel limitation, `tokio::sync::Semaphore`.

/// Without `#[async_trait::async_trait]`, `AsTask<K>` is not dyn-compatible.
#[async_trait::async_trait]
trait AsTask<K>: Send + Sync {
    async fn run(&self, ctx: &TaskContext<K>) -> Result<()>;
    /// Only be called when canceling a started task.
    async fn on_cancel(&self, ctx: &TaskContext<K>);
    async fn on_err(&self, ctx: &TaskContext<K>, e: anyhow::Error);
}

/// A Task implemented `IntoFuture`
struct Task<K> {
    key: K,
    /// `Tasks<K>` contains `Semaphore` and waiting queue,
    /// in `IntoFuture::into_future`, Box<dyn AsTask<K>> is gotten from the waiting queue,
    /// and turn into Future to be awaited
    tasks: Arc<Tasks<K>>,
}

impl<K> IntoFuture for Task<K>
where
    K: Hash + Eq + Clone + Send + Sync,
    Task<K>: Send + 'static,
{
    type Output = Result<()>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            // parallel limitation with Semaphore
            let _permit = self.tasks.semaphore.acquire().await?;
            let (task, context) = {
                // Get ongoing queue WriteGuard first.
                // If we remove task from waiting queue first without locking ongoing queue,
                // `TaskManage::cancel` may find task not exist in both waiting and ongoing queue,
                // leading unexpected behavior.
                let mut ongoing = self.tasks.ongoing.write();
                let Some(task) = self.tasks.waiting.write().remove(&self.key) else {
                    // The task is cancelled before started
                    return Ok(());
                };
                let context = TaskContext {
                    key: self.key.clone(),
                    cancel: CancellationToken::new(),
                };
                ongoing.insert(self.key, context.clone());
                (task, context)
            };
            // basic tokio::select! usage
            tokio::select! {
                res = task.run(&context), if !context.cancel.is_cancelled() => {
                    if let Err(e) = res {
                        task.on_err(&context, e).await;
                    }
                }
                _ = context.cancel.cancelled() => {
                    task.on_cancel(&context).await;
                }
            }
            // remove after finished
            let _ = self.tasks.ongoing.write().remove(&context.key);
            Ok(())
        })
    }
}

/// Let's define the easiest Sleep task as an example
#[derive(Debug)]
struct SleepTask {
    duration: Duration,
}

#[async_trait::async_trait]
impl<K> AsTask<K> for SleepTask
where
    K: Display,
    TaskContext<K>: Sync,
{
    async fn run(&self, ctx: &TaskContext<K>) -> Result<()> {
        println!("Started {}", ctx.key);
        sleep(self.duration).await;
        println!("Finished {}", ctx.key);
        Ok(())
    }
    async fn on_cancel(&self, ctx: &TaskContext<K>) {
        println!("Cancelled {}", ctx.key);
    }
    async fn on_err(&self, _ctx: &TaskContext<K>, _e: anyhow::Error) {
        unreachable!()
    }
}

/// The context of a certain task
#[derive(Clone)]
struct TaskContext<K> {
    key: K,
    cancel: CancellationToken,
}

/// The main entry of our task manager
struct TaskManager<K> {
    tx: Option<Sender<Task<K>>>,
    tasks: Arc<Tasks<K>>,
    jh: Option<JoinHandle<()>>,
}

/// The task queues holding semaphore and waiting/ongoing queues
struct Tasks<K> {
    semaphore: Arc<Semaphore>,
    waiting: RwLock<HashMap<K, Box<dyn AsTask<K>>>>,
    ongoing: RwLock<HashMap<K, TaskContext<K>>>,
}

impl<K> Default for Tasks<K> {
    fn default() -> Self {
        thread::available_parallelism()
            .map(|n| Self::new(n.get()))
            .unwrap_or(Self::new(8))
    }
}

impl<K> Tasks<K> {
    fn new(parallel_num: usize) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(parallel_num)),
            waiting: Default::default(),
            ongoing: Default::default(),
        }
    }
}

impl<K> TaskManager<K>
where
    K: Hash + Eq + Clone + Send + Sync,
    Task<K>: Send + 'static,
{
    fn new() -> Self {
        let (tx, rx) = unbounded::<Task<K>>();
        // spawn a new thread
        let jh = thread::spawn(move || {
            // build an async runtime in the new thread
            Builder::new_multi_thread()
                .enable_time()
                .build()
                .unwrap()
                .block_on(async move {
                    let mut jhs = vec![];
                    // wait new task
                    while let Ok(task) = rx.recv() {
                        // spawn it in tokio runtime
                        jhs.push(tokio::spawn(task.into_future()));
                    }
                    for jh in jhs.into_iter() {
                        let _ = jh.await;
                    }
                });
        });
        Self {
            tx: Some(tx),
            tasks: Default::default(),
            jh: Some(jh),
        }
    }

    fn cancel(&self, k: &K) {
        if self.tasks.waiting.write().remove(k).is_some() {
            // if not started, remove it from waiting queue
            return;
        }
        if let Some(context) = self.tasks.ongoing.read().get(k) {
            // if ongoing, cancel it
            context.cancel.cancel();
        }
    }
}

/// Impl a TaskManager whose task key is usize
impl TaskManager<usize> {
    fn spawn_task(&self, task: impl AsTask<usize> + 'static) -> Result<usize> {
        static KEY: AtomicUsize = AtomicUsize::new(0);

        let key = KEY.fetch_add(1, Ordering::Relaxed);

        self.tasks.waiting.write().insert(key, Box::new(task));

        self.tx
            .as_ref()
            .unwrap()
            .send(Task {
                key,
                tasks: self.tasks.clone(),
            })
            .map_err(|_| anyhow!("Failed to send task through channel"))?;

        Ok(key)
    }
}

impl<K> Drop for TaskManager<K> {
    fn drop(&mut self) {
        // clear all waiting task
        self.tasks.waiting.write().drain();
        // cancel all ongoing task
        self.tasks
            .ongoing
            .write()
            .values()
            .for_each(|context| context.cancel.cancel());
        // disconnect and close the channel
        let _ = self.tx.take();
        // wait until all tasks cancelled
        self.jh.take().unwrap().join().unwrap();
    }
}

/// ```ignore
/// Started 0
/// Started 1
/// Started 2
/// Cancelled 1
/// Finished 0
/// Cancelled 2
/// ```
#[test]
fn test2() -> Result<()> {
    // new a task manager
    let task_manager = TaskManager::<usize>::new();

    // spawn three Sleep task
    let _k0 = task_manager.spawn_task(SleepTask {
        duration: Duration::from_secs(1),
    })?;
    let k1 = task_manager.spawn_task(SleepTask {
        duration: Duration::from_secs(2),
    })?;
    let _k2 = task_manager.spawn_task(SleepTask {
        duration: Duration::from_secs(3),
    })?;

    thread::sleep(Duration::from_secs(1));

    // after 1s, cancel the task sleeping 2s
    task_manager.cancel(&k1);

    thread::sleep(Duration::from_secs(1));

    // after 2s, the task sleeping 3s is still in progress,
    // however, task_manager dropping will cancel it
    Ok(())
}
