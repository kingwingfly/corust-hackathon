use std::{
    collections::VecDeque,
    io::{self, SeekFrom},
    path::{Path, PathBuf},
    pin::Pin,
    sync::Arc,
    task::{Context, Poll, Waker},
};

use futures_util::future::{MaybeDone, maybe_done};
use notify::{RecommendedWatcher, RecursiveMode, Watcher, recommended_watcher};
use parking_lot::Mutex;
use tokio::{
    fs::File,
    io::{AsyncRead, AsyncSeekExt, ReadBuf},
};

/// A Future to open a file.
///
/// Re-open on Error.
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
    type Output = File;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        loop {
            match this.fut.as_mut().poll(cx) {
                Poll::Ready(Ok(file)) => return Poll::Ready(file),
                Poll::Ready(Err(_)) => {
                    // return Pending and re-open on error
                    *this = FileFut::new(&this.path);
                    return Poll::Pending;
                }
                Poll::Pending => {}
            }
        }
    }
}

/// Kinds of interests that `poll_read` prodeced
#[derive(Debug, PartialEq)]
enum InterestKind {
    Modified,
    Created,
}

// Interest with Waker
#[derive(Debug)]
struct Interest {
    kind: InterestKind,
    waker: Waker,
}

pub struct TailingFile {
    file: MaybeDone<FileFut>,
    // FIFO
    interests: Arc<Mutex<VecDeque<Interest>>>,
    _watcher: RecommendedWatcher,
}

impl TailingFile {
    pub async fn open(path: impl AsRef<Path>) -> Result<Self, io::Error> {
        if path.as_ref().is_dir() {
            return Err(io::ErrorKind::IsADirectory.into());
        }
        // no more error on NotFound here
        let interests = Arc::new(Mutex::new(VecDeque::<Interest>::new()));
        let mut watcher = recommended_watcher({
            let path = path.as_ref().to_path_buf();
            let interests = interests.clone();
            move |e: Result<notify::Event, notify::Error>| {
                let Ok(e) = e else {
                    return;
                };
                // condition to lock
                if (e.kind.is_modify() || e.kind.is_create()) && e.paths.contains(&path) {
                    let mut interests = interests.lock();
                    for i in interests.drain(..).collect::<Vec<_>>() {
                        if (i.kind == InterestKind::Modified && e.kind.is_modify())
                            || (i.kind == InterestKind::Created && e.kind.is_create())
                        {
                            i.waker.wake();
                        } else {
                            // put back if not interested
                            interests.push_back(i);
                        }
                    }
                }
            }
        })
        .unwrap();
        watcher
            .watch(path.as_ref().parent().unwrap(), RecursiveMode::NonRecursive)
            .unwrap();
        Ok(TailingFile {
            file: maybe_done(FileFut::new(path)),
            interests,
            _watcher: watcher,
        })
    }

    fn register(&self, kind: InterestKind, waker: Waker) {
        let mut interests = self.interests.lock();
        if interests.iter().all(|i| !i.waker.will_wake(&waker)) {
            interests.push_back(Interest { kind, waker });
        }
    }
}

impl AsyncRead for TailingFile {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();

        loop {
            match Pin::new(&mut this.file).output_mut() {
                // opened, try read
                Some(file) => {
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
                    this.register(InterestKind::Modified, cx.waker().clone());
                    return Poll::Pending;
                }
                // not open yet
                None => match Pin::new(&mut this.file).poll(cx) {
                    Poll::Ready(()) => {}
                    Poll::Pending => {
                        // register waker and interested on file Created event
                        this.register(InterestKind::Created, cx.waker().clone());
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
