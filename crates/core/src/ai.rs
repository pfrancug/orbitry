//! Detect AI-related project signals from manifests (no network).

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

/// Known JS / Python / path signals that mark a project as AI-related.
const JS_AI_PACKAGES: &[&str] = &[
    "openai",
    "@anthropic-ai/sdk",
    "@anthropic-ai/sdk-beta",
    "ai",
    "@ai-sdk/openai",
    "@ai-sdk/anthropic",
    "@ai-sdk/google",
    "@ai-sdk/mistral",
    "langchain",
    "@langchain/core",
    "@langchain/openai",
    "@langchain/anthropic",
    "ollama",
    "transformers",
    "@xenova/transformers",
    "@huggingface/transformers",
    "@google/generative-ai",
    "@mistralai/mistralai",
    "groq-sdk",
    "together-ai",
    "cohere-ai",
    "replicate",
    "llamaindex",
    "langfuse",
];

const PYTHON_AI_PACKAGES: &[&str] = &[
    "openai",
    "anthropic",
    "langchain",
    "langchain-core",
    "langchain-openai",
    "transformers",
    "torch",
    "torchvision",
    "torchaudio",
    "diffusers",
    "accelerate",
    "sentence-transformers",
    "ollama",
    "llama-cpp-python",
    "llama_cpp",
    "comfyui",
    "safetensors",
    "huggingface-hub",
    "gradio",
    "vllm",
];

/// Detect AI dependency / tooling names under a project directory.
pub fn detect_ai_dependencies(dir: &Path) -> Vec<String> {
    let mut found = BTreeSet::new();
    collect_js_ai(dir, &mut found);
    collect_python_ai(dir, &mut found);
    collect_path_hints(dir, &mut found);
    found.into_iter().collect()
}

fn collect_js_ai(dir: &Path, out: &mut BTreeSet<String>) {
    let Ok(data) = fs::read(dir.join("package.json")) else {
        return;
    };
    let data = crate::config::strip_bom(&data);
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(data) else {
        return;
    };
    for key in ["dependencies", "devDependencies", "peerDependencies", "optionalDependencies"] {
        let Some(map) = value.get(key).and_then(|v| v.as_object()) else {
            continue;
        };
        for name in map.keys() {
            if is_js_ai_package(name) {
                out.insert(name.clone());
            }
        }
    }
}

fn is_js_ai_package(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    if JS_AI_PACKAGES.iter().any(|item| *item == lower) {
        return true;
    }
    lower.starts_with("@ai-sdk/")
        || lower.starts_with("@anthropic-ai/")
        || lower.starts_with("@langchain/")
        || lower.starts_with("@huggingface/")
}

fn collect_python_ai(dir: &Path, out: &mut BTreeSet<String>) {
    for file in ["requirements.txt", "requirements-dev.txt"] {
        if let Ok(text) = fs::read_to_string(dir.join(file)) {
            for line in text.lines() {
                let pkg = python_req_name(line);
                if let Some(pkg) = pkg {
                    if is_python_ai_package(&pkg) {
                        out.insert(pkg);
                    }
                }
            }
        }
    }
    if let Ok(text) = fs::read_to_string(dir.join("pyproject.toml")) {
        let lower = text.to_ascii_lowercase();
        for pkg in PYTHON_AI_PACKAGES {
            // Cheap token search in pyproject bodies.
            if lower.contains(&format!("\"{pkg}\""))
                || lower.contains(&format!("'{pkg}'"))
                || lower.contains(&format!("\n{pkg} "))
                || lower.contains(&format!("\n{pkg}="))
                || lower.contains(&format!("\n{pkg}\n"))
            {
                out.insert((*pkg).to_string());
            }
        }
    }
}

fn python_req_name(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('-') {
        return None;
    }
    let token = trimmed
        .split(['=', '>', '<', '!', ';', '[', ' ', '\t'])
        .next()?
        .trim();
    if token.is_empty() {
        return None;
    }
    Some(token.to_ascii_lowercase())
}

fn is_python_ai_package(name: &str) -> bool {
    PYTHON_AI_PACKAGES.iter().any(|item| *item == name)
}

fn collect_path_hints(dir: &Path, out: &mut BTreeSet<String>) {
    let name = dir
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if name.contains("comfyui") || name.contains("comfy-ui") {
        out.insert("comfyui".into());
    }
    // Common ComfyUI / Stable Diffusion layout markers.
    for marker in ["models/checkpoints", "models/loras", "custom_nodes", "ComfyUI"] {
        if dir.join(marker).is_dir() {
            out.insert("comfyui".into());
            break;
        }
    }
}
