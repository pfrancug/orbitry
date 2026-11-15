//! Expand composite dev scripts (concurrently, npm-run-all, workspaces) into
//! the packages they actually start, so static port detection can follow them.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

#[derive(Debug, Clone)]
pub struct ScriptTarget {
    pub dir: PathBuf,
    pub script: Option<String>,
    pub name: Option<String>,
}

pub fn resolve_script_targets(
    dir: &Path,
    script_filter: Option<&str>,
) -> Vec<ScriptTarget> {
    let mut visited = BTreeSet::new();
    let mut out = Vec::new();
    expand(dir, script_filter, None, &mut visited, &mut out);
    if out.is_empty() {
        out.push(ScriptTarget {
            dir: dir.to_path_buf(),
            script: script_filter.map(ToOwned::to_owned),
            name: None,
        });
    }
    out
}

fn expand(
    dir: &Path,
    script: Option<&str>,
    name: Option<&str>,
    visited: &mut BTreeSet<(String, String)>,
    out: &mut Vec<ScriptTarget>,
) {
    let key = (
        dir.to_string_lossy().to_ascii_lowercase(),
        script.unwrap_or("*").to_ascii_lowercase(),
    );
    if !visited.insert(key) {
        return;
    }
    let Some(command) = script_command(dir, script) else {
        out.push(ScriptTarget {
            dir: dir.to_path_buf(),
            script: script.map(ToOwned::to_owned),
            name: name.map(ToOwned::to_owned),
        });
        return;
    };

    if let Some(children) = expand_composite(dir, &command) {
        for child in children {
            expand(
                &child.dir,
                child.script.as_deref(),
                child.name.as_deref().or(name),
                visited,
                out,
            );
        }
        return;
    }

    if let Some((pkg_dir, script_name, label)) = follow_package_runner(dir, &command) {
        expand(
            &pkg_dir,
            Some(&script_name),
            label.as_deref().or(name),
            visited,
            out,
        );
        return;
    }

    out.push(ScriptTarget {
        dir: dir.to_path_buf(),
        script: script.map(ToOwned::to_owned),
        name: name.map(ToOwned::to_owned),
    });
}

fn script_command(dir: &Path, script: Option<&str>) -> Option<String> {
    let text = fs::read_to_string(dir.join("package.json")).ok()?;
    let value: Value = serde_json::from_str(&text).ok()?;
    let scripts = value.get("scripts")?.as_object()?;
    if let Some(name) = script {
        return scripts.get(name)?.as_str().map(ToOwned::to_owned);
    }
    None
}

fn expand_composite(dir: &Path, command: &str) -> Option<Vec<ScriptTarget>> {
    let args = split_shell_args(command);
    if args.is_empty() {
        return None;
    }
    let bin = bin_name(&args[0]);
    if matches!(
        bin.as_str(),
        "concurrently" | "conc" | "npm-run-all" | "npm-run-all2" | "run-p" | "run-s"
    ) {
        return Some(expand_task_runner(dir, bin.as_str(), &args));
    }
    if bin == "turbo" {
        return expand_turbo(dir, &args);
    }
    // npm-run-all style can also appear as `npx concurrently ...`
    if matches!(bin.as_str(), "npx" | "pnpm" | "yarn" | "bunx")
        && args
            .get(1)
            .is_some_and(|next| matches!(bin_name(next).as_str(), "concurrently" | "npm-run-all" | "run-p" | "run-s"))
    {
        return Some(expand_task_runner(dir, &bin_name(&args[1]), &args[1..]));
    }
    None
}

fn expand_task_runner(dir: &Path, bin: &str, args: &[String]) -> Vec<ScriptTarget> {
    let mut names: Vec<String> = Vec::new();
    let mut commands: Vec<String> = Vec::new();
    let mut i = 1;
    while i < args.len() {
        let arg = &args[i];
        if arg == "-n" || arg == "--names" || arg == "--name" || arg == "-g" || arg == "--group" {
            if let Some(value) = args.get(i + 1) {
                names = value.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
                i += 2;
                continue;
            }
        }
        if let Some(value) = arg
            .strip_prefix("--names=")
            .or_else(|| arg.strip_prefix("-n="))
        {
            names = value.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
            i += 1;
            continue;
        }
        if arg.starts_with('-') {
            // Flags that take a value.
            if matches!(
                arg.as_str(),
                "-c" | "--prefix-colors"
                    | "-p"
                    | "--prefix"
                    | "--prefix-length"
                    | "--success"
                    | "--timings"
                    | "--kill-timeout"
                    | "--passthrough-arguments"
                    | "--npm-path"
                    | "--max-processes"
                    | "-l"
                    | "--length"
                    | "-m"
            ) && !arg.contains('=')
            {
                i += 2;
                continue;
            }
            i += 1;
            continue;
        }
        commands.push(arg.clone());
        i += 1;
    }

    let scripts = package_script_map(dir);
    if matches!(bin, "npm-run-all" | "npm-run-all2" | "run-p" | "run-s") {
        commands = expand_script_globs(&commands, &scripts);
    }

    commands
        .into_iter()
        .enumerate()
        .filter_map(|(index, command)| {
            let label = names.get(index).cloned();
            if looks_like_command(&command) {
                if let Some((pkg_dir, script, runner_label)) = follow_package_runner(dir, &command)
                {
                    return Some(ScriptTarget {
                        dir: pkg_dir,
                        script: Some(script),
                        name: label.or(runner_label),
                    });
                }
                // A raw command in this package (e.g. "vite --port 5173").
                return Some(ScriptTarget {
                    dir: dir.to_path_buf(),
                    script: None,
                    name: label,
                });
            }
            // Bare script name, possibly a glob already expanded.
            if scripts.contains_key(&command) {
                return Some(ScriptTarget {
                    dir: dir.to_path_buf(),
                    script: Some(command),
                    name: label,
                });
            }
            None
        })
        .collect()
}

