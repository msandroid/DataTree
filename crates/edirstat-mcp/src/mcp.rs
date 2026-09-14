use std::sync::Arc;

use parking_lot::Mutex;
use rmcp::{
    ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    schemars, tool, tool_handler, tool_router,
    transport::stdio,
};
use serde::Deserialize;

use crate::error::ApiError;
use crate::model::{
    CONFIRM_PERMANENT, CONFIRM_TRASH, DeleteModeArg, PERMANENT_DELETE_ENV, SortKey, json_error,
    json_ok,
};
use crate::session::Session;

#[derive(Clone)]
pub struct DataTreeMcp {
    #[allow(dead_code)]
    tool_router: ToolRouter<Self>,
    session: Arc<Mutex<Session>>,
}

impl DataTreeMcp {
    #[must_use]
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
            session: Session::shared(),
        }
    }
}

impl Default for DataTreeMcp {
    fn default() -> Self {
        Self::new()
    }
}

fn ok_or_err<T: serde::Serialize>(result: Result<T, ApiError>) -> String {
    match result {
        Ok(value) => json_ok(&value).unwrap_or_else(|e| json_error(&e)),
        Err(err) => json_error(&err),
    }
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ScanParams {
    path: String,
    #[serde(default)]
    same_filesystem: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct PathParams {
    path: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct SaveSnapshotParams {
    path: String,
    #[serde(default)]
    no_compression: bool,
}

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
struct ChildrenParams {
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    offset: u32,
    #[serde(default)]
    limit: Option<u32>,
    #[serde(default)]
    sort: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct SearchParams {
    query: String,
    #[serde(default)]
    offset: u32,
    #[serde(default)]
    limit: Option<u32>,
    #[serde(default)]
    files_only: bool,
    #[serde(default)]
    sort: Option<String>,
}

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
struct TopParams {
    #[serde(default)]
    n: Option<u32>,
    #[serde(default)]
    files: bool,
    #[serde(default)]
    dirs: bool,
    #[serde(default)]
    sort: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ExportCsvParams {
    path: String,
    #[serde(default)]
    files_only: bool,
}

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
struct OpenGuiParams {
    #[serde(default)]
    path: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct PlanDeleteParams {
    paths: Vec<String>,
    /// `trash` (default) or `permanent` (requires DATATREE_ALLOW_PERMANENT_DELETE=1)
    #[serde(default)]
    mode: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ConfirmDeleteParams {
    plan_id: String,
    confirm_token: String,
    /// Exactly `TRASH` or `DELETE PERMANENTLY`
    confirm: String,
}

#[tool_router]
impl DataTreeMcp {
    #[tool(description = "List mounted disks/volumes with size and filesystem type.")]
    fn list_volumes(&self) -> String {
        ok_or_err(Ok(Session::list_volumes()))
    }

    #[tool(
        description = "Scan a directory. Returns a summary only (engine, files, dirs, size, allocated). Does not dump the tree."
    )]
    fn scan(&self, Parameters(params): Parameters<ScanParams>) -> String {
        let mut session = self.session.lock();
        ok_or_err(session.scan(std::path::Path::new(&params.path), params.same_filesystem))
    }

    #[tool(description = "Return live scan progress (engine, files, dirs, bytes).")]
    fn scan_status(&self) -> String {
        let session = self.session.lock();
        ok_or_err(Ok(session.scan_status()))
    }

    #[tool(description = "Request cancellation of an in-progress scan.")]
    fn cancel_scan(&self) -> String {
        self.session.lock().cancel_scan();
        "{\"cancelled\":true}".to_owned()
    }

    #[tool(description = "Load a DataTree/eDirStat snapshot file (.edst or .edst.zst).")]
    fn load_snapshot(&self, Parameters(params): Parameters<PathParams>) -> String {
        let mut session = self.session.lock();
        ok_or_err(session.load_snapshot_file(std::path::Path::new(&params.path)))
    }

    #[tool(description = "Save the current snapshot to an .edst / .edst.zst file.")]
    fn save_snapshot(&self, Parameters(params): Parameters<SaveSnapshotParams>) -> String {
        let session = self.session.lock();
        match session.save_snapshot_file(std::path::Path::new(&params.path), !params.no_compression)
        {
            Ok(()) => format!("{{\"saved\":\"{}\"}}", params.path.replace('\\', "\\\\")),
            Err(err) => json_error(&err),
        }
    }

    #[tool(description = "Summary of the current snapshot: path, engine, totals.")]
    fn summary(&self) -> String {
        let session = self.session.lock();
        ok_or_err(session.summary())
    }

    #[tool(description = "Details for one path in the current snapshot.")]
    fn path_info(&self, Parameters(params): Parameters<PathParams>) -> String {
        let session = self.session.lock();
        ok_or_err(session.path_info(&params.path))
    }

    #[tool(description = "List immediate children of a path. Paginated. sort=allocated|size|name.")]
    fn children(&self, Parameters(params): Parameters<ChildrenParams>) -> String {
        let session = self.session.lock();
        ok_or_err(session.children(
            params.path.as_deref(),
            params.offset,
            params.limit,
            SortKey::parse(params.sort.as_deref()),
        ))
    }

    #[tool(
        description = "Search the snapshot with DataTree filter syntax: name, *.iso, <100m, a>=1g. Paginated."
    )]
    fn search(&self, Parameters(params): Parameters<SearchParams>) -> String {
        let session = self.session.lock();
        ok_or_err(session.search(
            &params.query,
            params.offset,
            params.limit,
            params.files_only,
            SortKey::parse(params.sort.as_deref()),
        ))
    }

    #[tool(description = "Largest files and/or directories in the snapshot (never the full tree).")]
    fn top(&self, Parameters(params): Parameters<TopParams>) -> String {
        let session = self.session.lock();
        ok_or_err(session.top(
            params.n,
            params.files,
            params.dirs,
            SortKey::parse(params.sort.as_deref()),
        ))
    }

    #[tool(description = "Extension statistics (extension, total size, file count).")]
    fn extensions(&self) -> String {
        let session = self.session.lock();
        ok_or_err(Ok(session.extensions()))
    }

    #[tool(description = "Export the current snapshot as CSV (WizTree-like columns).")]
    fn export_csv(&self, Parameters(params): Parameters<ExportCsvParams>) -> String {
        let session = self.session.lock();
        match session.export_csv(std::path::Path::new(&params.path), params.files_only) {
            Ok(()) => format!("{{\"exported\":\"{}\"}}", params.path.replace('\\', "\\\\")),
            Err(err) => json_error(&err),
        }
    }

    #[tool(description = "Open the DataTree GUI on a path or the current snapshot (human bridge).")]
    fn open_gui(&self, Parameters(params): Parameters<OpenGuiParams>) -> String {
        let session = self.session.lock();
        match session.open_gui(params.path.as_deref().map(std::path::Path::new)) {
            Ok(msg) => serde_json::json!({"ok": true, "message": msg}).to_string(),
            Err(err) => json_error(&err),
        }
    }

    #[tool(
        description = "Prepare a delete plan. mode=trash (default) or permanent. Does not delete. Show the item list to the user, then call confirm_delete."
    )]
    fn plan_delete(&self, Parameters(params): Parameters<PlanDeleteParams>) -> String {
        let mode = params
            .mode
            .as_deref()
            .and_then(DeleteModeArg::parse)
            .unwrap_or(DeleteModeArg::Trash);
        let mut session = self.session.lock();
        ok_or_err(session.plan_delete(&params.paths, mode))
    }

    #[tool(
        description = "Execute a previously returned delete plan. confirm must be TRASH or DELETE PERMANENTLY. Permanent delete also requires DATATREE_ALLOW_PERMANENT_DELETE=1."
    )]
    fn confirm_delete(&self, Parameters(params): Parameters<ConfirmDeleteParams>) -> String {
        let _ = (CONFIRM_TRASH, CONFIRM_PERMANENT, PERMANENT_DELETE_ENV);
        let mut session = self.session.lock();
        ok_or_err(session.confirm_delete(&params.plan_id, &params.confirm_token, &params.confirm))
    }
}

#[tool_handler]
impl ServerHandler for DataTreeMcp {}

/// Speak MCP over stdin/stdout. Logs must go to stderr only.
pub async fn serve_stdio() -> anyhow::Result<()> {
    let server = DataTreeMcp::new();
    let running = server.serve(stdio()).await?;
    running.waiting().await?;
    Ok(())
}
