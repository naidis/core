use async_imap::Session;
use async_native_tls::TlsStream;
use async_std::net::TcpStream;
use chrono::{DateTime, Utc};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum NewsletterError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("IMAP error: {0}")]
    Imap(String),
    #[error("TLS error: {0}")]
    Tls(String),
    #[error("Mail parse error: {0}")]
    Parse(String),
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Not configured")]
    NotConfigured,
}

impl From<async_imap::error::Error> for NewsletterError {
    fn from(e: async_imap::error::Error) -> Self {
        NewsletterError::Imap(e.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImapConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub use_tls: bool,
    pub folder: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Newsletter {
    pub id: String,
    pub message_id: Option<String>,
    pub from_name: Option<String>,
    pub from_email: String,
    pub subject: String,
    pub content_text: String,
    pub content_html: Option<String>,
    pub received_at: DateTime<Utc>,
    pub saved_at: DateTime<Utc>,
    pub is_read: bool,
    pub is_starred: bool,
    pub labels: Vec<String>,
    pub sender_info: Option<SenderInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SenderInfo {
    pub name: String,
    pub email: String,
    pub is_newsletter: bool,
    pub article_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchNewslettersRequest {
    pub config: ImapConfig,
    pub limit: Option<usize>,
    pub since: Option<DateTime<Utc>>,
    pub sender_filter: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsletterQuery {
    pub sender_email: Option<String>,
    pub is_read: Option<bool>,
    pub is_starred: Option<bool>,
    pub labels: Option<Vec<String>>,
    pub search: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsletterToMarkdownRequest {
    pub id: String,
    pub include_metadata: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionRule {
    pub id: String,
    pub sender_pattern: String,
    pub auto_save: bool,
    pub auto_labels: Vec<String>,
    pub save_folder: Option<String>,
}

pub struct NewsletterStore {
    data_dir: PathBuf,
    newsletters: HashMap<String, Newsletter>,
    senders: HashMap<String, SenderInfo>,
    rules: Vec<SubscriptionRule>,
}

impl NewsletterStore {
    pub fn new(data_dir: PathBuf) -> Result<Self, NewsletterError> {
        let newsletter_dir = data_dir.join("newsletters");
        fs::create_dir_all(&newsletter_dir)?;

        let mut store = Self {
            data_dir: newsletter_dir,
            newsletters: HashMap::new(),
            senders: HashMap::new(),
            rules: Vec::new(),
        };
        store.load_all()?;
        Ok(store)
    }

    fn load_all(&mut self) -> Result<(), NewsletterError> {
        let newsletters_path = self.data_dir.join("newsletters.json");
        if newsletters_path.exists() {
            let data = fs::read_to_string(&newsletters_path)?;
            self.newsletters = serde_json::from_str(&data)?;
        }

        let senders_path = self.data_dir.join("senders.json");
        if senders_path.exists() {
            let data = fs::read_to_string(&senders_path)?;
            self.senders = serde_json::from_str(&data)?;
        }

        let rules_path = self.data_dir.join("rules.json");
        if rules_path.exists() {
            let data = fs::read_to_string(&rules_path)?;
            self.rules = serde_json::from_str(&data)?;
        }

        Ok(())
    }

    fn save_all(&self) -> Result<(), NewsletterError> {
        let newsletters_path = self.data_dir.join("newsletters.json");
        fs::write(
            &newsletters_path,
            serde_json::to_string_pretty(&self.newsletters)?,
        )?;

        let senders_path = self.data_dir.join("senders.json");
        fs::write(&senders_path, serde_json::to_string_pretty(&self.senders)?)?;

        let rules_path = self.data_dir.join("rules.json");
        fs::write(&rules_path, serde_json::to_string_pretty(&self.rules)?)?;

        Ok(())
    }

    pub fn add(&mut self, newsletter: Newsletter) -> Result<Newsletter, NewsletterError> {
        let sender_email = newsletter.from_email.clone();

        self.senders
            .entry(sender_email.clone())
            .and_modify(|s| s.article_count += 1)
            .or_insert_with(|| SenderInfo {
                name: newsletter.from_name.clone().unwrap_or_default(),
                email: sender_email.clone(),
                is_newsletter: true,
                article_count: 1,
            });

        self.newsletters
            .insert(newsletter.id.clone(), newsletter.clone());
        self.save_all()?;
        Ok(newsletter)
    }

    pub fn get(&self, id: &str) -> Option<&Newsletter> {
        self.newsletters.get(id)
    }

    pub fn query(&self, q: NewsletterQuery) -> Vec<&Newsletter> {
        let mut results: Vec<&Newsletter> = self
            .newsletters
            .values()
            .filter(|n| {
                if let Some(ref sender) = q.sender_email {
                    if !n.from_email.contains(sender) {
                        return false;
                    }
                }
                if let Some(is_read) = q.is_read {
                    if n.is_read != is_read {
                        return false;
                    }
                }
                if let Some(is_starred) = q.is_starred {
                    if n.is_starred != is_starred {
                        return false;
                    }
                }
                if let Some(ref labels) = q.labels {
                    if !labels.iter().any(|l| n.labels.contains(l)) {
                        return false;
                    }
                }
                if let Some(ref search) = q.search {
                    let search_lower = search.to_lowercase();
                    let in_subject = n.subject.to_lowercase().contains(&search_lower);
                    let in_content = n.content_text.to_lowercase().contains(&search_lower);
                    let in_sender = n.from_email.to_lowercase().contains(&search_lower)
                        || n.from_name
                            .as_ref()
                            .map(|name| name.to_lowercase().contains(&search_lower))
                            .unwrap_or(false);
                    if !in_subject && !in_content && !in_sender {
                        return false;
                    }
                }
                true
            })
            .collect();

        results.sort_by(|a, b| b.received_at.cmp(&a.received_at));

        let offset = q.offset.unwrap_or(0);
        let limit = q.limit.unwrap_or(50);
        results.into_iter().skip(offset).take(limit).collect()
    }

    pub fn mark_read(&mut self, id: &str) -> Result<(), NewsletterError> {
        let newsletter = self
            .newsletters
            .get_mut(id)
            .ok_or_else(|| NewsletterError::NotFound(id.to_string()))?;
        newsletter.is_read = true;
        self.save_all()
    }

    pub fn toggle_star(&mut self, id: &str) -> Result<bool, NewsletterError> {
        let newsletter = self
            .newsletters
            .get_mut(id)
            .ok_or_else(|| NewsletterError::NotFound(id.to_string()))?;
        newsletter.is_starred = !newsletter.is_starred;
        let starred = newsletter.is_starred;
        self.save_all()?;
        Ok(starred)
    }

    pub fn delete(&mut self, id: &str) -> Result<(), NewsletterError> {
        self.newsletters
            .remove(id)
            .ok_or_else(|| NewsletterError::NotFound(id.to_string()))?;
        self.save_all()
    }

    pub fn get_senders(&self) -> Vec<&SenderInfo> {
        self.senders.values().collect()
    }

    pub fn add_rule(&mut self, rule: SubscriptionRule) -> Result<(), NewsletterError> {
        self.rules.push(rule);
        self.save_all()
    }

    pub fn get_rules(&self) -> &[SubscriptionRule] {
        &self.rules
    }

    pub fn to_markdown(&self, id: &str, include_metadata: bool) -> Result<String, NewsletterError> {
        let newsletter = self
            .newsletters
            .get(id)
            .ok_or_else(|| NewsletterError::NotFound(id.to_string()))?;

        let mut output = String::new();

        output.push_str(&format!("# {}\n\n", newsletter.subject));

        if include_metadata {
            let from = newsletter
                .from_name
                .as_ref()
                .map(|name| format!("{} <{}>", name, newsletter.from_email))
                .unwrap_or_else(|| newsletter.from_email.clone());

            output.push_str(&format!("- **From**: {}\n", from));
            output.push_str(&format!(
                "- **Date**: {}\n",
                newsletter.received_at.format("%Y-%m-%d %H:%M")
            ));

            if !newsletter.labels.is_empty() {
                let tags: Vec<String> = newsletter
                    .labels
                    .iter()
                    .map(|l| format!("#{}", l))
                    .collect();
                output.push_str(&format!("- **Tags**: {}\n", tags.join(" ")));
            }

            output.push_str("\n---\n\n");
        }

        output.push_str(&newsletter.content_text);

        Ok(output)
    }
}

async fn connect_imap(
    config: &ImapConfig,
) -> Result<Session<TlsStream<TcpStream>>, NewsletterError> {
    let addr = format!("{}:{}", config.host, config.port);
    let tcp_stream = TcpStream::connect(&addr)
        .await
        .map_err(|e| NewsletterError::Imap(format!("Failed to connect: {}", e)))?;

    let tls = async_native_tls::TlsConnector::new();
    let tls_stream = tls
        .connect(&config.host, tcp_stream)
        .await
        .map_err(|e| NewsletterError::Tls(e.to_string()))?;

    let client = async_imap::Client::new(tls_stream);
    let session = client
        .login(&config.username, &config.password)
        .await
        .map_err(|(e, _)| NewsletterError::Imap(format!("Login failed: {}", e)))?;

    Ok(session)
}

fn parse_email(raw: &[u8]) -> Result<Newsletter, NewsletterError> {
    let parsed = mailparse::parse_mail(raw).map_err(|e| NewsletterError::Parse(e.to_string()))?;

    let headers = &parsed.headers;

    let from = headers
        .iter()
        .find(|h| h.get_key().to_lowercase() == "from")
        .map(|h| h.get_value())
        .unwrap_or_default();

    let (from_name, from_email) = parse_email_address(&from);

    let subject = headers
        .iter()
        .find(|h| h.get_key().to_lowercase() == "subject")
        .map(|h| h.get_value())
        .unwrap_or_else(|| "(No Subject)".to_string());

    let message_id = headers
        .iter()
        .find(|h| h.get_key().to_lowercase() == "message-id")
        .map(|h| h.get_value());

    let date_str = headers
        .iter()
        .find(|h| h.get_key().to_lowercase() == "date")
        .map(|h| h.get_value());

    let received_at = date_str
        .and_then(|d| mailparse::dateparse(&d).ok())
        .map(|ts| DateTime::from_timestamp(ts, 0).unwrap_or_else(Utc::now))
        .unwrap_or_else(Utc::now);

    let (content_text, content_html) = extract_content(&parsed);

    Ok(Newsletter {
        id: Uuid::new_v4().to_string(),
        message_id,
        from_name,
        from_email,
        subject,
        content_text,
        content_html,
        received_at,
        saved_at: Utc::now(),
        is_read: false,
        is_starred: false,
        labels: Vec::new(),
        sender_info: None,
    })
}

fn parse_email_address(addr: &str) -> (Option<String>, String) {
    if let Some(start) = addr.find('<') {
        if let Some(end) = addr.find('>') {
            let name = addr[..start].trim().trim_matches('"').to_string();
            let email = addr[start + 1..end].trim().to_string();
            return (if name.is_empty() { None } else { Some(name) }, email);
        }
    }
    (None, addr.trim().to_string())
}

fn extract_content(mail: &mailparse::ParsedMail) -> (String, Option<String>) {
    let mut text_content = String::new();
    let mut html_content: Option<String> = None;

    if mail.subparts.is_empty() {
        let body = mail.get_body().unwrap_or_default();
        let content_type = mail.ctype.mimetype.to_lowercase();

        if content_type.contains("text/plain") {
            text_content = body;
        } else if content_type.contains("text/html") {
            html_content = Some(body.clone());
            text_content = html2text::from_read(body.as_bytes(), 80);
        }
    } else {
        for part in &mail.subparts {
            let (text, html) = extract_content(part);
            if !text.is_empty() && text_content.is_empty() {
                text_content = text;
            }
            if html.is_some() && html_content.is_none() {
                html_content = html;
            }
        }
    }

    if text_content.is_empty() {
        if let Some(ref html) = html_content {
            text_content = html2text::from_read(html.as_bytes(), 80);
        }
    }

    (text_content, html_content)
}

pub async fn fetch_newsletters(
    data_dir: PathBuf,
    req: FetchNewslettersRequest,
) -> Result<Vec<Newsletter>, NewsletterError> {
    let mut session = connect_imap(&req.config).await?;

    let folder = req.config.folder.as_deref().unwrap_or("INBOX");
    session.select(folder).await?;

    let limit = req.limit.unwrap_or(50);

    let search_query = if let Some(since) = req.since {
        format!("SINCE {}", since.format("%d-%b-%Y"))
    } else {
        "ALL".to_string()
    };

    let messages = session.search(&search_query).await?;

    let mut message_ids: Vec<u32> = messages.into_iter().collect();
    message_ids.sort_by(|a, b| b.cmp(a));
    message_ids.truncate(limit);

    let mut store = NewsletterStore::new(data_dir)?;
    let mut newsletters = Vec::new();

    for msg_id in message_ids {
        let mut fetches = session.fetch(msg_id.to_string(), "RFC822").await?;

        while let Some(fetch_result) = fetches.next().await {
            if let Ok(fetch) = fetch_result {
                if let Some(body) = fetch.body() {
                    match parse_email(body) {
                        Ok(newsletter) => {
                            if let Some(ref sender_filter) = req.sender_filter {
                                if !sender_filter
                                    .iter()
                                    .any(|s| newsletter.from_email.contains(s))
                                {
                                    continue;
                                }
                            }

                            let saved = store.add(newsletter)?;
                            newsletters.push(saved);
                        }
                        Err(e) => {
                            tracing::warn!("Failed to parse email: {}", e);
                        }
                    }
                }
            }
        }
    }

    session.logout().await?;
    Ok(newsletters)
}

pub fn query_newsletters(
    data_dir: PathBuf,
    query: NewsletterQuery,
) -> Result<Vec<Newsletter>, NewsletterError> {
    let store = NewsletterStore::new(data_dir)?;
    Ok(store.query(query).into_iter().cloned().collect())
}

pub fn get_newsletter(data_dir: PathBuf, id: &str) -> Result<Option<Newsletter>, NewsletterError> {
    let store = NewsletterStore::new(data_dir)?;
    Ok(store.get(id).cloned())
}

pub fn mark_newsletter_read(data_dir: PathBuf, id: &str) -> Result<(), NewsletterError> {
    let mut store = NewsletterStore::new(data_dir)?;
    store.mark_read(id)
}

pub fn toggle_newsletter_star(data_dir: PathBuf, id: &str) -> Result<bool, NewsletterError> {
    let mut store = NewsletterStore::new(data_dir)?;
    store.toggle_star(id)
}

pub fn delete_newsletter(data_dir: PathBuf, id: &str) -> Result<(), NewsletterError> {
    let mut store = NewsletterStore::new(data_dir)?;
    store.delete(id)
}

pub fn newsletter_to_markdown(
    data_dir: PathBuf,
    id: &str,
    include_metadata: bool,
) -> Result<String, NewsletterError> {
    let store = NewsletterStore::new(data_dir)?;
    store.to_markdown(id, include_metadata)
}

pub fn get_newsletter_senders(data_dir: PathBuf) -> Result<Vec<SenderInfo>, NewsletterError> {
    let store = NewsletterStore::new(data_dir)?;
    Ok(store.get_senders().into_iter().cloned().collect())
}

pub fn add_subscription_rule(
    data_dir: PathBuf,
    rule: SubscriptionRule,
) -> Result<(), NewsletterError> {
    let mut store = NewsletterStore::new(data_dir)?;
    store.add_rule(rule)
}

pub fn get_subscription_rules(data_dir: PathBuf) -> Result<Vec<SubscriptionRule>, NewsletterError> {
    let store = NewsletterStore::new(data_dir)?;
    Ok(store.get_rules().to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn create_test_newsletter() -> Newsletter {
        Newsletter {
            id: "test-123".to_string(),
            message_id: Some("<test@example.com>".to_string()),
            from_name: Some("Test Sender".to_string()),
            from_email: "sender@example.com".to_string(),
            subject: "Test Newsletter".to_string(),
            content_text: "This is the newsletter content.".to_string(),
            content_html: Some("<p>This is the newsletter content.</p>".to_string()),
            received_at: Utc::now(),
            saved_at: Utc::now(),
            is_read: false,
            is_starred: false,
            labels: vec!["tech".to_string()],
            sender_info: None,
        }
    }

    #[test]
    fn test_parse_email_address_with_name() {
        let (name, email) = parse_email_address("John Doe <john@example.com>");
        assert_eq!(name, Some("John Doe".to_string()));
        assert_eq!(email, "john@example.com");
    }

    #[test]
    fn test_parse_email_address_with_quoted_name() {
        let (name, email) = parse_email_address("\"John Doe\" <john@example.com>");
        assert_eq!(name, Some("John Doe".to_string()));
        assert_eq!(email, "john@example.com");
    }

    #[test]
    fn test_parse_email_address_without_name() {
        let (name, email) = parse_email_address("<john@example.com>");
        assert_eq!(name, None);
        assert_eq!(email, "john@example.com");
    }

    #[test]
    fn test_parse_email_address_plain() {
        let (name, email) = parse_email_address("john@example.com");
        assert_eq!(name, None);
        assert_eq!(email, "john@example.com");
    }

    #[test]
    fn test_newsletter_store_add() {
        let dir = tempdir().unwrap();
        let mut store = NewsletterStore::new(dir.path().to_path_buf()).unwrap();

        let newsletter = store.add(create_test_newsletter()).unwrap();

        assert_eq!(newsletter.subject, "Test Newsletter");
        assert!(store.get("test-123").is_some());
    }

    #[test]
    fn test_newsletter_store_sender_tracking() {
        let dir = tempdir().unwrap();
        let mut store = NewsletterStore::new(dir.path().to_path_buf()).unwrap();

        store.add(create_test_newsletter()).unwrap();

        let senders = store.get_senders();
        assert_eq!(senders.len(), 1);
        assert_eq!(senders[0].email, "sender@example.com");
        assert_eq!(senders[0].article_count, 1);

        let mut second = create_test_newsletter();
        second.id = "test-456".to_string();
        store.add(second).unwrap();

        let senders = store.get_senders();
        assert_eq!(senders[0].article_count, 2);
    }

    #[test]
    fn test_newsletter_store_query_by_sender() {
        let dir = tempdir().unwrap();
        let mut store = NewsletterStore::new(dir.path().to_path_buf()).unwrap();

        store.add(create_test_newsletter()).unwrap();

        let results = store.query(NewsletterQuery {
            sender_email: Some("sender@example.com".to_string()),
            is_read: None,
            is_starred: None,
            labels: None,
            search: None,
            limit: None,
            offset: None,
        });

        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_newsletter_store_query_by_read_status() {
        let dir = tempdir().unwrap();
        let mut store = NewsletterStore::new(dir.path().to_path_buf()).unwrap();

        store.add(create_test_newsletter()).unwrap();

        let unread = store.query(NewsletterQuery {
            sender_email: None,
            is_read: Some(false),
            is_starred: None,
            labels: None,
            search: None,
            limit: None,
            offset: None,
        });
        assert_eq!(unread.len(), 1);

        let read = store.query(NewsletterQuery {
            sender_email: None,
            is_read: Some(true),
            is_starred: None,
            labels: None,
            search: None,
            limit: None,
            offset: None,
        });
        assert_eq!(read.len(), 0);
    }

    #[test]
    fn test_newsletter_store_query_search() {
        let dir = tempdir().unwrap();
        let mut store = NewsletterStore::new(dir.path().to_path_buf()).unwrap();

        store.add(create_test_newsletter()).unwrap();

        let by_subject = store.query(NewsletterQuery {
            sender_email: None,
            is_read: None,
            is_starred: None,
            labels: None,
            search: Some("Newsletter".to_string()),
            limit: None,
            offset: None,
        });
        assert_eq!(by_subject.len(), 1);

        let by_content = store.query(NewsletterQuery {
            sender_email: None,
            is_read: None,
            is_starred: None,
            labels: None,
            search: Some("content".to_string()),
            limit: None,
            offset: None,
        });
        assert_eq!(by_content.len(), 1);
    }

    #[test]
    fn test_newsletter_store_mark_read() {
        let dir = tempdir().unwrap();
        let mut store = NewsletterStore::new(dir.path().to_path_buf()).unwrap();

        store.add(create_test_newsletter()).unwrap();
        assert!(!store.get("test-123").unwrap().is_read);

        store.mark_read("test-123").unwrap();
        assert!(store.get("test-123").unwrap().is_read);
    }

    #[test]
    fn test_newsletter_store_toggle_star() {
        let dir = tempdir().unwrap();
        let mut store = NewsletterStore::new(dir.path().to_path_buf()).unwrap();

        store.add(create_test_newsletter()).unwrap();
        assert!(!store.get("test-123").unwrap().is_starred);

        let starred = store.toggle_star("test-123").unwrap();
        assert!(starred);
        assert!(store.get("test-123").unwrap().is_starred);

        let unstarred = store.toggle_star("test-123").unwrap();
        assert!(!unstarred);
        assert!(!store.get("test-123").unwrap().is_starred);
    }

    #[test]
    fn test_newsletter_store_delete() {
        let dir = tempdir().unwrap();
        let mut store = NewsletterStore::new(dir.path().to_path_buf()).unwrap();

        store.add(create_test_newsletter()).unwrap();
        assert!(store.get("test-123").is_some());

        store.delete("test-123").unwrap();
        assert!(store.get("test-123").is_none());
    }

    #[test]
    fn test_newsletter_store_delete_not_found() {
        let dir = tempdir().unwrap();
        let mut store = NewsletterStore::new(dir.path().to_path_buf()).unwrap();

        let result = store.delete("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_newsletter_to_markdown() {
        let dir = tempdir().unwrap();
        let mut store = NewsletterStore::new(dir.path().to_path_buf()).unwrap();

        store.add(create_test_newsletter()).unwrap();

        let md = store.to_markdown("test-123", true).unwrap();

        assert!(md.contains("# Test Newsletter"));
        assert!(md.contains("**From**:"));
        assert!(md.contains("sender@example.com"));
        assert!(md.contains("This is the newsletter content."));
    }

    #[test]
    fn test_newsletter_to_markdown_without_metadata() {
        let dir = tempdir().unwrap();
        let mut store = NewsletterStore::new(dir.path().to_path_buf()).unwrap();

        store.add(create_test_newsletter()).unwrap();

        let md = store.to_markdown("test-123", false).unwrap();

        assert!(md.contains("# Test Newsletter"));
        assert!(!md.contains("**From**:"));
        assert!(md.contains("This is the newsletter content."));
    }

    #[test]
    fn test_subscription_rules() {
        let dir = tempdir().unwrap();
        let mut store = NewsletterStore::new(dir.path().to_path_buf()).unwrap();

        store
            .add_rule(SubscriptionRule {
                id: "rule-1".to_string(),
                sender_pattern: "@substack.com".to_string(),
                auto_save: true,
                auto_labels: vec!["newsletter".to_string()],
                save_folder: Some("Newsletters".to_string()),
            })
            .unwrap();

        let rules = store.get_rules();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].sender_pattern, "@substack.com");
    }

    #[test]
    fn test_newsletter_persistence() {
        let dir = tempdir().unwrap();
        let path = dir.path().to_path_buf();

        {
            let mut store = NewsletterStore::new(path.clone()).unwrap();
            store.add(create_test_newsletter()).unwrap();
        }

        {
            let store = NewsletterStore::new(path).unwrap();
            assert!(store.get("test-123").is_some());
        }
    }

    #[test]
    fn test_imap_config_serialization() {
        let config = ImapConfig {
            host: "imap.example.com".to_string(),
            port: 993,
            username: "user@example.com".to_string(),
            password: "secret".to_string(),
            use_tls: true,
            folder: Some("INBOX".to_string()),
        };

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: ImapConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.host, "imap.example.com");
        assert_eq!(deserialized.port, 993);
        assert!(deserialized.use_tls);
    }
}
