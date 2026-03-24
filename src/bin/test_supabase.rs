use work_hours_calculator::supabase::SupabaseClient;

#[tokio::main]
async fn main() {
    println!("🚀 Fetching work days from Supabase...");

    // Your Supabase credentials
    let supabase_url = "https://YOUR_PROJECT_ID.supabase.co".to_string();
    let api_key = "YOUR_ANON_KEY".to_string();
    let user_id = "YOUR_USER_ID"; // The UUID from auth.users

    let client = SupabaseClient::new(supabase_url, api_key);

    // Test 1: Fetch a specific day
    match client.get_work_day(user_id, "2026-01-27").await {
        Ok(days) => {
            println!("✅ Success! Fetched {} day(s)", days.len());
            for day in days {
                println!("\n{:#?}", day);
            }
        }
        Err(e) => {
            println!("❌ Error: {}", e);
        }
    }

    // Test 2: Fetch a date range
    println!("\n\n📅 Fetching week of Jan 27 - Feb 2...");
    match client.get_work_days_range(user_id, "2026-01-27", "2026-02-02").await {
        Ok(days) => {
            println!("✅ Fetched {} day(s) in range", days.len());
            for day in days {
                println!("{}: {:?}", day.day_date, day.time_entries);
            }
        }
        Err(e) => {
            println!("❌ Error: {}", e);
        }
    }
}
