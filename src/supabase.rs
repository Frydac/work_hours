use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WorkDay {
    pub id: String,
    pub user_id: String,
    pub day_date: String,
    pub time_entries: serde_json::Value,
}

pub struct SupabaseClient {
    pub url: String,
    pub api_key: String,
}

impl SupabaseClient {
    pub fn new(url: String, api_key: String) -> Self {
        SupabaseClient { url, api_key }
    }

    pub async fn get_work_day(&self, user_id: &str, day_date: &str) -> Result<Vec<WorkDay>, Box<dyn std::error::Error>> {
        let url = format!("{}/rest/v1/work_day?user_id=eq.{}&day_date=eq.{}", self.url, user_id, day_date);

        let client = reqwest::Client::new();
        let response = client.get(&url).header("apikey", &self.api_key).send().await?;

        let work_days = response.json::<Vec<WorkDay>>().await?;
        Ok(work_days)
    }

    pub async fn get_work_days_range(
        &self,
        user_id: &str,
        start_date: &str,
        end_date: &str,
    ) -> Result<Vec<WorkDay>, Box<dyn std::error::Error>> {
        let url = format!(
            "{}/rest/v1/work_day?user_id=eq.{}&day_date=gte.{}&day_date=lte.{}",
            self.url, user_id, start_date, end_date
        );

        let client = reqwest::Client::new();
        let response = client.get(&url).header("apikey", &self.api_key).send().await?;

        let work_days = response.json::<Vec<WorkDay>>().await?;
        Ok(work_days)
    }
}
