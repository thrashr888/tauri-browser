use anyhow::{Context, Result, bail};
use futures_util::StreamExt;
use serde_json::Value;

/// HTTP/WS client for communicating with the debug bridge plugin.
pub struct BridgeClient {
    base_url: String,
    ws_url: String,
    http: reqwest::Client,
}

impl BridgeClient {
    pub fn new(port: u16) -> Self {
        Self {
            base_url: format!("http://127.0.0.1:{port}"),
            ws_url: format!("ws://127.0.0.1:{port}"),
            http: reqwest::Client::new(),
        }
    }

    pub async fn health(&self) -> Result<Value> {
        let resp = self
            .http
            .get(format!("{}/health", self.base_url))
            .send()
            .await
            .context("connecting to debug bridge — is the app running with the plugin enabled?")?;
        Ok(resp.json().await?)
    }

    pub async fn screenshot(&self) -> Result<Vec<u8>> {
        let resp = self
            .http
            .get(format!("{}/screenshot", self.base_url))
            .send()
            .await?;
        if !resp.status().is_success() {
            bail!("screenshot failed: {}", resp.text().await?);
        }
        Ok(resp.bytes().await?.to_vec())
    }

    pub async fn snapshot(&self, interactive: bool) -> Result<Value> {
        let mut url = format!("{}/snapshot", self.base_url);
        if interactive {
            url.push_str("?interactive=true");
        }
        let resp = self.http.get(&url).send().await?;
        if !resp.status().is_success() {
            bail!("snapshot failed: {}", resp.text().await?);
        }
        Ok(resp.json().await?)
    }

    pub async fn click(&self, selector: &str) -> Result<Value> {
        let resp = self
            .http
            .post(format!("{}/click", self.base_url))
            .json(&serde_json::json!({ "selector": selector }))
            .send()
            .await?;
        if !resp.status().is_success() {
            bail!("click failed: {}", resp.text().await?);
        }
        Ok(resp.json().await?)
    }

    pub async fn fill(&self, selector: &str, text: &str) -> Result<Value> {
        let resp = self
            .http
            .post(format!("{}/fill", self.base_url))
            .json(&serde_json::json!({ "selector": selector, "text": text }))
            .send()
            .await?;
        if !resp.status().is_success() {
            bail!("fill failed: {}", resp.text().await?);
        }
        Ok(resp.json().await?)
    }

    pub async fn run_js(&self, code: &str) -> Result<Value> {
        let resp = self
            .http
            .post(format!("{}/eval", self.base_url))
            .json(&serde_json::json!({ "js": code }))
            .send()
            .await?;
        if !resp.status().is_success() {
            bail!("eval failed: {}", resp.text().await?);
        }
        Ok(resp.json().await?)
    }

    pub async fn invoke(&self, command: &str, args: &str) -> Result<Value> {
        let args: Value = serde_json::from_str(args).context("invalid JSON args")?;
        let resp = self
            .http
            .post(format!("{}/invoke", self.base_url))
            .json(&serde_json::json!({ "command": command, "args": args }))
            .send()
            .await?;
        if !resp.status().is_success() {
            bail!("invoke failed: {}", resp.text().await?);
        }
        Ok(resp.json().await?)
    }

    pub async fn state(&self) -> Result<Value> {
        let resp = self
            .http
            .get(format!("{}/state", self.base_url))
            .send()
            .await?;
        if !resp.status().is_success() {
            bail!("state failed: {}", resp.text().await?);
        }
        Ok(resp.json().await?)
    }

    pub async fn commands(&self) -> Result<Value> {
        let resp = self
            .http
            .get(format!("{}/commands", self.base_url))
            .send()
            .await?;
        if !resp.status().is_success() {
            bail!("commands failed: {}", resp.text().await?);
        }
        Ok(resp.json().await?)
    }

    pub async fn windows(&self) -> Result<Value> {
        let resp = self
            .http
            .get(format!("{}/windows", self.base_url))
            .send()
            .await?;
        if !resp.status().is_success() {
            bail!("windows failed: {}", resp.text().await?);
        }
        Ok(resp.json().await?)
    }

    pub async fn event_emit(&self, name: &str, payload: &str) -> Result<Value> {
        let payload: Value = serde_json::from_str(payload).context("invalid JSON payload")?;
        let resp = self
            .http
            .post(format!("{}/events/emit", self.base_url))
            .json(&serde_json::json!({ "event": name, "payload": payload }))
            .send()
            .await?;
        if !resp.status().is_success() {
            bail!("event emit failed: {}", resp.text().await?);
        }
        Ok(resp.json().await?)
    }

    pub async fn event_list(&self) -> Result<Value> {
        let resp = self
            .http
            .get(format!("{}/events/list", self.base_url))
            .send()
            .await?;
        if !resp.status().is_success() {
            bail!("event list failed: {}", resp.text().await?);
        }
        Ok(resp.json().await?)
    }

    pub async fn event_listen(&self, name: &str) -> Result<()> {
        let url = format!("{}/events/listen?name={name}", self.ws_url);
        let (ws, _) = tokio_tungstenite::connect_async(&url)
            .await
            .context("connecting to event stream")?;

        let (_, mut read) = ws.split();
        while let Some(msg) = read.next().await {
            match msg? {
                tokio_tungstenite::tungstenite::Message::Text(text) => {
                    println!("{text}");
                }
                tokio_tungstenite::tungstenite::Message::Close(_) => break,
                _ => {}
            }
        }
        Ok(())
    }

    pub async fn stream_console(&self) -> Result<()> {
        let url = format!("{}/console", self.ws_url);
        let (ws, _) = tokio_tungstenite::connect_async(&url)
            .await
            .context("connecting to console stream")?;

        let (_, mut read) = ws.split();
        while let Some(msg) = read.next().await {
            match msg? {
                tokio_tungstenite::tungstenite::Message::Text(text) => {
                    println!("{text}");
                }
                tokio_tungstenite::tungstenite::Message::Close(_) => break,
                _ => {}
            }
        }
        Ok(())
    }

    pub async fn stream_errors(&self) -> Result<()> {
        // Errors are a filtered subset of console output
        self.stream_console().await
    }

    pub async fn stream_logs(&self, _level: &str) -> Result<()> {
        let url = format!("{}/logs", self.ws_url);
        let (ws, _) = tokio_tungstenite::connect_async(&url)
            .await
            .context("connecting to log stream")?;

        let (_, mut read) = ws.split();
        while let Some(msg) = read.next().await {
            match msg? {
                tokio_tungstenite::tungstenite::Message::Text(text) => {
                    println!("{text}");
                }
                tokio_tungstenite::tungstenite::Message::Close(_) => break,
                _ => {}
            }
        }
        Ok(())
    }
}
