use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use crossbeam::sync::ShardedLock;
use log::error;
use notify::{watcher, DebouncedEvent, RecursiveMode, Watcher};

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
    let mut watcher = watcher(tx, Duration::from_secs(5))?;
    watcher.watch(path, RecursiveMode::NonRecursive)?;

    loop {
        match rx.recv() {
            Ok(event) => match event {
                DebouncedEvent::Write(ref p) => listeners
                    .read()
                    .expect("listener mutex poisoned!")
                    .iter()
                    .for_each(|l| l.notify_file_changed(p)),
                DebouncedEvent::Error(e, o) => {
                    error!("Path {:?} watch error {}.", o, e);
                }
                _ => {}
            },
            Err(e) => {
                error!("Error receiving file events - stopping file watch! {}", e);
                break;
            }
        }
    }
    Ok(())
}
