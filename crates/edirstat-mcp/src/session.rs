use std::{
    path::{Path, PathBuf},
    sync::{Arc, atomic::Ordering},
    time::Instant,
};

use edirstat::coordinator::{Coordinator, SharedState};
use edirstat::traversal::TraversalEngine;
use edirstat_core::arena::{FileArenaSnapshot, NodeStorage, clean_unc_path, precompute_dir_counts};
use edirstat_core::snapshot::{load_snapshot, save_snapshot};
use parking_lot::Mutex;

use crate::delete::{self, DeletePlan};
use crate::error::ApiError;
use crate::model::{
    DeleteModeArg, DeletePlanView, DeleteResult, ExtensionInfo, NodeInfo, Page, ScanStatus,
    ScanSummary, SortKey, VolumeInfo, engine_name,
};
use crate::query;
use crate::volumes;

pub struct Session {
    shared: Arc<SharedState>,
    scan_path: Option<PathBuf>,
    last_elapsed_ms: f64,
    plans: std::collections::HashMap<String, DeletePlan>,
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

impl Session {
    #[must_use]
    pub fn new() -> Self {
        Self {
            shared: Arc::new(SharedState::new()),
            scan_path: None,
            last_elapsed_ms: 0.0,
            plans: std::collections::HashMap::new(),
        }
    }

