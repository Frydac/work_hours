use crate::supabase::StoredSession;

// Purely UI-facing state for auth and sync surfaces. Keeping this separate from
// `SyncState` lets the sync layer focus on orchestration rather than form fields
// and banners.
#[derive(serde::Deserialize, serde::Serialize, Default)]
#[serde(default)]
pub struct AppUiState {
    #[serde(skip)]
    show_login_window: bool,
    #[serde(skip)]
    login_email: String,
    #[serde(skip)]
    login_password: String,
    #[serde(skip)]
    status_message: Option<String>,
    #[serde(skip)]
    error_message: Option<String>,
}

impl AppUiState {
    pub fn adopt_stored_session_email(&mut self, stored_session: Option<&StoredSession>) {
        if self.login_email.is_empty() {
            if let Some(email) = stored_session.and_then(|session| session.email.clone()) {
                self.login_email = email;
            }
        }
    }

    pub fn clear_error(&mut self) {
        self.error_message = None;
    }

    pub fn set_error_message(&mut self, message: String) {
        self.error_message = Some(message);
    }

    pub fn set_status_message(&mut self, message: String) {
        self.status_message = Some(message);
    }

    pub fn status_message(&self) -> Option<&str> {
        self.status_message.as_deref()
    }

    pub fn error_message(&self) -> Option<&str> {
        self.error_message.as_deref()
    }

    pub fn show_login_window(&self) -> bool {
        self.show_login_window
    }

    pub fn set_show_login_window(&mut self, show: bool) {
        self.show_login_window = show;
    }

    pub fn login_email_mut(&mut self) -> &mut String {
        &mut self.login_email
    }

    pub fn login_password_mut(&mut self) -> &mut String {
        &mut self.login_password
    }

    pub fn login_email(&self) -> &str {
        &self.login_email
    }

    pub fn login_password(&self) -> &str {
        &self.login_password
    }

    pub fn clear_login_password(&mut self) {
        self.login_password.clear();
    }

    pub fn set_login_email(&mut self, email: String) {
        self.login_email = email;
    }
}
