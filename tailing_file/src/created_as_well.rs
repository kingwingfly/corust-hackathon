//! Source from file

use std::{
    io::{self, SeekFrom},
    path::{Path, PathBuf},
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use futures_util::{
    future::{MaybeDone, maybe_done},
    task::AtomicWaker,
};
use notify::{RecommendedWatcher, RecursiveMode, Watcher, recommended_watcher};
use tokio::{
    fs::File,
    io::{AsyncRead, AsyncSeekExt, ReadBuf},
};

/// A Future to open a file.
///
/// Re-open on NotFound Error and return Pending.
struct FileFut {
    fut: Pin<Box<dyn Future<Output = Result<File, io::Error>> + Send + Sync + 'static>>,
    path: PathBuf,
}

impl FileFut {
    fn new(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref().to_path_buf();
        Self {
            fut: Box::pin({
                let path = path.clone();
                async move {
                    let mut file = File::open(path).await?;
                    file.seek(SeekFrom::End(0)).await?;
                    Ok(file)
                }
            }),
            path,
        }
    }
}

impl Future for FileFut {
    type Output = io::Result<File>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        loop {
            match this.fut.as_mut().poll(cx) {
                Poll::Ready(Ok(file)) => return Poll::Ready(Ok(file)),
                Poll::Ready(Err(e)) if e.kind() == io::ErrorKind::NotFound => {
                    // return Pending and re-open on NotFound error
                    *this = FileFut::new(&this.path);
                    return Poll::Pending;
                }
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Pending => {
                    // loop until a result, since Modified and Created event from notify
                    // will wake this up **only once**.
                    // It's incorrect to return Pending after opened but before seeked.
                    // In other words, we hope this.fut atomic.
                }
            }
        }
    }
}

/// A file wrapper treat EOF and NotFound as Pending
pub struct TailingFile {
    file: MaybeDone<FileFut>,
    waker: Arc<AtomicWaker>,
    _watcher: RecommendedWatcher,
}

impl TailingFile {
    /// Open a file.
    pub async fn open(path: impl AsRef<Path>) -> Result<Self, io::Error> {
        // convert into an absolute path
        let path = std::env::current_dir()
            .map(|p| p.join(path.as_ref()))
            .unwrap();
        debug_assert!(path.is_absolute());

        if path.is_dir() {
            return Err(io::ErrorKind::IsADirectory.into());
        }

        let waker = Arc::new(AtomicWaker::new());
        let mut watcher = recommended_watcher({
            let path = path.clone();
            let waker = waker.clone();
            move |e: Result<notify::Event, notify::Error>| {
                let Ok(e) = e else {
                    return;
                };
                if (e.kind.is_modify() || e.kind.is_create())
                    && e.paths.contains(&path)
                    && let Some(w) = waker.take()
                {
                    w.wake();
                }
            }
        })
        .unwrap(); // panic if not supported
        watcher
            .watch(
                path.parent().unwrap(), // file has parent
                RecursiveMode::NonRecursive,
            )
            .unwrap(); // panic if not supported
        Ok(TailingFile {
            file: maybe_done(FileFut::new(path)),
            waker,
            _watcher: watcher,
        })
    }
}

impl AsyncRead for TailingFile {
    /// Pending if EOF or NotFound. Self-wake waker in Context on OS event.
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();

        loop {
            match Pin::new(&mut this.file).output_mut() {
                // opened, try read
                Some(Ok(file)) => {
                    let before = buf.filled().len();
                    match Pin::new(file).poll_read(cx, buf) {
                        Poll::Ready(Ok(())) => {
                            let after = buf.filled().len();
                            if after > before {
                                return Poll::Ready(Ok(()));
                            }
                            // no more new bytes
                        }
                        Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                        Poll::Pending => {}
                    }
                    // register waker and interested on file Modified event
                    this.waker.register(cx.waker());
                    return Poll::Pending;
                }
                // not NotFound error
                Some(Err(_)) => {
                    let e = Pin::new(&mut this.file).take_output().unwrap().unwrap_err();
                    return Poll::Ready(Err(e));
                }
                // not open yet
                None => match Pin::new(&mut this.file).poll(cx) {
                    Poll::Ready(()) => {}
                    Poll::Pending => {
                        // register waker and interested on file Created event
                        this.waker.register(cx.waker());
                        return Poll::Pending;
                    }
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::{fs::OpenOptions, io::Write as _, path::PathBuf, thread, time::Duration};

    use tokio::{
        io::{AsyncBufReadExt as _, BufReader},
        time::timeout,
    };

    #[tokio::test]
    async fn tailing_file() {
        let path = PathBuf::from(env!("OUT_DIR")).join("tailing_file.log");
        let _ = std::fs::remove_file(&path);

        let jh = thread::spawn({
            let path = path.clone();
            move || {
                // create file later
                thread::park_timeout(Duration::from_millis(100));
                let mut file = OpenOptions::new()
                    .create(true)
                    .truncate(true)
                    .read(true)
                    .write(true)
                    .open(&path)
                    .unwrap();
                thread::park_timeout(Duration::from_millis(100));
                file.write_all(b"hello world\n").unwrap();
            }
        });
        let mut tailing_file = BufReader::new(TailingFile::open(&path).await.unwrap());
        let mut buf = String::new();

        timeout(Duration::from_secs(1), tailing_file.read_line(&mut buf))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(&buf, "hello world\n");

        jh.join().unwrap();
        let _ = std::fs::remove_file(path);
    }
}
