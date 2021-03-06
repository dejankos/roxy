use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::sync::mpsc::channel;
use std::thread;
use std::time::Duration;

use notify::{DebouncedEvent, RecursiveMode, watcher, Watcher};

type Listener = Arc<Mutex<Vec<Box<dyn Notify + Sync + Send>>>>;

pub trait Notify:  Sync + Send {
    fn change_event(&mut self, e: DebouncedEvent);

    fn path(&self) -> &Path;
}

pub struct FileWatcher {
    path: PathBuf,
    listeners: Listener,
}

impl FileWatcher {
    pub fn new<P>(path: P) -> Self
        where
            P: Into<PathBuf>
    {
        FileWatcher {
            path: path.into(),
            listeners: Arc::new(
                Mutex::new(vec![])),
        }
    }

    pub fn register_listener(&mut self, listener: Box<dyn Notify + Sync + Send>) {
        self.listeners.lock().unwrap().push(listener);
    }

    pub fn watch(&self) {
        let path = self.path.clone();
        thread::Builder::new()
            .name("file-watch-thread".into())
            .spawn(move || {
                let (tx, rx) = channel();
                let mut watcher = watcher(tx, Duration::from_secs(2)).unwrap();
                watcher.watch(path, RecursiveMode::NonRecursive).unwrap();

                for e in rx {
                    println!("events {:?}", e);
                }
            })
            .expect("Failed to register receiver thread");
    }
}

