use edirstat_core::SearchQuery;
use edirstat_core::arena::{FileArenaSnapshot, NO_INDEX, clean_unc_path};

use crate::error::ApiError;
use crate::model::{ExtensionInfo, NodeInfo, Page, SortKey, clamp_limit, clamp_top};

fn normalize_path(path: &str) -> String {
    clean_unc_path(path)
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_ascii_lowercase()
}

/// Resolve a user-supplied path against the current snapshot.
#[must_use]
pub fn resolve_node(snapshot: &FileArenaSnapshot, path: &str) -> Option<u32> {
    if snapshot.nodes.is_empty() {
        return None;
    }
    let root = snapshot.get_full_path(0);
    let root_n = normalize_path(&root);
    let input_n = normalize_path(path);
    if input_n == root_n || input_n.is_empty() {
        return Some(0);
    }
    let relative = input_n.strip_prefix(&root_n)?;
    let mut curr = 0u32;
    for component in relative.split('/') {
        if component.is_empty() {
            continue;
        }
        curr = find_child_ci(snapshot, curr, component)?;
    }
    Some(curr)
}

fn find_child_ci(snapshot: &FileArenaSnapshot, parent: u32, name_lower: &str) -> Option<u32> {
    let node = snapshot.nodes.get(parent as usize)?;
    let mut curr = node.first_child;
    while curr != NO_INDEX {
        let child = snapshot.nodes.get(curr as usize)?;
        if let Some(n) = snapshot.string_pool.get(child.name_id)
            && n.replace('\\', "/").to_ascii_lowercase() == name_lower
        {
            return Some(curr);
        }
        curr = child.next_sibling;
    }
    None
}

#[must_use]
pub fn node_info(snapshot: &FileArenaSnapshot, idx: u32) -> Option<NodeInfo> {
    let node = snapshot.nodes.get(idx as usize)?;
    let raw_name = snapshot.string_pool.get(node.name_id).unwrap_or("unknown");
    let name = if node.parent_opt().is_none() {
        clean_unc_path(raw_name).into_owned()
    } else {
        raw_name.to_string()
    };
    let folders = if node.is_directory() {
        *snapshot.dir_counts.get(idx as usize).unwrap_or(&0)
    } else {
        0
    };
    Some(NodeInfo {
        path: clean_unc_path(&snapshot.get_full_path(idx)).into_owned(),
        name,
        kind: if node.is_directory() {
            "dir".to_owned()
        } else {
            "file".to_owned()
        },
        size: node.size,
        allocated: node.allocated,
        files: if node.is_directory() {
            node.file_count
        } else {
            0
        },
        folders,
        modified: node.modified_timestamp,
    })
}

fn metric(info: &NodeInfo, sort: SortKey) -> u64 {
    match sort {
        SortKey::Allocated => info.allocated,
        SortKey::Size => info.size,
        SortKey::Name => 0,
    }
}

fn sort_infos(items: &mut [NodeInfo], sort: SortKey) {
    match sort {
        SortKey::Name => items.sort_by_key(|a| a.name.to_ascii_lowercase()),
        SortKey::Allocated | SortKey::Size => {
            items.sort_by(|a, b| {
                metric(b, sort)
                    .cmp(&metric(a, sort))
                    .then_with(|| a.name.cmp(&b.name))
            });
        }
    }
}

pub fn children(
    snapshot: &FileArenaSnapshot,
    path: Option<&str>,
    offset: u32,
    limit: Option<u32>,
    sort: SortKey,
) -> Result<Page, ApiError> {
    if snapshot.nodes.is_empty() {
        return Err(ApiError::msg(
            "No snapshot loaded. Call scan or load_snapshot first.",
        ));
    }
    let idx = match path {
        None | Some("") => 0,
        Some(p) => resolve_node(snapshot, p)
            .ok_or_else(|| ApiError::msg(format!("Path not found in snapshot: {p}")))?,
    };
    let node = snapshot
        .nodes
        .get(idx as usize)
        .ok_or_else(|| ApiError::msg("Invalid node"))?;
    let mut items = Vec::new();
    let mut curr = node.first_child;
    while curr != NO_INDEX {
        if let Some(info) = node_info(snapshot, curr) {
            items.push(info);
        }
        curr = snapshot
            .nodes
            .get(curr as usize)
            .map_or(NO_INDEX, |n| n.next_sibling);
    }
    sort_infos(&mut items, sort);
    Ok(paginate(items, offset, limit))
}

pub fn search(
    snapshot: &FileArenaSnapshot,
    query: &str,
    offset: u32,
    limit: Option<u32>,
    files_only: bool,
    sort: SortKey,
) -> Result<Page, ApiError> {
    if snapshot.nodes.is_empty() {
        return Err(ApiError::msg(
            "No snapshot loaded. Call scan or load_snapshot first.",
        ));
    }
    let parsed = SearchQuery::parse(query);
    let mut items = Vec::new();
    for idx in 0..snapshot.nodes.len() {
        let node = &snapshot.nodes[idx];
        if files_only && node.is_directory() {
            continue;
        }
        let raw_name = snapshot.string_pool.get(node.name_id).unwrap_or("");
        let name = if node.parent_opt().is_none() {
            clean_unc_path(raw_name)
        } else {
            std::borrow::Cow::Borrowed(raw_name)
        };
        if parsed.matches_name(&name, false)
            && parsed.matches_sizes(node.size, node.allocated)
            && let Some(info) = node_info(snapshot, idx as u32)
        {
            items.push(info);
        }
    }
    sort_infos(&mut items, sort);
    Ok(paginate(items, offset, limit))
}

pub fn top(
    snapshot: &FileArenaSnapshot,
    n: Option<u32>,
    files: bool,
    dirs: bool,
    sort: SortKey,
) -> Result<Vec<NodeInfo>, ApiError> {
    if snapshot.nodes.is_empty() {
        return Err(ApiError::msg(
            "No snapshot loaded. Call scan or load_snapshot first.",
        ));
    }
    let want_files = files || !dirs;
    let want_dirs = dirs;
    let mut items = Vec::new();
    for idx in 0..snapshot.nodes.len() {
        let node = &snapshot.nodes[idx];
        if node.is_directory() {
            if !want_dirs || idx == 0 {
                continue;
            }
        } else if !want_files {
            continue;
        }
        if let Some(info) = node_info(snapshot, idx as u32) {
            items.push(info);
        }
    }
    sort_infos(&mut items, sort);
    let take = clamp_top(n) as usize;
    items.truncate(take);
    Ok(items)
}

pub fn extensions(snapshot_stats: &[(compact_str::CompactString, u64, u32)]) -> Vec<ExtensionInfo> {
    let mut items: Vec<ExtensionInfo> = snapshot_stats
        .iter()
        .map(|(ext, size, files)| ExtensionInfo {
            extension: ext.to_string(),
            size: *size,
            files: *files,
        })
        .collect();
    items.sort_by(|a, b| {
        b.size
            .cmp(&a.size)
            .then_with(|| a.extension.cmp(&b.extension))
    });
    items
}

fn paginate(items: Vec<NodeInfo>, offset: u32, limit: Option<u32>) -> Page {
    let total = items.len() as u64;
    let limit = clamp_limit(limit);
    let start = (offset as usize).min(items.len());
    let end = (start + limit as usize).min(items.len());
    Page {
        total,
        offset,
        limit,
        items: items[start..end].to_vec(),
    }
}
