//! Swift / Apple project detection (SPM, Xcode, XcodeGen).

use std::fs;
use std::path::{Path, PathBuf};

/// True when this directory looks like a Swift/Apple project.
pub fn is_swift_project(dir: &Path) -> bool {
    dir.join("Package.swift").is_file()
        || dir_has_xcode_bundle(dir)
        || is_xcodegen_spec(&dir.join("project.yml"))
}

/// Prefer SPM / XcodeGen / Xcode labels for the framework field.
pub fn swift_framework(dir: &Path) -> &'static str {
    if dir.join("Package.swift").is_file() {
        "swift"
    } else if is_xcodegen_spec(&dir.join("project.yml")) {
        "xcodegen"
    } else {
        "xcode"
    }
}

/// Cheap name from Package.swift `name: "…"`.
pub fn read_package_swift_name(dir: &Path) -> Option<String> {
    let text = fs::read_to_string(dir.join("Package.swift")).ok()?;
    for line in text.lines().take(80) {
        let trimmed = line.trim();
        let Some(rest) = trimmed
            .strip_prefix("name:")
            .or_else(|| trimmed.strip_prefix("name :"))
        else {
            continue;
        };
        let rest = rest.trim().trim_matches(',').trim();
        if let Some(name) = quoted_string(rest) {
            if !name.is_empty() {
                return Some(name);
            }
        }
    }
    None
}

/// Dependency / URL tokens from Package.swift and XcodeGen `packages:`.
pub fn read_swift_dep_tokens(dir: &Path) -> Vec<String> {
    let mut tokens = Vec::new();
    if let Ok(text) = fs::read_to_string(dir.join("Package.swift")) {
        collect_swift_package_tokens(&text, &mut tokens);
    }
    if is_xcodegen_spec(&dir.join("project.yml")) {
        if let Ok(text) = fs::read_to_string(dir.join("project.yml")) {
            collect_xcodegen_package_tokens(&text, &mut tokens);
        }
    }
    tokens.sort();
    tokens.dedup();
    tokens
}

fn dir_has_xcode_bundle(dir: &Path) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        let lower = name.to_ascii_lowercase();
        if lower.ends_with(".xcodeproj") || lower.ends_with(".xcworkspace") {
            return true;
        }
    }
    false
}

/// XcodeGen specs usually declare both a top-level `name:` and `targets:`.
pub fn is_xcodegen_spec(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    let Ok(text) = fs::read_to_string(path) else {
        return false;
    };
    let mut has_name = false;
    let mut has_targets = false;
    for line in text.lines().take(120) {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }
        // Only treat document-level keys (no leading indent) as the project header.
        if !line.starts_with(' ') && !line.starts_with('\t') {
            if trimmed.starts_with("name:") || trimmed.starts_with("name :") {
                has_name = true;
            }
            if trimmed.starts_with("targets:") || trimmed.starts_with("targets :") {
                has_targets = true;
            }
        }
        if has_name && has_targets {
            return true;
        }
    }
    false
}

fn collect_swift_package_tokens(text: &str, out: &mut Vec<String>) {
    for line in text.lines() {
        let lower = line.to_ascii_lowercase();
        if lower.contains(".package(") || lower.contains("url:") || lower.contains("path:") {
            push_url_or_product_tokens(line, out);
        }
        // Product names: .product(name: "GRDB", …) or dependencies: ["SQLite"]
        if lower.contains(".product(") || lower.contains("dependencies:") {
            push_quoted_idents(line, out);
        }
    }
}

fn collect_xcodegen_package_tokens(text: &str, out: &mut Vec<String>) {
    let mut in_packages = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if !line.starts_with(' ') && !line.starts_with('\t') {
            in_packages = trimmed.starts_with("packages:") || trimmed.starts_with("packages :");
            continue;
        }
        if !in_packages {
            continue;
        }
        if trimmed.starts_with("url:") || trimmed.starts_with("url :") || trimmed.contains("github.com")
        {
            push_url_or_product_tokens(line, out);
        }
        // Package keys under packages: (e.g. `GRDB:`)
        if !line.starts_with("    ") && !line.starts_with('\t') {
            if let Some(key) = trimmed.strip_suffix(':') {
                let key = key.trim();
                if !key.is_empty()
                    && key
                        .chars()
                        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
                {
                    out.push(key.to_string());
                }
            }
        }
        push_quoted_idents(line, out);
    }
}

fn push_url_or_product_tokens(line: &str, out: &mut Vec<String>) {
    for part in line.split(|ch: char| ch == '"' || ch == '\'') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if part.contains("://") || part.contains("github.com") || part.ends_with(".git") {
            if let Some(name) = repo_name_from_url(part) {
                out.push(name);
            }
            out.push(part.to_string());
        }
    }
    push_quoted_idents(line, out);
}

fn push_quoted_idents(line: &str, out: &mut Vec<String>) {
    let mut rest = line;
    while let Some(idx) = rest.find('"') {
        rest = &rest[idx + 1..];
        let Some(end) = rest.find('"') else {
            break;
        };
        let value = &rest[..end];
        rest = &rest[end + 1..];
        if value.len() >= 2
            && value.len() <= 64
            && !value.contains('/')
            && !value.contains(' ')
            && value
                .chars()
                .next()
                .is_some_and(|ch| ch.is_ascii_alphabetic())
        {
            out.push(value.to_string());
        }
    }
}

fn repo_name_from_url(url: &str) -> Option<String> {
    let trimmed = url.trim_end_matches('/').trim_end_matches(".git");
    let name = trimmed.rsplit('/').next()?.trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

fn quoted_string(value: &str) -> Option<String> {
    let value = value.trim();
    if let Some(inner) = value.strip_prefix('"').and_then(|v| v.strip_suffix('"')) {
        return Some(inner.to_string());
    }
    if let Some(inner) = value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')) {
        return Some(inner.to_string());
    }
    None
}

/// Walk a few levels for Core Data model bundles.
pub fn find_xcdatamodeld(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    walk_xcdatamodeld(dir, dir, 0, &mut out);
    out
}

fn walk_xcdatamodeld(root: &Path, dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if out.len() >= 12 || depth > 3 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    entries.sort();
    for path in entries {
        if out.len() >= 12 {
            break;
        }
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if name.starts_with('.') {
            continue;
        }
        let lower = name.to_ascii_lowercase();
        if lower.ends_with(".xcdatamodeld") && path.is_dir() {
            out.push(path);
            continue;
        }
        if path.is_dir() {
            if matches!(
                lower.as_str(),
                "node_modules"
                    | "dist"
                    | "build"
                    | "target"
                    | ".git"
                    | "deriveddata"
                    | "pods"
                    | "carthage"
            ) {
                continue;
            }
            walk_xcdatamodeld(root, &path, depth + 1, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn xcodegen_requires_name_and_targets() {
        let dir = std::env::temp_dir().join(format!("orbitry-xcodegen-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("project.yml");
        let mut file = fs::File::create(&path).unwrap();
        writeln!(file, "name: DemoApp\noptions:\n  bundleIdPrefix: com.example\ntargets:\n  App:\n    type: application").unwrap();
        assert!(is_xcodegen_spec(&path));
        let other = dir.join("random.yml");
        fs::write(&other, "services:\n  web:\n    image: nginx\n").unwrap();
        assert!(!is_xcodegen_spec(&other));
        let _ = fs::remove_dir_all(&dir);
    }
}