    #[must_use]
    pub fn shared() -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self::new()))
    }

    pub fn list_volumes() -> Vec<VolumeInfo> {
        volumes::list_volumes()
    }

    pub fn scan(&mut self, path: &Path, same_filesystem: bool) -> Result<ScanSummary, ApiError> {
        if !path.exists() {
            return Err(ApiError::msg(format!(
                "Path does not exist: {}",
                path.display()
            )));
        }
        let path = std::fs::canonicalize(path)?;
        let is_mft = path
            .file_name()
            .and_then(|s| s.to_str())
            .is_some_and(|s| s.eq_ignore_ascii_case("$mft"));
        if !is_mft && !path.is_dir() {
            return Err(ApiError::msg(format!(
                "Path is not a directory: {}",
                path.display()
            )));
        }

        self.shared.scan_cancel.store(false, Ordering::SeqCst);
        self.shared.scan_stats.reset();
        self.plans.clear();

        let traversal = TraversalEngine::new(self.shared.scan_stats.clone());
        let (tx, rx) = crossbeam::channel::unbounded();
        let start = Instant::now();
        let handle = traversal.start_traversal(
            path.clone(),
            same_filesystem,
            self.shared.scan_cancel.clone(),
            tx,
        )?;
        let mut coordinator = Coordinator::new(rx, self.shared.clone());
        coordinator.run_coordinator_loop(&path.to_string_lossy());
        let _ = handle.join();

        self.scan_path = Some(path);
        self.last_elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
        self.summary()
    }

    pub fn cancel_scan(&self) {
        self.shared.scan_cancel.store(true, Ordering::SeqCst);
    }

    #[must_use]
    pub fn scan_status(&self) -> ScanStatus {
        let stats = &self.shared.scan_stats;
        ScanStatus {
            scanning: self.shared.is_scanning.load(Ordering::SeqCst),
            cancel_requested: self.shared.scan_cancel.load(Ordering::SeqCst),
            engine: engine_name(stats.scan_engine.load(Ordering::SeqCst)).to_owned(),
            files: stats.files_scanned.load(Ordering::SeqCst) as u64,
            dirs: stats.dirs_scanned.load(Ordering::SeqCst) as u64,
            bytes: stats.bytes_scanned.load(Ordering::SeqCst) as u64,
            path: self
                .scan_path
                .as_ref()
                .map(|p| clean_unc_path(&p.to_string_lossy()).into_owned()),
        }
    }

    pub fn load_snapshot_file(&mut self, path: &Path) -> Result<ScanSummary, ApiError> {
        let (arena, string_pool) = load_snapshot(path)?;
        let dir_counts = Arc::new(precompute_dir_counts(arena.nodes()));
        let snapshot = FileArenaSnapshot {
            nodes: Arc::new(NodeStorage::Mmapped(arena)),
            string_pool: Arc::new(string_pool),
            dir_counts,
        };
        self.shared.store_snapshot(snapshot);
        self.scan_path = Some(path.to_path_buf());
        self.last_elapsed_ms = 0.0;
        self.plans.clear();
        self.summary()
    }

    pub fn save_snapshot_file(&self, path: &Path, compress: bool) -> Result<(), ApiError> {
        let snapshot = self.shared.current_snapshot.load();
        if snapshot.nodes.is_empty() {
            return Err(ApiError::msg("No snapshot loaded."));
        }
        save_snapshot(&snapshot.nodes, &snapshot.string_pool, path, compress)?;
        Ok(())
    }

    pub fn summary(&self) -> Result<ScanSummary, ApiError> {
        let snapshot = self.shared.current_snapshot.load();
        if snapshot.nodes.is_empty() {
            return Err(ApiError::msg(
                "No snapshot loaded. Call scan or load_snapshot first.",
            ));
        }
        let root = &snapshot.nodes[0];
        let stats = &self.shared.scan_stats;
        let path = self.scan_path.as_ref().map_or_else(
            || clean_unc_path(&snapshot.get_full_path(0)).into_owned(),
            |p| clean_unc_path(&p.to_string_lossy()).into_owned(),
        );
        Ok(ScanSummary {
            path,
            engine: engine_name(stats.scan_engine.load(Ordering::SeqCst)).to_owned(),
            files: stats.files_scanned.load(Ordering::SeqCst) as u64,
            dirs: stats.dirs_scanned.load(Ordering::SeqCst) as u64,
            size: root.size,
            allocated: root.allocated,
            elapsed_ms: self.last_elapsed_ms,
            node_count: snapshot.nodes.len() as u64,
        })
    }

    pub fn path_info(&self, path: &str) -> Result<NodeInfo, ApiError> {
        let snapshot = self.shared.current_snapshot.load();
        let idx = query::resolve_node(&snapshot, path)
            .ok_or_else(|| ApiError::msg(format!("Path not found in snapshot: {path}")))?;
        query::node_info(&snapshot, idx).ok_or_else(|| ApiError::msg("Invalid node"))
    }

    pub fn children(
        &self,
        path: Option<&str>,
        offset: u32,
        limit: Option<u32>,
        sort: SortKey,
    ) -> Result<Page, ApiError> {
        let snapshot = self.shared.current_snapshot.load();
        query::children(&snapshot, path, offset, limit, sort)
    }

    pub fn search(
        &self,
        query_str: &str,
        offset: u32,
        limit: Option<u32>,
        files_only: bool,
        sort: SortKey,
    ) -> Result<Page, ApiError> {
        let snapshot = self.shared.current_snapshot.load();
        query::search(&snapshot, query_str, offset, limit, files_only, sort)
    }

    pub fn top(
        &self,
        n: Option<u32>,
        files: bool,
        dirs: bool,
        sort: SortKey,
    ) -> Result<Vec<NodeInfo>, ApiError> {
        let snapshot = self.shared.current_snapshot.load();
        query::top(&snapshot, n, files, dirs, sort)
    }

    pub fn extensions(&self) -> Vec<ExtensionInfo> {
        query::extensions(&self.shared.extension_stats.load())
    }

    pub fn export_csv(&self, path: &Path, files_only: bool) -> Result<(), ApiError> {
        let snapshot = self.shared.current_snapshot.load();
        if snapshot.nodes.is_empty() {
            return Err(ApiError::msg("No snapshot loaded."));
        }
        edirstat_core::csv::export_csv(&snapshot, path, files_only)?;
        Ok(())
    }

    pub fn open_gui(&self, path: Option<&Path>) -> Result<String, ApiError> {
        let target = if let Some(p) = path {
            p.to_path_buf()
        } else if let Some(existing) = &self.scan_path {
            existing.clone()
        } else {
            let snapshot = self.shared.current_snapshot.load();
            if snapshot.nodes.is_empty() {
                return Err(ApiError::msg("No snapshot or path to open."));
            }
            let dir = std::env::temp_dir().join("datatree-agent");
            std::fs::create_dir_all(&dir)?;
            let tmp = dir.join("current.edst.zst");
            save_snapshot(&snapshot.nodes, &snapshot.string_pool, &tmp, true)?;
            tmp
        };

        std::process::Command::new(sibling_or_path("datatree"))
            .arg(&target)
            .spawn()
            .map_err(|e| ApiError::msg(format!("Failed to launch GUI: {e}")))?;
        Ok(format!(
            "Launched {} {}",
            sibling_or_path("datatree").display(),
            target.display()
        ))
    }

    pub fn plan_delete(
        &mut self,
        paths: &[String],
        mode: DeleteModeArg,
    ) -> Result<DeletePlanView, ApiError> {
        delete::purge_expired(&mut self.plans);
        let snapshot = self.shared.current_snapshot.load();
        let (plan, view) = delete::plan_delete(&snapshot, paths, mode)?;
        self.plans.insert(plan.plan_id.clone(), plan);
        Ok(view)
    }

    pub fn confirm_delete(
        &mut self,
        plan_id: &str,
        token: &str,
        confirm: &str,
    ) -> Result<DeleteResult, ApiError> {
        delete::purge_expired(&mut self.plans);
        let plan = self
            .plans
            .remove(plan_id)
            .ok_or_else(|| ApiError::msg("Unknown or expired plan_id"))?;
        let snapshot = self.shared.current_snapshot.load();
        let (result, new_snapshot) = delete::confirm_delete(&snapshot, plan, token, confirm)?;
        if let Some(snap) = new_snapshot {
            self.shared.store_snapshot(snap);
        }
        Ok(result)
    }
}

#[must_use]
pub fn sibling_or_path(stem: &str) -> PathBuf {
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        let with_exe = dir.join(format!("{stem}.exe"));
        if with_exe.exists() {
            return with_exe;
        }
        let plain = dir.join(stem);
        if plain.exists() {
            return plain;
        }
    }
    PathBuf::from(stem)
}
