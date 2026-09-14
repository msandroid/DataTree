//! Filesystem mutation and snapshot unlink helpers shared by the GUI and MCP.

use std::sync::Arc;

use crate::arena::{FileArenaSnapshot, FileNode, NO_INDEX, NodeStorage, precompute_dir_counts};

/// How a path should be removed from disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteMode {
    Trash,
    Permanent,
}

/// Result of unlinking nodes from a snapshot without touching the filesystem.
#[derive(Debug, Clone)]
pub struct RemoveNodesResult {
    pub snapshot: FileArenaSnapshot,
    pub files_removed: usize,
    pub dirs_removed: usize,
    pub bytes_removed: u64,
}

/// Maximum number of paths a single delete plan may cover.
pub const MAX_DELETE_ITEMS: usize = 1000;

/// Returns true when `path` is a filesystem/volume root (`/` or `C:\`).
#[must_use]
pub fn is_volume_root(path: &std::path::Path) -> bool {
    let owned = path.to_string_lossy().into_owned();
    let cleaned = crate::arena::clean_unc_path(&owned);
    let s = cleaned.trim();
    if s == "/" || s == "\\" {
        return true;
    }
    let trimmed = s.trim_end_matches(['\\', '/']);
    let bytes = trimmed.as_bytes();
    bytes.len() == 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

fn count_nested_stats(nodes: &[FileNode], idx: u32, files: &mut usize, dirs: &mut usize) {
    if idx as usize >= nodes.len() {
        return;
    }
    let node = &nodes[idx as usize];
    if node.is_directory() {
        *dirs += 1;
        let mut curr = node.first_child;
        while curr != NO_INDEX {
            if curr as usize >= nodes.len() {
                break;
            }
            count_nested_stats(nodes, curr, files, dirs);
            curr = nodes[curr as usize].next_sibling;
        }
    } else {
        *files += 1;
    }
}

/// Unlink `target_indices` from the tree, rolling `size` / `allocated` / `file_count` up
/// to the root. Nodes stay in the arena but are detached (same as the GUI).
#[must_use]
pub fn remove_nodes(snapshot: &FileArenaSnapshot, target_indices: &[u32]) -> RemoveNodesResult {
    let mut cloned_nodes = snapshot.nodes.to_vec();
    let mut files_removed = 0usize;
    let mut dirs_removed = 0usize;
    let mut bytes_removed = 0u64;

    if target_indices.is_empty() {
        return RemoveNodesResult {
            snapshot: snapshot.clone(),
            files_removed,
            dirs_removed,
            bytes_removed,
        };
    }

    for &node_idx in target_indices {
        let idx = node_idx as usize;
        if idx >= cloned_nodes.len() {
            continue;
        }
        bytes_removed += cloned_nodes[idx].size;
        count_nested_stats(
            &cloned_nodes,
            node_idx,
            &mut files_removed,
            &mut dirs_removed,
        );
    }

    for &node_idx in target_indices {
        let node_idx = node_idx as usize;
        if node_idx >= cloned_nodes.len() {
            continue;
        }

        let node_size = cloned_nodes[node_idx].size;
        let node_allocated = cloned_nodes[node_idx].allocated;
        let parent_idx = cloned_nodes[node_idx].parent;
        let is_dir = cloned_nodes[node_idx].is_directory();

        if parent_idx != NO_INDEX {
            let p_idx = parent_idx as usize;
            if p_idx < cloned_nodes.len() {
                let mut prev_sibling: Option<u32> = None;
                let mut curr_sibling = cloned_nodes[p_idx].first_child;

                while curr_sibling != NO_INDEX {
                    if curr_sibling as usize >= cloned_nodes.len() {
                        break;
                    }
                    if curr_sibling == node_idx as u32 {
                        let next_sib = cloned_nodes[node_idx].next_sibling;
                        if let Some(prev) = prev_sibling {
                            cloned_nodes[prev as usize].next_sibling = next_sib;
                        } else {
                            cloned_nodes[p_idx].first_child = next_sib;
                        }
                        break;
                    }
                    prev_sibling = Some(curr_sibling);
                    curr_sibling = cloned_nodes[curr_sibling as usize].next_sibling;
                }
            }
        }

        let mut current_parent = if parent_idx == NO_INDEX {
            None
        } else {
            Some(parent_idx)
        };
        while let Some(p_idx) = current_parent {
            if p_idx as usize >= cloned_nodes.len() {
                break;
            }
            let p_node = &mut cloned_nodes[p_idx as usize];
            p_node.size = p_node.size.saturating_sub(node_size);
            p_node.allocated = p_node.allocated.saturating_sub(node_allocated);
            if !is_dir {
                p_node.file_count = p_node.file_count.saturating_sub(1);
            }
            current_parent = p_node.parent_opt();
        }

        cloned_nodes[node_idx].size = 0;
        cloned_nodes[node_idx].allocated = 0;
        cloned_nodes[node_idx].file_count = 0;
        cloned_nodes[node_idx].first_child = NO_INDEX;
        cloned_nodes[node_idx].next_sibling = NO_INDEX;
    }

    let dir_counts = Arc::new(precompute_dir_counts(&cloned_nodes));
    RemoveNodesResult {
        snapshot: FileArenaSnapshot {
            nodes: Arc::new(NodeStorage::Owned(cloned_nodes)),
            string_pool: snapshot.string_pool.clone(),
            dir_counts,
        },
        files_removed,
        dirs_removed,
        bytes_removed,
    }
}

/// Delete `path` via the OS trash or permanently. The bool is `is_permission_denied`.
#[cfg(not(target_family = "wasm"))]
pub fn delete_path(path: &std::path::Path, mode: DeleteMode) -> Result<(), (String, bool)> {
    match mode {
        DeleteMode::Trash => {
            trash::delete(path).map_err(|e| (e.to_string(), is_permission_denied_trash(&e)))
        }
        DeleteMode::Permanent => match path.symlink_metadata() {
            Ok(meta) => {
                if meta.is_dir() {
                    std::fs::remove_dir_all(path).map_err(|e| {
                        (
                            e.to_string(),
                            e.kind() == std::io::ErrorKind::PermissionDenied,
                        )
                    })
                } else {
                    std::fs::remove_file(path).map_err(|e| {
                        (
                            e.to_string(),
                            e.kind() == std::io::ErrorKind::PermissionDenied,
                        )
                    })
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err((
                e.to_string(),
                e.kind() == std::io::ErrorKind::PermissionDenied,
            )),
        },
    }
}

#[cfg(not(target_family = "wasm"))]
fn is_permission_denied_trash(err: &trash::Error) -> bool {
    match err {
        trash::Error::CouldNotAccess { .. } => true,
        #[cfg(all(
            unix,
            not(target_os = "macos"),
            not(target_os = "ios"),
            not(target_os = "android")
        ))]
        trash::Error::FileSystem { source, .. } => {
            source.kind() == std::io::ErrorKind::PermissionDenied
        }
        trash::Error::Os { description, .. } | trash::Error::Unknown { description } => {
            let desc_lower = description.to_lowercase();
            desc_lower.contains("permission")
                || desc_lower.contains("access is denied")
                || desc_lower.contains("denied")
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{is_volume_root, remove_nodes};
    use crate::arena::{FileArenaSnapshot, FileNode, NodeStorage, StringPool};
    use std::sync::Arc;

    #[test]
    fn test_volume_root_unix_style() {
        assert!(is_volume_root(std::path::Path::new("/")));
    }

    #[test]
    fn test_volume_root_windows_drive() {
        assert!(is_volume_root(std::path::Path::new(r"C:\")));
        assert!(is_volume_root(std::path::Path::new(r"C:")));
        assert!(is_volume_root(std::path::Path::new(r"\\?\C:\")));
        assert!(!is_volume_root(std::path::Path::new(r"C:\Users")));
        assert!(!is_volume_root(std::path::Path::new("/home")));
    }

    #[test]
    fn test_remove_nodes_unlinks_child() {
        let mut pool = StringPool::new();
        let root_id = pool.get_or_insert(b"root");
        let child_id = pool.get_or_insert(b"child.txt");
        let mut root = FileNode::new(root_id, None, true, false, 0, 0);
        root.first_child = 1;
        root.size = 10;
        root.allocated = 16;
        root.file_count = 1;
        let mut child = FileNode::new(child_id, Some(0), false, false, 0, 0);
        child.size = 10;
        child.allocated = 16;
        let snapshot = FileArenaSnapshot {
            nodes: Arc::new(NodeStorage::Owned(vec![root, child])),
            string_pool: Arc::new(pool),
            dir_counts: Arc::new(vec![0, 0]),
        };
        let result = remove_nodes(&snapshot, &[1]);
        assert_eq!(result.files_removed, 1);
        assert_eq!(result.snapshot.nodes[0].first_child, crate::arena::NO_INDEX);
        assert_eq!(result.snapshot.nodes[0].size, 0);
        assert_eq!(result.snapshot.nodes[0].file_count, 0);
    }
}
