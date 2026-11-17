use crate::db::Db;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailAddress {
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Personalization {
    pub to: Vec<EmailAddress>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sender {
    pub email: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Content {
    #[serde(rename = "type")]
    pub content_type: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendGridPayload {
    pub personalizations: Vec<Personalization>,
    pub from: Sender,
    pub subject: String,
    pub content: Vec<Content>,
}

impl SendGridPayload {
    pub fn new(
        to_email: &str,
        from_email: &str,
        from_name: &str,
        subject: &str,
        body: &str,
    ) -> Self {
        Self {
            personalizations: vec![Personalization {
                to: vec![EmailAddress {
                    email: to_email.trim().to_string(),
                }],
            }],
            from: Sender {
                email: from_email.trim().to_string(),
                name: from_name.trim().to_string(),
            },
            subject: subject.to_string(),
            content: vec![Content {
                content_type: "text/plain".to_string(),
                value: body.to_string(),
            }],
        }
    }
}

#[derive(Clone, Debug)]
pub struct SendGridMailer {
    client: reqwest::Client,
    pub api_key: String,
    pub from_email: String,
    pub from_name: String,
    pub endpoint: String,
}

impl SendGridMailer {
    pub fn new(api_key: String, from_email: String, from_name: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
            from_email,
            from_name,
            endpoint: "https://api.sendgrid.com/v3/mail/send".to_string(),
        }
    }

    pub fn from_env() -> Option<Self> {
        let api_key = std::env::var("SENDGRID_API_KEY").ok()?.trim().to_string();
        if api_key.is_empty() {
            return None;
        }
        let from_email = std::env::var("SENDGRID_FROM_EMAIL")
            .unwrap_or_else(|_| "no-reply@example.com".to_string());
        let from_name = std::env::var("SENDGRID_FROM_NAME")
            .unwrap_or_else(|_| "Mantis Bug Tracker".to_string());
        let mut mailer = Self::new(api_key, from_email, from_name);
        if let Ok(endpoint) = std::env::var("SENDGRID_ENDPOINT") {
            if !endpoint.trim().is_empty() {
                mailer.endpoint = endpoint.trim().to_string();
            }
        }
        Some(mailer)
    }

    pub fn with_endpoint(mut self, endpoint: String) -> Self {
        self.endpoint = endpoint;
        self
    }

    pub async fn send(&self, to_email: &str, subject: &str, body: &str) -> Result<(), String> {
        let payload =
            SendGridPayload::new(to_email, &self.from_email, &self.from_name, subject, body);

        let res = self
            .client
            .post(&self.endpoint)
            .bearer_auth(&self.api_key)
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("Failed to send request to SendGrid: {}", e))?;

        let status = res.status();
        if status.is_success() {
            Ok(())
        } else {
            let error_text = res
                .text()
                .await
                .unwrap_or_else(|_| "<failed to read response text>".to_string());
            Err(format!(
                "SendGrid API returned status {}: {}",
                status, error_text
            ))
        }
    }

    pub async fn dispatch_pending_queue(&self, db: &Db) -> Result<usize, String> {
        let emails = db
            .list_queued_emails()
            .map_err(|e| format!("Database query error: {}", e))?;

        let mut sent_count = 0;
        for item in emails {
            match self.send(&item.email, &item.subject, &item.body).await {
                Ok(_) => {
                    if let Err(e) = db.delete_queued_email(item.email_id) {
                        eprintln!(
                            "[mailer] Failed to remove sent email ID {} from queue: {}",
                            item.email_id, e
                        );
                    }
                    sent_count += 1;
                }
                Err(e) => {
                    eprintln!(
                        "[mailer] Failed to send email ID {} to {}: {}",
                        item.email_id, item.email, e
                    );
                }
            }
        }
        Ok(sent_count)
    }
}
