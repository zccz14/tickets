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

    /// The browser API acts as the signed-in person: the token is Auth Mini's
    /// access token, not a Cybion API key. Tickets proxies these calls so the
    /// ticket page can show and drive the reporter's own Thread.
    async fn browser_send(
        &self,
        method: reqwest::Method,
        path: &str,
        token: &str,
        body: Option<Value>,
    ) -> Result<Value, String> {
        let mut request = self
            .http
            .request(method, format!("{}{}", self.base, path))
            .bearer_auth(token)
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
                401 => "登录已失效或当前登录没有 Cybion 权限，请重新登录".to_owned(),
                403 => "当前账号没有访问这个 Cybion Thread 的权限".to_owned(),
                404 => "Cybion Thread 不存在或已被删除".to_owned(),
                _ => format!("Cybion 返回 {status}: {detail}"),
            });
        }
        if text.trim().is_empty() {
            return Ok(Value::Null);
        }
        serde_json::from_str(&text).map_err(|error| format!("Cybion 响应不是合法 JSON: {error}"))
    }

    /// History records after `after`: the ticket page polls with the newest id
    /// it has seen, so a long investigation never re-transfers old evidence.
    pub async fn browser_history(
        &self,
        token: &str,
        thread_id: &str,
        after: i64,
    ) -> Result<Vec<Value>, String> {
        let value = self
            .browser_send(
                reqwest::Method::GET,
                &format!("/api/threads/{thread_id}/history?after={after}"),
                token,
                None,
            )
            .await?;
        match value {
            Value::Array(records) => Ok(records),
            _ => Err("Cybion 线程历史不是数组".to_owned()),
        }
    }

    /// The reply that is still being produced, if any. This is the cheap poll
    /// that makes the conversation feel live between durable history records.
    pub async fn browser_response(
        &self,
        token: &str,
        thread_id: &str,
    ) -> Result<Option<Value>, String> {
        let value = self
            .browser_send(
                reqwest::Method::GET,
                &format!("/api/threads/{thread_id}/response"),
                token,
                None,
            )
            .await?;
        Ok(match value {
            Value::Null => None,
            other => Some(other),
        })
    }

    pub async fn browser_input(
        &self,
        token: &str,
        thread_id: &str,
        input: &str,
    ) -> Result<(), String> {
        self.browser_send(
            reqwest::Method::POST,
            &format!("/api/threads/{thread_id}/inputs"),
            token,
            Some(json!({ "input": input })),
        )
        .await
        .map(|_| ())
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

/// A Cybion browser API response for a freshly minted key.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CreatedApiKey {
    /// Cybion flattens the key fields next to `secret`, so `{ id, label,
    /// prefix, …, secret }` arrives in one object.
    #[serde(flatten)]
    pub key: ApiKeyRef,
    pub secret: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ApiKeyRef {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub prefix: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WorkerInfo {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub owner_user_id: Option<String>,
    #[serde(default)]
    pub last_seen_at: Option<i64>,
}

impl CybionClient {
    /// Calls Cybion's browser API with the end user's Auth Mini token. These
    /// endpoints are not part of the `cyb_` integration API: they act for the
    /// signed-in person, which is exactly why the token is forwarded instead of
    /// a shared server key.
    async fn browser_call(
        &self,
        method: reqwest::Method,
        path: &str,
        bearer: &str,
        body: Option<Value>,
    ) -> Result<(u16, String), String> {
        let mut request = self
            .http
            .request(method, format!("{}{}", self.base, path))
            .bearer_auth(bearer)
            .header("accept", "application/json");
        if let Some(body) = body {
            request = request.json(&body);
        }
        let response = request
            .send()
            .await
            .map_err(|error| format!("无法连接 Cybion: {error}"))?;
        let status = response.status().as_u16();
        let text = response.text().await.unwrap_or_default();
        Ok((status, text))
    }

    fn browser_error(status: u16, text: &str) -> String {
        let detail: String = text.chars().take(300).collect();
        match status {
            401 => {
                "Cybion 不认可这次登录授权（缺少 cybion.ntnl.io 受众），请重新登录后再试".to_owned()
            }
            403 => "当前 Cybion 账号无权执行该操作".to_owned(),
            404 => "Cybion 里找不到对应的资源".to_owned(),
            _ => format!("Cybion 返回 {status}: {detail}"),
        }
    }

    fn parse<T: serde::de::DeserializeOwned>(text: &str) -> Result<T, String> {
        serde_json::from_str(text).map_err(|error| format!("Cybion 响应无法解析: {error}"))
    }

    pub async fn create_api_key(&self, bearer: &str, label: &str) -> Result<CreatedApiKey, String> {
        let body = json!({ "label": label });
        let (status, text) = self
            .browser_call(reqwest::Method::POST, "/api/api-keys", bearer, Some(body))
            .await?;
        if !(200..300).contains(&status) {
            return Err(Self::browser_error(status, &text));
        }
        Self::parse(&text)
    }

    pub async fn delete_api_key(&self, bearer: &str, id: &str) -> Result<(), String> {
        let path = format!("/api/api-keys/{id}");
        let (status, text) = self
            .browser_call(reqwest::Method::DELETE, &path, bearer, None)
            .await?;
        if (200..300).contains(&status) || status == 404 {
            return Ok(());
        }
        Err(Self::browser_error(status, &text))
    }

    pub async fn list_workers(&self, bearer: &str) -> Result<Vec<WorkerInfo>, String> {
        let (status, text) = self
            .browser_call(reqwest::Method::GET, "/api/workers", bearer, None)
            .await?;
        if !(200..300).contains(&status) {
            return Err(Self::browser_error(status, &text));
        }
        Self::parse(&text)
    }
}
