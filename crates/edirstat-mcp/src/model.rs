use serde::{Deserialize, Serialize};

pub const DEFAULT_LIMIT: u32 = 50;
pub const MAX_LIMIT: u32 = 200;
pub const DEFAULT_TOP: u32 = 20;
pub const MAX_TOP: u32 = 100;
pub const PLAN_TTL_SECS: u64 = 600;
pub const CONFIRM_TRASH: &str = "TRASH";
pub const CONFIRM_PERMANENT: &str = "DELETE PERMANENTLY";
pub const PERMANENT_DELETE_ENV: &str = "DATATREE_ALLOW_PERMANENT_DELETE";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SortKey {
    #[default]
    Allocated,
    Size,
    Name,
}

impl SortKey {
    #[must_use]
    pub fn parse(raw: Option<&str>) -> Self {
        match raw.map(str::to_ascii_lowercase).as_deref() {
            Some("size") => Self::Size,
            Some("name") => Self::Name,
            _ => Self::Allocated,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeleteModeArg {
    Trash,
    Permanent,
}

impl DeleteModeArg {
    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.to_ascii_lowercase().as_str() {
            "trash" => Some(Self::Trash),
            "permanent" => Some(Self::Permanent),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeInfo {
    pub name: String,
    pub mount_point: String,
    pub fs_type: String,
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub is_removable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanSummary {
    pub path: String,
    pub engine: String,
    pub files: u64,
    pub dirs: u64,
    pub size: u64,
    pub allocated: u64,
    pub elapsed_ms: f64,
    pub node_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanStatus {
    pub scanning: bool,
    pub cancel_requested: bool,
    pub engine: String,
    pub files: u64,
    pub dirs: u64,
    pub bytes: u64,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub path: String,
    pub name: String,
    pub kind: String,
    pub size: u64,
    pub allocated: u64,
    pub files: u32,
    pub folders: u32,
    pub modified: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page {
    pub total: u64,
    pub offset: u32,
    pub limit: u32,
    pub items: Vec<NodeInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionInfo {
    pub extension: String,
    pub size: u64,
    pub files: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteItem {
    pub path: String,
    pub kind: String,
    pub size: u64,
    pub allocated: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeletePlanView {
    pub plan_id: String,
    pub confirm_token: String,
    pub mode: String,
    pub expires_in_secs: u64,
    pub confirm_phrase: String,
    pub total_size: u64,
    pub total_allocated: u64,
    pub items: Vec<DeleteItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteResult {
    pub mode: String,
    pub deleted: Vec<String>,
    pub failed: Vec<DeleteFailure>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteFailure {
    pub path: String,
    pub error: String,
    pub permission_denied: bool,
}

#[must_use]
pub fn clamp_limit(limit: Option<u32>) -> u32 {
    limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT)
}

#[must_use]
pub fn clamp_top(n: Option<u32>) -> u32 {
    n.unwrap_or(DEFAULT_TOP).clamp(1, MAX_TOP)
}

#[must_use]
pub fn engine_name(code: u8) -> &'static str {
    match code {
        edirstat_core::state::SCAN_ENGINE_MFT => "MFT",
        edirstat_core::state::SCAN_ENGINE_WALK => "Walk",
        _ => "none",
    }
}

pub fn json_error(err: &crate::ApiError) -> String {
    serde_json::json!({ "error": err.to_string() }).to_string()
}

pub fn json_ok<T: Serialize>(value: &T) -> Result<String, crate::ApiError> {
    Ok(serde_json::to_string(value)?)
}
