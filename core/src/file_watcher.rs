use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use crossbeam::sync::ShardedLock;
use log::error;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use crate::task::spawn;

type Listener = Box<dyn FileListener + Sync + Send>;
type Listeners = Arc<ShardedLock<Vec<Listener>>>;

pub trait FileListener: Sync + Send {
    fn notify_file_changed(&self, path: &Path);
}

pub struct FileWatcher {
    path: PathBuf,
    listeners: Listeners,
}

impl FileWatcher {
    pub fn new<P>(base_path: P) -> Self
    where
        P: Into<PathBuf>,
    {
        FileWatcher {
            path: base_path.into(),
            listeners: Arc::new(ShardedLock::new(vec![])),
        }
    }

    pub fn register_listener(&self, listener: Listener) {
        self.listeners
            .write()
            .expect("Listener mutex poisoned!")
            .push(listener);
    }

    pub fn watch_file_changes(&self) -> Result<()> {
        let path = self.path.clone();
        let listeners = self.listeners.clone();
        spawn(
            move || {
                if let Err(e) = run_event_loop(&path, listeners) {
                    error!("Error watching files on path {:?}. Error = {}", &path, e);
                }
            },
            "file-watch-thread".into(),
        )?;
        Ok(())
    }
}

fn run_event_loop(path: &Path, listeners: Listeners) -> Result<()> {
    let (tx, rx) = channel();

    let mut watcher = RecommendedWatcher::new(
        move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                let _ = tx.send(event);
            }
        },
        Config::default().with_poll_interval(Duration::from_secs(5)),
    )?;

    watcher.watch(path, RecursiveMode::NonRecursive)?;

    // Keep watcher alive in the loop
    loop {
        match rx.recv() {
            Ok(event) => {
                if matches!(event.kind, EventKind::Modify(_)) {
                    for path in event.paths {
                        listeners
                            .read()
                            .expect("listener mutex poisoned!")
                            .iter()
                            .for_each(|l| l.notify_file_changed(&path));
                    }
                }
            }
            Err(e) => {
                error!("Error receiving file events - stopping file watch! {}", e);
                drop(watcher); // Explicitly drop watcher before breaking
                break;
            }
        }
    }
    Ok(())
}
