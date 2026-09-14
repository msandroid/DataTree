use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::Serialize;
use serde_json::{Value, json};

use crate::error::ApiError;

#[derive(Debug, Clone, Copy)]
pub enum InstallTarget {
    Cursor,
    Claude,
    Codex,
    Antigravity,
}

#[derive(Debug, Serialize)]
pub struct InstallResult {
    pub target: String,
    pub path: String,
    pub status: String,
}

fn mcp_stdio_entry() -> Value {
    json!({
        "type": "stdio",
        "command": "datatree-mcp",
        "args": ["mcp"]
    })
}

fn merge_mcp_servers(path: &Path, server_name: &str) -> Result<InstallResult, ApiError> {
    let mut root = if path.exists() {
        let text = fs::read_to_string(path)?;
        serde_json::from_str(&text).unwrap_or_else(|_| json!({}))
    } else {
        json!({})
    };
    let obj = root
        .as_object_mut()
        .ok_or_else(|| ApiError::msg("MCP config is not a JSON object"))?;
    let servers = obj.entry("mcpServers").or_insert_with(|| json!({}));
    let map = servers
        .as_object_mut()
        .ok_or_else(|| ApiError::msg("mcpServers is not an object"))?;
    let status = if map.contains_key(server_name) {
        "updated"
    } else {
        "created"
    };
    map.insert(server_name.to_owned(), mcp_stdio_entry());
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(&root)?)?;
    Ok(InstallResult {
        target: server_name.to_owned(),
        path: path.display().to_string(),
        status: status.to_owned(),
    })
}

fn home_dir() -> Result<PathBuf, ApiError> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .ok_or_else(|| ApiError::msg("HOME/USERPROFILE is not set"))
}

fn cursor_path(global: bool) -> Result<PathBuf, ApiError> {
    if global {
        Ok(home_dir()?.join(".cursor").join("mcp.json"))
    } else {
        Ok(PathBuf::from(".cursor").join("mcp.json"))
    }
}

fn claude_path(global: bool) -> Result<PathBuf, ApiError> {
    if global {
        Ok(home_dir()?.join(".claude").join("mcp.json"))
    } else {
        Ok(PathBuf::from(".mcp.json"))
    }
}

fn antigravity_path(global: bool) -> Result<PathBuf, ApiError> {
    if global {
        Ok(home_dir()?
            .join(".gemini")
            .join("config")
            .join("mcp_config.json"))
    } else {
        Ok(PathBuf::from(".agents")
            .join("plugins")
            .join("datatree")
            .join("mcp_config.json"))
    }
}

fn install_codex() -> Result<InstallResult, ApiError> {
    let path = home_dir()?.join(".codex").join("config.toml");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let existing = if path.exists() {
        fs::read_to_string(&path)?
    } else {
        String::new()
    };
    if existing.contains("[mcp_servers.datatree]") {
        return Ok(InstallResult {
            target: "codex".to_owned(),
            path: path.display().to_string(),
            status: "already_present".to_owned(),
        });
    }
    let mut next = existing;
    if !next.is_empty() && !next.ends_with('\n') {
        next.push('\n');
    }
    next.push_str("\n[mcp_servers.datatree]\ncommand = \"datatree-mcp\"\nargs = [\"mcp\"]\n");
    fs::write(&path, next)?;
    Ok(InstallResult {
        target: "codex".to_owned(),
        path: path.display().to_string(),
        status: "created".to_owned(),
    })
}

pub fn install(targets: &[InstallTarget], global: bool) -> Result<Vec<InstallResult>, ApiError> {
    let mut results = Vec::new();
    for target in targets {
        let result = match target {
            InstallTarget::Cursor => {
                let mut r = merge_mcp_servers(&cursor_path(global)?, "datatree")?;
                r.target = "cursor".to_owned();
                r
            }
            InstallTarget::Claude => {
                let mut r = merge_mcp_servers(&claude_path(global)?, "datatree")?;
                r.target = "claude".to_owned();
                r
            }
            InstallTarget::Codex => install_codex()?,
            InstallTarget::Antigravity => {
                let mut r = merge_mcp_servers(&antigravity_path(global)?, "datatree")?;
                r.target = "antigravity".to_owned();
                r
            }
        };
        results.push(result);
    }
    Ok(results)
}
