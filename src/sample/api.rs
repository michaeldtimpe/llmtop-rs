use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct ApiModel {
    pub source: String,
    pub model_id: String,
    pub size_bytes: Option<u64>,
    pub resident_bytes: Option<u64>,
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(2)))
        .build()
        .new_agent()
}

// ── Ollama ──────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct OllamaPs {
    models: Option<Vec<OllamaPsModel>>,
}

#[derive(Deserialize)]
struct OllamaPsModel {
    name: String,
    size: u64,
    size_vram: u64,
}

pub fn query_ollama(port: u16) -> Vec<ApiModel> {
    let url = format!("http://127.0.0.1:{port}/api/ps");
    let mut resp = match agent().get(&url).call() {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    let ps: OllamaPs = match resp.body_mut().read_json() {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    ps.models
        .unwrap_or_default()
        .into_iter()
        .map(|m| ApiModel {
            source: "ollama".into(),
            model_id: m.name,
            size_bytes: Some(m.size),
            resident_bytes: Some(m.size_vram),
        })
        .collect()
}

// ── omlx ────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct OmlxStatus {
    models: Vec<OmlxModel>,
}

#[derive(Deserialize)]
struct OmlxModel {
    id: String,
    loaded: bool,
    estimated_size: Option<u64>,
}

pub fn query_omlx(port: u16, api_key: Option<&str>) -> Vec<ApiModel> {
    let url = format!("http://127.0.0.1:{port}/v1/models/status");
    let a = agent();
    let mut req = a.get(&url);
    if let Some(key) = api_key {
        req = req.header("Authorization", &format!("Bearer {key}"));
    }
    let mut resp = match req.call() {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    let status: OmlxStatus = match resp.body_mut().read_json() {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    status
        .models
        .into_iter()
        .filter(|m| m.loaded)
        .map(|m| ApiModel {
            source: "omlx".into(),
            model_id: m.id,
            size_bytes: m.estimated_size,
            resident_bytes: m.estimated_size,
        })
        .collect()
}

// ── LM Studio ───────────────────────────────────────────────────────────────
//
// `/api/v0/models` is LM Studio's extended endpoint (REST v0). It exposes
// `state` ("loaded"/"not-loaded") and `size` (bytes on disk for the model).
// Older or non-LM-Studio servers on the same port may only serve the
// OpenAI-compatible `/v1/models`, which returns just `id`. On 404 we fall back
// and emit entries with size = None so the UI can omit the residency column
// instead of rendering a fake 0%.

#[derive(Deserialize)]
struct LmStudioV0Models {
    data: Vec<LmStudioV0Model>,
}

#[derive(Deserialize)]
struct LmStudioV0Model {
    id: String,
    #[serde(default)]
    state: Option<String>,
    #[serde(default)]
    size: Option<u64>,
}

#[derive(Deserialize)]
struct LmStudioV1Models {
    data: Vec<LmStudioV1Model>,
}

#[derive(Deserialize)]
struct LmStudioV1Model {
    id: String,
}

pub fn query_lmstudio(port: u16) -> Vec<ApiModel> {
    let a = agent();
    let v0_url = format!("http://127.0.0.1:{port}/api/v0/models");

    match a.get(&v0_url).call() {
        Ok(mut resp) => {
            if resp.status() == 404 {
                return query_lmstudio_v1(port, &a);
            }
            let models: LmStudioV0Models = match resp.body_mut().read_json() {
                Ok(v) => v,
                Err(_) => return Vec::new(),
            };
            models
                .data
                .into_iter()
                .filter(|m| m.state.as_deref() == Some("loaded"))
                .map(|m| ApiModel {
                    source: "lmstudio".into(),
                    model_id: m.id,
                    size_bytes: m.size,
                    resident_bytes: m.size,
                })
                .collect()
        }
        Err(_) => query_lmstudio_v1(port, &a),
    }
}

fn query_lmstudio_v1(port: u16, a: &ureq::Agent) -> Vec<ApiModel> {
    let url = format!("http://127.0.0.1:{port}/v1/models");
    let mut resp = match a.get(&url).call() {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    let models: LmStudioV1Models = match resp.body_mut().read_json() {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    models
        .data
        .into_iter()
        .map(|m| ApiModel {
            source: "lmstudio".into(),
            model_id: m.id,
            size_bytes: None,
            resident_bytes: None,
        })
        .collect()
}
