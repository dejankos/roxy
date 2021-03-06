use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::channel;
use std::thread;
use std::time::Duration;

use anyhow::Result;
use log::error;
use notify::{DebouncedEvent, RecursiveMode, watcher, Watcher};

type Listeners = Arc<Mutex<Vec<Box<dyn Notify + Sync + Send>>>>;

pub trait Notify: Sync + Send {
    fn change_event(&self, e: &DebouncedEvent);
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

    pub fn register_listener(&mut self, listener: Box<dyn Notify + Sync + Send>) {
        self.listeners
            .lock()
            .expect("listener mutex poisoned!")
            .push(listener);
    }

    pub fn watch(&self) -> Result<()> {
        let path = self.path.clone();
        let listeners = self.listeners.clone();
        thread::Builder::new()
            .name("file-watch-thread".into())
            .spawn(move || -> Result<()> {
                let (tx, rx) = channel();
                let mut watcher = watcher(tx, Duration::from_secs(5))?;
                watcher.watch(path, RecursiveMode::NonRecursive)?;

                loop {
                    match rx.recv() {
                        Ok(event) => match event {
                            DebouncedEvent::Write(_) => {
                                listeners
                                    .lock()
                                    .expect("listener mutex poisoned!")
                                    .iter_mut()
                                    .for_each(|l| l.change_event(&event));
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
            })?;
        Ok(())
    }
}
