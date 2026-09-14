use std::{
    fs::File,
    io::{BufWriter, Write},
    path::Path,
};

use crate::arena::FileArenaSnapshot;

/// Writes a WizTree-like CSV of the scanned tree.
///
/// When `files_only` is true, directories are omitted (File View). Otherwise
/// every node is written (Tree View).
pub fn export_csv(
    snapshot: &FileArenaSnapshot,
    path: &Path,
    files_only: bool,
) -> Result<(), crate::EdirstatError> {
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);
    writeln!(writer, "File Name,Size,Allocated,Modified,Files,Folders")?;

    for (idx, node) in snapshot.nodes.iter().enumerate() {
        if files_only && node.is_directory() {
            continue;
        }
        let full_path_raw = snapshot.get_full_path(idx as u32);
        let full_path = crate::arena::clean_unc_path(&full_path_raw);
        let escaped = escape_csv(&full_path);
        let files = if node.is_directory() {
            node.file_count
        } else {
            0
        };
        let folders = if node.is_directory() {
            *snapshot.dir_counts.get(idx).unwrap_or(&0)
        } else {
            0
        };
        writeln!(
            writer,
            "{escaped},{},{},{},{files},{folders}",
            node.size, node.allocated, node.modified_timestamp
        )?;
    }

    writer.flush()?;
    Ok(())
}

fn escape_csv(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        let mut out = String::from("\"");
        for ch in value.chars() {
            if ch == '"' {
                out.push('"');
            }
            out.push(ch);
        }
        out.push('"');
        out
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::escape_csv;

    #[test]
    fn test_escape_csv_plain() {
        assert_eq!(escape_csv("C:\\Windows"), "C:\\Windows");
    }

    #[test]
    fn test_escape_csv_comma() {
        assert_eq!(escape_csv("a,b"), "\"a,b\"");
        assert_eq!(escape_csv("say \"hi\""), "\"say \"\"hi\"\"\"");
    }

    #[test]
    fn test_export_csv_files_only() -> Result<(), crate::EdirstatError> {
        use std::sync::Arc;

        use crate::arena::{FileArenaSnapshot, FileNode, NodeStorage, StringPool};

        let mut pool = StringPool::new();
        let r_id = pool.get_or_insert(b"C:\\");
        let f_id = pool.get_or_insert(b"a.txt");
        let mut root = FileNode::new(r_id, None, true, false, 0, 0);
        root.first_child = 1;
        root.size = 10;
        root.allocated = 4096;
        root.file_count = 1;
        let mut file = FileNode::new(f_id, Some(0), false, false, 1, 0);
        file.size = 10;
        file.allocated = 4096;
        let snapshot = FileArenaSnapshot {
            nodes: Arc::new(NodeStorage::Owned(vec![root, file])),
            string_pool: Arc::new(pool),
            dir_counts: Arc::new(vec![0, 0]),
        };
        let path = std::env::temp_dir().join("datatree_export_test.csv");
        super::export_csv(&snapshot, &path, true)?;
        let text = std::fs::read_to_string(&path)?;
        assert!(text.starts_with("File Name,Size,Allocated,Modified,Files,Folders"));
        assert!(text.contains("a.txt,10,4096"));
        assert_eq!(text.lines().count(), 2);
        let _ = std::fs::remove_file(&path);
        Ok(())
    }
}
