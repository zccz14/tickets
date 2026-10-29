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

/// A Cybion browser API response for a freshly minted key.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CreatedApiKey {
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

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MachineInfo {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub worker_id: String,
    #[serde(default)]
    pub worker_label: Option<String>,
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub intent: Option<String>,
    #[serde(default)]
    pub interval_seconds: i64,
    #[serde(default)]
    pub thread_id: String,
    #[serde(default)]
    pub thread_title: Option<String>,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub last_run_at: Option<i64>,
    #[serde(default)]
    pub last_exit: Option<i64>,
    #[serde(default)]
    pub created_at: i64,
}

/// What Cybion needs to create an Ultimate Machine.
#[derive(Clone, Debug, Serialize)]
pub struct NewMachine<'a> {
    pub name: &'a str,
    pub worker_id: &'a str,
    pub command: &'a str,
    pub intent: Option<&'a str>,
    pub interval_seconds: i64,
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

    pub async fn create_machine(
        &self,
        bearer: &str,
        machine: &NewMachine<'_>,
    ) -> Result<MachineInfo, String> {
        let body = serde_json::to_value(machine)
            .map_err(|error| format!("机器参数无法序列化: {error}"))?;
        let (status, text) = self
            .browser_call(reqwest::Method::POST, "/api/machines", bearer, Some(body))
            .await?;
        if !(200..300).contains(&status) {
            return Err(Self::browser_error(status, &text));
        }
        Self::parse(&text)
    }

    /// `Ok(None)` means the machine is gone on Cybion's side.
    pub async fn read_machine(
        &self,
        bearer: &str,
        id: &str,
    ) -> Result<Option<MachineInfo>, String> {
        let path = format!("/api/machines/{id}");
        let (status, text) = self
            .browser_call(reqwest::Method::GET, &path, bearer, None)
            .await?;
        if status == 404 {
            return Ok(None);
        }
        if !(200..300).contains(&status) {
            return Err(Self::browser_error(status, &text));
        }
        Self::parse(&text).map(Some)
    }

    pub async fn update_machine(
        &self,
        bearer: &str,
        id: &str,
        body: Value,
    ) -> Result<MachineInfo, String> {
        let path = format!("/api/machines/{id}");
        let (status, text) = self
            .browser_call(reqwest::Method::PATCH, &path, bearer, Some(body))
            .await?;
        if !(200..300).contains(&status) {
            return Err(Self::browser_error(status, &text));
        }
        Self::parse(&text)
    }

    pub async fn delete_machine(&self, bearer: &str, id: &str) -> Result<(), String> {
        let path = format!("/api/machines/{id}");
        let (status, text) = self
            .browser_call(reqwest::Method::DELETE, &path, bearer, None)
            .await?;
        if (200..300).contains(&status) || status == 404 {
            return Ok(());
        }
        Err(Self::browser_error(status, &text))
    }

    pub async fn run_machine(&self, bearer: &str, id: &str) -> Result<(), String> {
        let path = format!("/api/machines/{id}/run");
        let (status, text) = self
            .browser_call(reqwest::Method::POST, &path, bearer, None)
            .await?;
        if (200..300).contains(&status) {
            return Ok(());
        }
        Err(Self::browser_error(status, &text))
    }
}
