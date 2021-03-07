use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use anyhow::Result;
use log::error;
use notify::{watcher, DebouncedEvent, RecursiveMode, Watcher};

type Listener = Box<dyn FileListener + Sync + Send>;
type Listeners = Arc<Mutex<Vec<Listener>>>;

pub trait FileListener: Sync + Send {
    fn notify_file_changed(&self, path: &PathBuf);
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
            listeners: Arc::new(Mutex::new(vec![])),
        }
    }

    pub fn register_listener(&mut self, listener: Listener) {
        self.listeners
            .lock()
            .expect("listener mutex poisoned!")
            .push(listener);
    }

    pub fn watch_file_changes(&self) -> Result<()> {
        let path = self.path.clone();
        let listeners = self.listeners.clone();
        thread::Builder::new()
            .name("file-watch-thread".into())
            .spawn(move || {
                if let Err(e) = run_event_loop(&path, listeners) {
                    error!("Error watching files on path {:?}. Error = {}", &path, e);
                }
            })?;
        Ok(())
    }
}

fn run_event_loop(path: &Path, listeners: Listeners) -> Result<()> {
    let (tx, rx) = channel();
    let mut watcher = watcher(tx, Duration::from_secs(5))?;
    watcher.watch(path, RecursiveMode::NonRecursive)?;

    loop {
        match rx.recv() {
            Ok(event) => {
                if let DebouncedEvent::Write(ref p) = event {
                    listeners
                        .lock()
                        .expect("listener mutex poisoned!")
                        .iter()
                        .for_each(|l| l.notify_file_changed(p));
                }
                //TODO watch errors ?
            }
            Err(e) => {
                error!("Error receiving file events - stopping file watch! {}", e);
                break;
            }
        }
    }
    Ok(())
}