fn expand_turbo(dir: &Path, args: &[String]) -> Option<Vec<ScriptTarget>> {
    let mut script = "dev";
    let mut i = 1;
    while i < args.len() {
        if args[i] == "run" {
            if let Some(next) = args.get(i + 1) {
                if !next.starts_with('-') {
                    script = next;
                }
            }
            break;
        }
        if !args[i].starts_with('-') && args[i] != "run" {
            script = &args[i];
            break;
        }
        i += 1;
    }
    let packages = workspace_packages(dir);
    if packages.is_empty() {
        return None;
    }
    let mut targets = Vec::new();
    for (name, path) in packages {
        if script_command(&path, Some(script)).is_some() {
            targets.push(ScriptTarget {
                dir: path,
                script: Some(script.to_string()),
                name: Some(short_pkg_name(&name)),
            });
        }
    }
    if targets.is_empty() {
        None
    } else {
        Some(targets)
    }
}

fn follow_package_runner(dir: &Path, command: &str) -> Option<(PathBuf, String, Option<String>)> {
    let args = split_shell_args(command);
    if args.is_empty() {
        return None;
    }
    let bin = bin_name(&args[0]);
    match bin.as_str() {
        "npm" | "pnpm" | "yarn" | "bun" => {}
        _ => return None,
    }

    let mut workspace: Option<String> = None;
    let mut script: Option<String> = None;
    let mut i = 1;
    while i < args.len() {
        let arg = &args[i];
        if matches!(arg.as_str(), "-w" | "--workspace" | "--filter" | "-F") {
            workspace = args.get(i + 1).cloned();
            i += 2;
            continue;
        }
        if let Some(value) = arg
            .strip_prefix("--workspace=")
            .or_else(|| arg.strip_prefix("-w="))
            .or_else(|| arg.strip_prefix("--filter="))
            .or_else(|| arg.strip_prefix("-F="))
        {
            workspace = Some(value.to_string());
            i += 1;
            continue;
        }
        if bin == "yarn" && arg == "workspace" {
            workspace = args.get(i + 1).cloned();
            if let Some(next) = args.get(i + 2) {
                if next == "run" {
                    script = args.get(i + 3).cloned();
                } else if !next.starts_with('-') {
                    script = Some(next.clone());
                }
            }
            break;
        }
        if arg == "run" {
            script = args.get(i + 1).filter(|s| !s.starts_with('-')).cloned();
            i += 2;
            continue;
        }
        if !arg.starts_with('-') && script.is_none() {
            script = Some(arg.clone());
        }
        i += 1;
    }

    let script = script?;
    let (pkg_dir, label) = if let Some(workspace) = workspace {
        let packages = workspace_packages(workspace_root(dir));
        let found = packages.into_iter().find(|(name, path)| {
            name == &workspace
                || short_pkg_name(name) == workspace
                || path.file_name().is_some_and(|n| n == workspace.as_str())
        })?;
        (found.1, Some(short_pkg_name(&found.0)))
    } else {
        (dir.to_path_buf(), None)
    };
    Some((pkg_dir, script, label))
}

fn looks_like_command(command: &str) -> bool {
    let first = bin_name(command.split_whitespace().next().unwrap_or(command));
    matches!(
        first.as_str(),
        "npm" | "pnpm" | "yarn" | "bun" | "npx" | "node" | "vite" | "tsx" | "next" | "nuxt"
            | "cargo" | "python" | "uv" | "poetry" | "go"
    ) || command.contains(' ')
}

