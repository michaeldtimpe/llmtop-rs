use serde::Serialize;
use std::path::Path;

use super::api::{self, ApiModel};
use super::ffi;
use super::process::MatchedProcess;

const MODEL_EXTENSIONS: &[&str] = &["gguf", "ggml", "safetensors", "bin"];
const MIN_MODEL_BYTES: u64 = 50 * 1024 * 1024;

// Filenames that happen to end in a model extension but are clearly not models.
const NON_MODEL_STEMS: &[&str] = &[
    "readme", "license", "notice", "changelog", "tokenizer", "config",
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelSource {
    Ollama,
    Omlx,
    LmStudio,
    Files,
    Cmdline,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelEntry {
    pub source: ModelSource,
    pub model_id: String,
    pub size_bytes: Option<u64>,
    pub resident_bytes: Option<u64>,
    pub process_name: Option<String>,
    pub pid: Option<i32>,
}

pub struct ModelDetector {
    pub ollama_port: u16,
    pub omlx_port: Option<u16>,
    pub omlx_api_key: Option<String>,
    pub lmstudio_port: u16,
    page_size: u64,
}

impl ModelDetector {
    pub fn new(
        ollama_port: u16,
        omlx_port: Option<u16>,
        omlx_api_key: Option<String>,
        lmstudio_port: u16,
    ) -> Self {
        let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) as u64 };
        Self {
            ollama_port,
            omlx_port,
            omlx_api_key,
            lmstudio_port,
            page_size,
        }
    }

    pub fn detect(&self, procs: &[MatchedProcess]) -> Vec<ModelEntry> {
        let mut models = Vec::new();

        // API sources
        for m in api::query_ollama(self.ollama_port) {
            push_unique(&mut models, from_api(m, ModelSource::Ollama));
        }
        let omlx_key = self.omlx_api_key.as_deref();
        if let Some(port) = self.omlx_port {
            for m in api::query_omlx(port, omlx_key) {
                push_unique(&mut models, from_api(m, ModelSource::Omlx));
            }
        } else {
            for port in &[8000u16, 5741] {
                let found = api::query_omlx(*port, omlx_key);
                if !found.is_empty() {
                    for m in found {
                        push_unique(&mut models, from_api(m, ModelSource::Omlx));
                    }
                    break;
                }
            }
        }
        for m in api::query_lmstudio(self.lmstudio_port) {
            push_unique(&mut models, from_api(m, ModelSource::LmStudio));
        }

        // File-based detection via proc_pidinfo FFI
        for proc in procs {
            let files = ffi::detect_model_files(proc.pid, self.page_size, MODEL_EXTENSIONS, MIN_MODEL_BYTES);
            for f in files {
                push_unique(
                    &mut models,
                    ModelEntry {
                        source: ModelSource::Files,
                        model_id: model_id_from_path(&f.path),
                        size_bytes: Some(f.size_bytes),
                        resident_bytes: Some(f.resident_bytes),
                        process_name: Some(proc.name.clone()),
                        pid: Some(proc.pid),
                    },
                );
            }
        }

        // Cmdline-based detection
        for proc in procs {
            for arg in &proc.cmd {
                if looks_like_model_path(arg) {
                    push_unique(
                        &mut models,
                        ModelEntry {
                            source: ModelSource::Cmdline,
                            model_id: model_id_from_path(arg),
                            size_bytes: None,
                            resident_bytes: None,
                            process_name: Some(proc.name.clone()),
                            pid: Some(proc.pid),
                        },
                    );
                }
            }
        }

        models
    }
}

fn from_api(m: ApiModel, source: ModelSource) -> ModelEntry {
    ModelEntry {
        source,
        model_id: m.model_id,
        size_bytes: m.size_bytes,
        resident_bytes: m.resident_bytes,
        process_name: None,
        pid: None,
    }
}

fn push_unique(models: &mut Vec<ModelEntry>, entry: ModelEntry) {
    if !models.iter().any(|m| same_model(m, &entry)) {
        models.push(entry);
    }
}

// Strict same-model check. Only collapses true duplicates so we never hide a
// legitimately distinct model behind another:
//   1. Exact model_id match (covers same-source reconnects and file-based
//      finds that produce identical parent/stem strings).
//   2. Otherwise the entries are treated as distinct.
//
// We deliberately do NOT match on file_stem alone — `/Users/x/models/qwen3.gguf`
// and `/Volumes/External/qwen3.gguf` could be different quantizations.
fn same_model(a: &ModelEntry, b: &ModelEntry) -> bool {
    a.model_id == b.model_id
}

fn model_id_from_path(path: &str) -> String {
    let p = Path::new(path);
    let stem = p.file_stem().unwrap_or(p.as_os_str());
    let parent = p.parent().and_then(|p| p.file_name());

    match parent {
        Some(dir) => format!("{}/{}", dir.to_string_lossy(), stem.to_string_lossy()),
        None => stem.to_string_lossy().to_string(),
    }
}

// Table-driven cmdline arg classifier. An arg looks like a model path if:
//   - it has an extension in MODEL_EXTENSIONS
//   - it has a filename (not a bare extension)
//   - the stem isn't a known non-model name (readme.gguf, tokenizer.bin, …)
//   - it contains a path separator (rules out bare `model.gguf` strings that
//     are usually display labels or config keys, not real paths)
fn looks_like_model_path(arg: &str) -> bool {
    if !arg.contains('/') {
        return false;
    }
    let path = Path::new(arg);
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    let ext_lower = ext.to_ascii_lowercase();
    if !MODEL_EXTENSIONS.iter().any(|e| *e == ext_lower) {
        return false;
    }
    let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
        return false;
    };
    let stem_lower = stem.to_ascii_lowercase();
    if NON_MODEL_STEMS.iter().any(|s| *s == stem_lower) {
        return false;
    }
    true
}
