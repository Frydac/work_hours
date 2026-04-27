use std::future::Future;
use std::sync::{Arc, Mutex};

// Small async bridge between background work and egui. Sync/auth code pushes
// completed results into a shared queue, and the app drains that queue during
// the next UI update.

pub(crate) type AsyncResults<T> = Arc<Mutex<Vec<T>>>;

pub(crate) fn new_async_results<T>() -> AsyncResults<T> {
    Arc::new(Mutex::new(Vec::new()))
}

pub(crate) fn take_async_results<T>(results: &AsyncResults<T>) -> Vec<T> {
    let mut queue = results.lock().unwrap();
    std::mem::take(&mut *queue)
}

fn push_async_result<T>(results: AsyncResults<T>, ctx: egui::Context, result: T) {
    results.lock().unwrap().push(result);
    ctx.request_repaint();
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn spawn_async_task<F, T>(ctx: egui::Context, results: AsyncResults<T>, future: F)
where
    F: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime should initialize");
        // Each background task uses its own lightweight runtime so the egui
        // thread does not need to be async-aware.
        let result = runtime.block_on(future);
        push_async_result(results, ctx, result);
    });
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn spawn_async_task<F, T>(ctx: egui::Context, results: AsyncResults<T>, future: F)
where
    F: Future<Output = T> + 'static,
    T: 'static,
{
    wasm_bindgen_futures::spawn_local(async move {
        let result = future.await;
        push_async_result(results, ctx, result);
    });
}
