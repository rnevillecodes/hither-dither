//! Running plugin calls on the UI thread and waiting for their results.
//!
//! VST3 requires controller, view and setup calls on the UI thread. Commands run on Tauri's
//! worker threads, so they post a closure to the main thread through a [`UiThread`] and block
//! until it has run. Tests and the scanner subprocess have no event loop and use [`Inline`].

use std::sync::OnceLock;
use std::sync::mpsc;
use std::thread::ThreadId;

/// Somewhere to run closures that must run on the UI thread.
pub trait UiThread: Send + Sync {
    /// Queues `f` to run on the UI thread. Fails when the event loop has gone, during quit.
    fn post(&self, f: Box<dyn FnOnce() + Send>) -> Result<(), Closed>;

    /// Whether the calling thread is the UI thread, so `run` must not wait on itself.
    fn is_current(&self) -> bool;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("the app is closing")]
pub struct Closed;

/// Runs `f` on the UI thread and returns its result, running it directly when already there.
pub fn run<T: Send + 'static>(
    ui: &dyn UiThread,
    f: impl FnOnce() -> T + Send + 'static,
) -> Result<T, Closed> {
    if ui.is_current() {
        return Ok(f());
    }
    let (tx, rx) = mpsc::sync_channel(1);
    ui.post(Box::new(move || {
        let _ = tx.send(f());
    }))?;
    rx.recv().map_err(|_| Closed)
}

/// Runs everything on the calling thread, for tests and the scanner subprocess, which drive
/// plugins from one thread with no event loop.
pub struct Inline;

impl UiThread for Inline {
    fn post(&self, f: Box<dyn FnOnce() + Send>) -> Result<(), Closed> {
        f();
        Ok(())
    }

    fn is_current(&self) -> bool {
        true
    }
}

static MAIN: OnceLock<ThreadId> = OnceLock::new();

/// Records the calling thread as the app's main thread. Call once, from `main`, before Tauri
/// starts its event loop there.
pub fn mark_main_thread() {
    let _ = MAIN.set(std::thread::current().id());
}

impl<R: tauri::Runtime> UiThread for tauri::AppHandle<R> {
    fn post(&self, f: Box<dyn FnOnce() + Send>) -> Result<(), Closed> {
        self.run_on_main_thread(f).map_err(|_| Closed)
    }

    fn is_current(&self) -> bool {
        MAIN.get() == Some(&std::thread::current().id())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// A UI thread backed by a plain thread with a job queue.
    struct Worker(Mutex<mpsc::Sender<Box<dyn FnOnce() + Send>>>, ThreadId);

    impl Worker {
        fn spawn() -> Arc<Worker> {
            let (tx, rx) = mpsc::channel::<Box<dyn FnOnce() + Send>>();
            let handle = std::thread::spawn(move || rx.into_iter().for_each(|f| f()));
            Arc::new(Worker(Mutex::new(tx), handle.thread().id()))
        }
    }

    impl UiThread for Worker {
        fn post(&self, f: Box<dyn FnOnce() + Send>) -> Result<(), Closed> {
            self.0.lock().unwrap().send(f).map_err(|_| Closed)
        }

        fn is_current(&self) -> bool {
            std::thread::current().id() == self.1
        }
    }

    #[test]
    fn runs_on_the_ui_thread_and_returns_the_result() {
        let ui = Worker::spawn();
        let ui_id = ui.1;
        let (id, value) = run(&*ui, || (std::thread::current().id(), 42)).unwrap();
        assert_eq!((id, value), (ui_id, 42));
        // Nested calls from the UI thread run directly instead of deadlocking.
        let ui2 = ui.clone();
        let nested = run(&*ui, move || run(&*ui2, || 7).unwrap()).unwrap();
        assert_eq!(nested, 7);
    }
}
