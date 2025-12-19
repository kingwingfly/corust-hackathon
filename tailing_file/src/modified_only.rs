use std::{
    collections::VecDeque,
    io,
    path::Path,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll, Waker},
};

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use parking_lot::Mutex;
use tokio::{
    fs::File,
    io::{AsyncRead, ReadBuf},
};

#[derive(Debug)]
struct TailingFile {
    inner: File,
    wakers: Arc<Mutex<VecDeque<Waker>>>,
    _watcher: RecommendedWatcher,
}

impl TailingFile {
    async fn open(path: impl AsRef<Path>) -> Result<Self, io::Error> {
        if path.as_ref().is_dir() {
            return Err(io::ErrorKind::IsADirectory.into());
        }
        // If file not exists, return NotFound error,
        // this is not expected
        if !path.as_ref().exists() {
            return Err(io::ErrorKind::NotFound.into());
        }

        let wakers = Arc::new(Mutex::new(VecDeque::<Waker>::new()));

        let mut watcher = notify::recommended_watcher({
            let wakers = wakers.clone();
            move |e: Result<notify::Event, notify::Error>| {
                let Ok(e) = e else { return };
                // only listen to Modify event
                if e.kind.is_modify() {
                    for w in wakers.lock().drain(..) {
                        w.wake();
                    }
                }
            }
        })
        .unwrap();

        watcher
            .watch(path.as_ref().parent().unwrap(), RecursiveMode::NonRecursive)
            .unwrap();

        Ok(Self {
            inner: File::open(path).await?,
            wakers,
            _watcher: watcher,
        })
    }

    fn register(&self, waker: Waker) {
        self.wakers.lock().push_back(waker);
    }
}

impl AsyncRead for TailingFile {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        let before = buf.filled().len();
        match Pin::new(&mut this.inner).poll_read(cx, buf) {
            Poll::Ready(Ok(())) => {
                let after = buf.filled().len();
                if after > before {
                    return Poll::Ready(Ok(()));
                }
            }
            Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
            Poll::Pending => {}
        }
        this.register(cx.waker().clone());
        Poll::Pending
    }
}
