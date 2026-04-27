use anyhow::{anyhow, Result};
use tracing::debug;

// Public application configuration. These values are safe to ship in the
// client and tell the app which Supabase project to talk to.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppConfig {
    pub supabase_url: String,
    pub supabase_anon_key: String,
}

impl AppConfig {
    /// Loads the public Supabase config from the environment.
    ///
    /// Native builds prefer runtime environment variables so the same binary
    /// can be pointed at different projects. WASM builds read them at compile
    /// time because the browser cannot read the host process environment.
    pub fn load_public() -> Result<Self> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let supabase_url = std::env::var("SUPABASE_URL")
                .ok()
                .or_else(|| option_env!("SUPABASE_URL").map(ToOwned::to_owned))
                .ok_or_else(|| anyhow!("missing SUPABASE_URL"))?;
            let supabase_anon_key = std::env::var("SUPABASE_ANON_KEY")
                .ok()
                .or_else(|| option_env!("SUPABASE_ANON_KEY").map(ToOwned::to_owned))
                .ok_or_else(|| anyhow!("missing SUPABASE_ANON_KEY"))?;

            debug!(
                target = "config",
                has_supabase_url = true,
                has_supabase_anon_key = true,
                "loaded native Supabase public config"
            );

            Ok(Self {
                supabase_url,
                supabase_anon_key,
            })
        }

        #[cfg(target_arch = "wasm32")]
        {
            let supabase_url = option_env!("SUPABASE_URL").ok_or_else(|| anyhow!("missing SUPABASE_URL at compile time for wasm"))?;
            let supabase_anon_key =
                option_env!("SUPABASE_ANON_KEY").ok_or_else(|| anyhow!("missing SUPABASE_ANON_KEY at compile time for wasm"))?;

            debug!(
                target = "config",
                has_supabase_url = true,
                has_supabase_anon_key = true,
                "loaded wasm Supabase public config"
            );

            Ok(Self {
                supabase_url: supabase_url.to_owned(),
                supabase_anon_key: supabase_anon_key.to_owned(),
            })
        }
    }

    /// Returns a redacted Supabase host marker that is safe to show in logs.
    pub fn supabase_host_marker(&self) -> &str {
        self.supabase_url
            .split("//")
            .nth(1)
            .unwrap_or(&self.supabase_url)
            .split('/')
            .next()
            .unwrap_or(&self.supabase_url)
    }
}

#[cfg(test)]
mod tests {
    use super::AppConfig;

    #[test]
    fn app_config_type_is_cloneable() {
        let config = AppConfig {
            supabase_url: "https://example.supabase.co".to_string(),
            supabase_anon_key: "public-anon-key".to_string(),
        };

        assert_eq!(config.clone(), config);
    }
}
