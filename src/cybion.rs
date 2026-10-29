use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// A thin client for the Cybion integration API (`/v1/threads`).
///
/// Every call carries the end user's own API key: the Thread it creates belongs
/// to that user, runs on that user's Worker, and stays readable only to them.
#[derive(Clone)]
pub struct CybionClient {
    base: String,
    http: reqwest::Client,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ThreadInfo {
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub display_status: String,
    #[serde(default)]
    pub created_by: String,
    #[serde(default)]
    pub context_tokens: Option<i64>,
}

impl CybionClient {
    pub fn new(base: impl Into<String>) -> Self {
        Self {
            base: base.into().trim_end_matches('/').to_owned(),
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("reqwest 客户端"),
        }
    }

    pub fn base(&self) -> &str {
        &self.base
    }

    pub fn thread_url(&self, thread_id: &str) -> String {
        format!("{}/#/threads/{}", self.base, thread_id)
    }

    async fn send(
        &self,
        method: reqwest::Method,
        path: &str,
        key: &str,
        body: Option<Value>,
    ) -> Result<Value, String> {
        let mut request = self
            .http
            .request(method, format!("{}{}", self.base, path))
            .bearer_auth(key)
            .header("accept", "application/json");
        if let Some(body) = body {
            request = request.json(&body);
        }
        let response = request
            .send()
            .await
            .map_err(|error| format!("无法连接 Cybion: {error}"))?;
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        if !status.is_success() {
            let detail: String = text.chars().take(300).collect();
            return Err(match status.as_u16() {
                401 => {
                    "Cybion API Key 无效或已失效，请到 Cybion 的 API Keys 页面重新生成".to_owned()
                }
                403 => "该 Cybion API Key 没有权限执行此操作".to_owned(),
                _ => format!("Cybion 返回 {status}: {detail}"),
            });
        }
        if text.trim().is_empty() {
            return Ok(Value::Null);
        }
        serde_json::from_str(&text).map_err(|error| format!("Cybion 响应不是合法 JSON: {error}"))
    }

    /// Checks that a key is accepted without creating anything: a read of a
    /// thread that cannot exist answers 401 for a bad key and 400/404 for a
    /// good one.
    pub async fn probe_key(&self, key: &str) -> Result<(), String> {
        let response = self
            .http
            .get(format!(
                "{}/v1/threads/00000000-0000-0000-0000-000000000000",
                self.base
            ))
            .bearer_auth(key)
            .header("accept", "application/json")
            .send()
            .await
            .map_err(|error| format!("无法连接 Cybion: {error}"))?;
        match response.status().as_u16() {
            401 => Err("Cybion API Key 无效或已失效，请重新生成".to_owned()),
            _ => Ok(()),
        }
    }

    pub async fn create_thread(
        &self,
        key: &str,
        title: &str,
        external_ref: &str,
    ) -> Result<ThreadInfo, String> {
        let body = json!({ "title": title, "external_ref": external_ref });
        let value = self
            .send(reqwest::Method::POST, "/v1/threads", key, Some(body))
            .await?;
        serde_json::from_value(value).map_err(|error| format!("Cybion 线程响应无法解析: {error}"))
    }

    pub async fn post_input(&self, key: &str, thread_id: &str, input: &str) -> Result<(), String> {
        let body = json!({ "input": input });
        self.send(
            reqwest::Method::POST,
            &format!("/v1/threads/{thread_id}/inputs"),
            key,
            Some(body),
        )
        .await
        .map(|_| ())
    }

    pub async fn read_thread(&self, key: &str, thread_id: &str) -> Result<ThreadInfo, String> {
        let value = self
            .send(
                reqwest::Method::GET,
                &format!("/v1/threads/{thread_id}"),
                key,
                None,
            )
            .await?;
        serde_json::from_value(value).map_err(|error| format!("Cybion 线程响应无法解析: {error}"))
    }
}
