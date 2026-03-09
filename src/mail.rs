/// Mail reading module - reads Outlook/Hotmail emails

pub struct MailMessage {
    pub id: String,
    pub subject: String,
    pub from: String,
    pub date: String,
    pub preview: Option<String>,
}

impl std::fmt::Display for MailMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{}]\n  Subject: {}\n  From: {}\n  Date: {}",
            self.id,
            self.subject.chars().take(60).collect::<String>(),
            self.from,
            self.date
        )
    }
}

pub struct MailReader {
    pub access_token: Option<String>,
}

impl MailReader {
    pub fn new() -> Self {
        Self {
            access_token: None,
        }
    }

    /// Read emails using Graph API with OAuth token
    pub fn read_emails(&self, limit: usize) -> Result<Vec<MailMessage>, String> {
        if self.access_token.is_none() {
            return Err(
                "Not authenticated. Use 'axcli mail-auth' first to complete OAuth login"
                    .to_string(),
            );
        }

        let token = self.access_token.as_ref().unwrap();

        // Build Graph API request
        let client = reqwest::blocking::Client::new();
        let url = format!(
            "https://graph.microsoft.com/v1.0/me/messages?$top={}&$orderby=receivedDateTime desc",
            limit
        );

        let response = client
            .get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .map_err(|e| format!("HTTP request failed: {}", e))?;

        if response.status() != 200 {
            return Err(format!("API error: HTTP {}", response.status()));
        }

        let body: serde_json::Value = response
            .json()
            .map_err(|e| format!("JSON parse error: {}", e))?;

        let mut messages = Vec::new();

        if let Some(values) = body["value"].as_array() {
            for msg in values {
                let id = msg["id"].as_str().unwrap_or("").to_string();
                let subject = msg["subject"].as_str().unwrap_or("(No subject)").to_string();
                let from = msg["from"]["emailAddress"]["address"]
                    .as_str()
                    .unwrap_or("(Unknown)")
                    .to_string();
                let date = msg["receivedDateTime"]
                    .as_str()
                    .unwrap_or("(No date)")
                    .to_string();
                let preview = msg["bodyPreview"].as_str().map(|s| s.to_string());

                messages.push(MailMessage {
                    id,
                    subject,
                    from,
                    date,
                    preview,
                });
            }
        }

        Ok(messages)
    }

    /// Get OAuth authorization URL
    pub fn get_auth_url() -> String {
        let client_id = "04b07795-8ddb-461a-bbee-02f9e1bf7b46";
        let redirect_uri = "http://localhost:8888/callback";
        let scopes = "Mail.Read";

        format!(
            "https://login.microsoftonline.com/common/oauth2/v2.0/authorize?client_id={}&response_type=code&scope={}&redirect_uri={}&response_mode=query",
            client_id, scopes, urlencoding::encode(redirect_uri)
        )
    }
}