fn expand_script_globs(patterns: &[String], scripts: &std::collections::BTreeMap<String, String>) -> Vec<String> {
    let mut out = Vec::new();
    for pattern in patterns {
        if pattern.contains('*') {
            let prefix = pattern.split('*').next().unwrap_or("");
            let suffix = pattern.split('*').nth(1).unwrap_or("");
            for name in scripts.keys() {
                if name.starts_with(prefix) && name.ends_with(suffix) && !out.contains(name) {
                    out.push(name.clone());
                }
            }
        } else if !out.contains(pattern) {
            out.push(pattern.clone());
        }
    }
    out
}

fn package_script_map(dir: &Path) -> std::collections::BTreeMap<String, String> {
    let mut map = std::collections::BTreeMap::new();
    let Ok(text) = fs::read_to_string(dir.join("package.json")) else {
        return map;
    };
    let Ok(value) = serde_json::from_str::<Value>(&text) else {
        return map;
    };
    if let Some(scripts) = value.get("scripts").and_then(Value::as_object) {
        for (name, command) in scripts {
            if let Some(command) = command.as_str() {
                map.insert(name.clone(), command.to_string());
            }
        }
    }
    map
}

fn workspace_root(dir: &Path) -> &Path {
    for folder in dir.ancestors() {
        if folder.join("pnpm-workspace.yaml").exists() || folder.join("pnpm-workspace.yml").exists()
        {
            return folder;
        }
        if let Ok(text) = fs::read_to_string(folder.join("package.json")) {
            if let Ok(value) = serde_json::from_str::<Value>(&text) {
                if value.get("workspaces").is_some() {
                    return folder;
                }
            }
        }
    }
    dir
}

fn workspace_packages(root: &Path) -> Vec<(String, PathBuf)> {
    let mut patterns: Vec<String> = Vec::new();
    if let Ok(text) = fs::read_to_string(root.join("package.json")) {
        if let Ok(value) = serde_json::from_str::<Value>(&text) {
            match value.get("workspaces") {
                Some(Value::Array(items)) => {
                    patterns.extend(items.iter().filter_map(|item| item.as_str().map(ToOwned::to_owned)));
                }
                Some(Value::Object(obj)) => {
                    if let Some(Value::Array(items)) = obj.get("packages") {
                        patterns.extend(items.iter().filter_map(|item| item.as_str().map(ToOwned::to_owned)));
                    }
                }
                _ => {}
            }
        }
    }
    for name in ["pnpm-workspace.yaml", "pnpm-workspace.yml"] {
        if let Ok(text) = fs::read_to_string(root.join(name)) {
            for line in text.lines() {
                let trimmed = line.trim();
                if let Some(pattern) = trimmed.strip_prefix("- ") {
                    let pattern = pattern.trim().trim_matches(['\'', '"']);
                    if !pattern.is_empty() && pattern != "packages:" {
                        patterns.push(pattern.to_string());
                    }
                }
            }
        }
    }
    let mut packages = Vec::new();
    for pattern in patterns {
        let base = if let Some(dir) = pattern.strip_suffix("/*") {
            root.join(dir)
        } else if pattern.ends_with("/**") {
            continue;
        } else {
            let path = root.join(&pattern);
            if path.join("package.json").exists() {
                if let Some(entry) = package_name_and_path(&path) {
                    packages.push(entry);
                }
            }
            continue;
        };
        if let Ok(rd) = fs::read_dir(&base) {
            for ent in rd.flatten() {
                let path = ent.path();
                if path.is_dir() {
                    if let Some(entry) = package_name_and_path(&path) {
                        packages.push(entry);
                    }
                }
            }
        }
    }
    packages
}

fn package_name_and_path(dir: &Path) -> Option<(String, PathBuf)> {
    let text = fs::read_to_string(dir.join("package.json")).ok()?;
    let value: Value = serde_json::from_str(&text).ok()?;
    let name = value
        .get("name")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| {
            dir.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default()
        });
    Some((name, dir.to_path_buf()))
}

fn short_pkg_name(name: &str) -> String {
    name.rsplit('/').next().unwrap_or(name).to_string()
}

fn bin_name(token: &str) -> String {
    Path::new(token)
        .file_stem()
        .map(|s| s.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_else(|| token.to_ascii_lowercase())
}

pub fn split_shell_args(command: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut chars = command.chars().peekable();
    let mut quote: Option<char> = None;
    while let Some(ch) = chars.next() {
        if let Some(q) = quote {
            if ch == '\\' && q == '"' {
                if let Some(next) = chars.next() {
                    current.push(next);
                }
                continue;
            }
            if ch == q {
                quote = None;
            } else {
                current.push(ch);
            }
            continue;
        }
        match ch {
            '\'' | '"' => quote = Some(ch),
            c if c.is_whitespace() => {
                if !current.is_empty() {
                    args.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }
    if !current.is_empty() {
        args.push(current);
    }
    args
}
