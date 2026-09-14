use std::{
    collections::HashMap,
    hash::{BuildHasher, RandomState},
    io::Write as _,
    path::Path,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use edirstat_core::arena::FileArenaSnapshot;
use edirstat_core::ops::{DeleteMode, MAX_DELETE_ITEMS, is_volume_root, remove_nodes};

use crate::error::ApiError;
use crate::model::{
    CONFIRM_PERMANENT, CONFIRM_TRASH, DeleteFailure, DeleteItem, DeleteModeArg, DeletePlanView,
    DeleteResult, PERMANENT_DELETE_ENV, PLAN_TTL_SECS,
};
use crate::query;

pub struct DeletePlan {
    pub plan_id: String,
    pub confirm_token: String,
    pub mode: DeleteModeArg,
    pub created: Instant,
    pub indices: Vec<u32>,
    pub items: Vec<DeleteItem>,
}

pub fn purge_expired(plans: &mut HashMap<String, DeletePlan>) {
    let ttl = Duration::from_secs(PLAN_TTL_SECS);
    plans.retain(|_, plan| plan.created.elapsed() < ttl);
}

pub fn plan_delete(
    snapshot: &FileArenaSnapshot,
    paths: &[String],
    mode: DeleteModeArg,
) -> Result<(DeletePlan, DeletePlanView), ApiError> {
    if snapshot.nodes.is_empty() {
        return Err(ApiError::msg(
            "No snapshot loaded. Call scan or load_snapshot first.",
        ));
    }
    if paths.is_empty() {
        return Err(ApiError::msg("plan_delete requires at least one path."));
    }
    if mode == DeleteModeArg::Permanent && !permanent_delete_allowed() {
        return Err(ApiError::msg(format!(
            "Permanent delete is disabled. Set {PERMANENT_DELETE_ENV}=1 to enable it."
        )));
    }
    if paths.len() > MAX_DELETE_ITEMS {
        return Err(ApiError::msg(format!(
            "Too many paths ({}). Maximum is {MAX_DELETE_ITEMS}.",
            paths.len()
        )));
    }

    let mut items = Vec::new();
    let mut indices = Vec::new();
    let mut total_size = 0u64;
    let mut total_allocated = 0u64;

    for raw in paths {
        let path = Path::new(raw);
        if is_volume_root(path) {
            return Err(ApiError::msg(format!(
                "Refusing to delete volume root: {raw}"
            )));
        }
        let idx = query::resolve_node(snapshot, raw)
            .ok_or_else(|| ApiError::msg(format!("Path not found in snapshot: {raw}")))?;
        if idx == 0 {
            return Err(ApiError::msg(format!(
                "Refusing to delete the scan root: {raw}"
            )));
        }
        let info = query::node_info(snapshot, idx)
            .ok_or_else(|| ApiError::msg(format!("Invalid node for {raw}")))?;
        if is_volume_root(Path::new(&info.path)) {
            return Err(ApiError::msg(format!(
                "Refusing to delete volume root: {}",
                info.path
            )));
        }
        total_size += info.size;
        total_allocated += info.allocated;
        indices.push(idx);
        items.push(DeleteItem {
            path: info.path,
            kind: info.kind,
            size: info.size,
            allocated: info.allocated,
        });
    }

    let plan_id = new_id("plan");
    let confirm_token = new_id("tok");
    let confirm_phrase = match mode {
        DeleteModeArg::Trash => CONFIRM_TRASH,
        DeleteModeArg::Permanent => CONFIRM_PERMANENT,
    };
    let view = DeletePlanView {
        plan_id: plan_id.clone(),
        confirm_token: confirm_token.clone(),
        mode: mode_name(mode).to_owned(),
        expires_in_secs: PLAN_TTL_SECS,
        confirm_phrase: confirm_phrase.to_owned(),
        total_size,
        total_allocated,
        items: items.clone(),
    };
    let plan = DeletePlan {
        plan_id,
        confirm_token,
        mode,
        created: Instant::now(),
        indices,
        items,
    };
    Ok((plan, view))
}

pub fn confirm_delete(
    snapshot: &FileArenaSnapshot,
    plan: DeletePlan,
    token: &str,
    confirm: &str,
) -> Result<(DeleteResult, Option<FileArenaSnapshot>), ApiError> {
    if plan.created.elapsed() > Duration::from_secs(PLAN_TTL_SECS) {
        return Err(ApiError::msg(
            "Delete plan has expired. Call plan_delete again.",
        ));
    }
    if token != plan.confirm_token {
        return Err(ApiError::msg("confirm_token does not match the plan."));
    }
    let expected = match plan.mode {
        DeleteModeArg::Trash => CONFIRM_TRASH,
        DeleteModeArg::Permanent => CONFIRM_PERMANENT,
    };
    if confirm != expected {
        return Err(ApiError::msg(format!(
            "Confirmation phrase must be exactly \"{expected}\"."
        )));
    }
    if plan.mode == DeleteModeArg::Permanent && !permanent_delete_allowed() {
        return Err(ApiError::msg(format!(
            "Permanent delete is disabled. Set {PERMANENT_DELETE_ENV}=1 to enable it."
        )));
    }

    let fs_mode = match plan.mode {
        DeleteModeArg::Trash => DeleteMode::Trash,
        DeleteModeArg::Permanent => DeleteMode::Permanent,
    };

    let mut deleted = Vec::new();
    let mut failed = Vec::new();
    let mut ok_indices = Vec::new();

    for (idx, item) in plan.indices.iter().zip(plan.items.iter()) {
        let path = Path::new(&item.path);
        match edirstat_core::ops::delete_path(path, fs_mode) {
            Ok(()) => {
                deleted.push(item.path.clone());
                ok_indices.push(*idx);
            }
            Err((error, permission_denied)) => {
                failed.push(DeleteFailure {
                    path: item.path.clone(),
                    error,
                    permission_denied,
                });
            }
        }
    }

    write_audit(plan.mode, &deleted, &failed);

    let new_snapshot = if ok_indices.is_empty() {
        None
    } else {
        Some(remove_nodes(snapshot, &ok_indices).snapshot)
    };

    Ok((
        DeleteResult {
            mode: mode_name(plan.mode).to_owned(),
            deleted,
            failed,
        },
        new_snapshot,
    ))
}

fn mode_name(mode: DeleteModeArg) -> &'static str {
    match mode {
        DeleteModeArg::Trash => "trash",
        DeleteModeArg::Permanent => "permanent",
    }
}

