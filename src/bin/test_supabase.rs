#[cfg(not(target_arch = "wasm32"))]
use chrono::NaiveDate;
#[cfg(not(target_arch = "wasm32"))]
use work_hours_calculator::config::AppConfig;
#[cfg(not(target_arch = "wasm32"))]
use work_hours_calculator::supabase::SupabaseClient;

#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(not(target_arch = "wasm32"))]
#[tokio::main]
async fn main() {
    let config = AppConfig::load_public().expect("public Supabase config should be available");
    let email = std::env::var("SUPABASE_EMAIL").expect("SUPABASE_EMAIL must be set");
    let password = std::env::var("SUPABASE_PASSWORD").expect("SUPABASE_PASSWORD must be set");

    let client = SupabaseClient::new(config.supabase_url, config.supabase_anon_key);
    let session = client
        .sign_in_password(&email, &password)
        .await
        .expect("password sign-in should succeed");

    println!("signed in as {:?}", session.user.email);

    let start = NaiveDate::from_ymd_opt(2026, 1, 27).unwrap();
    let end = NaiveDate::from_ymd_opt(2026, 2, 2).unwrap();
    let days = client
        .get_work_days_range(&session.access_token, start, end)
        .await
        .expect("range fetch should succeed");

    println!("fetched {} day(s)", days.len());
    for day in days {
        println!(
            "{} target={} enabled={} entries={}",
            day.day.work_date,
            day.day.target_minutes,
            day.day.enabled,
            day.work_entries.len()
        );
        for entry in day.work_entries {
            println!(
                "  [{}] {} -> {} {}",
                entry.sort_index, entry.starts_at, entry.ends_at, entry.metadata
            );
        }
    }
}
