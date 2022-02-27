use std::thread;
use std::thread::JoinHandle;

pub fn spawn<F, T>(f: F, t_name: String) -> std::io::Result<JoinHandle<T>>
    where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static
{
    thread::Builder::new()
        .name(t_name)
        .spawn(f)
}