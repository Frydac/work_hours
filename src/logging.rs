// Shared tracing setup for native and WASM targets. The app uses tracing for
// its own diagnostics so auth/load/save failures can be followed end-to-end.

/// Installs a tracing subscriber appropriate for the current target.
pub fn init_tracing() {
    #[cfg(not(target_arch = "wasm32"))]
    {
        use tracing_subscriber::EnvFilter;

        let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_target(false)
            .without_time()
            .init();
    }

    #[cfg(target_arch = "wasm32")]
    {
        tracing_wasm::set_as_global_default();
    }
}