#[must_use]
pub fn permanent_delete_allowed() -> bool {
    matches!(
        std::env::var(PERMANENT_DELETE_ENV).as_deref(),
        Ok("1" | "true" | "TRUE" | "yes")
    )
}

fn new_id(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let salt = RandomState::new().hash_one(nanos);
    format!("{prefix}-{:016x}{salt:016x}", nanos as u64)
}

fn write_audit(mode: DeleteModeArg, deleted: &[String], failed: &[DeleteFailure]) {
    let Some(dirs) = directories::ProjectDirs::from("", "", "DataTree") else {
        return;
    };
    let path = dirs.data_local_dir().join("mcp-audit.jsonl");
    if let Some(parent) = path.parent()
        && std::fs::create_dir_all(parent).is_err()
    {
        return;
    }
    let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    else {
        return;
    };
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let line = serde_json::json!({
        "ts": ts,
        "mode": mode_name(mode),
        "deleted": deleted,
        "failed": failed.iter().map(|f| &f.path).collect::<Vec<_>>(),
    });
    let _ = writeln!(file, "{line}");
}

#[cfg(test)]
mod tests {
    use super::plan_delete;
    use crate::model::DeleteModeArg;
    use edirstat_core::arena::{FileArenaSnapshot, FileNode, NodeStorage, StringPool};
    use std::sync::Arc;

    fn tiny_snapshot() -> FileArenaSnapshot {
        let mut pool = StringPool::new();
        let root_id = pool.get_or_insert(br"C:\scan");
        let child_id = pool.get_or_insert(b"file.txt");
        let mut root = FileNode::new(root_id, None, true, false, 0, 0);
        root.first_child = 1;
        root.size = 4;
        root.allocated = 4;
        root.file_count = 1;
        let mut child = FileNode::new(child_id, Some(0), false, false, 0, 0);
        child.size = 4;
        child.allocated = 4;
        FileArenaSnapshot {
            nodes: Arc::new(NodeStorage::Owned(vec![root, child])),
            string_pool: Arc::new(pool),
            dir_counts: Arc::new(vec![0, 0]),
        }
    }

    #[test]
    fn test_plan_delete_rejects_volume_root() {
        let snap = tiny_snapshot();
        let result = plan_delete(&snap, &["C:\\".to_owned()], DeleteModeArg::Trash);
        assert!(result.is_err());
        let err = result.err().map(|e| e.to_string()).unwrap_or_default();
        assert!(err.contains("volume root"), "{err}");
    }

    #[test]
    fn test_plan_delete_rejects_scan_root() {
        let snap = tiny_snapshot();
        let result = plan_delete(&snap, &["C:\\scan".to_owned()], DeleteModeArg::Trash);
        assert!(result.is_err());
        let err = result.err().map(|e| e.to_string()).unwrap_or_default();
        assert!(err.contains("scan root"), "{err}");
    }

    #[test]
    fn test_plan_delete_accepts_file() {
        let snap = tiny_snapshot();
        let result = plan_delete(
            &snap,
            &["C:\\scan\\file.txt".to_owned()],
            DeleteModeArg::Trash,
        );
        assert!(result.is_ok());
        if let Ok((_plan, view)) = result {
            assert_eq!(view.items.len(), 1);
            assert_eq!(view.confirm_phrase, crate::model::CONFIRM_TRASH);
        }
    }
}
