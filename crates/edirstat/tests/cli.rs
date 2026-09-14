// -- Clippy Denies --
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::{path::PathBuf, process::Command};

/// Creates a fresh, uniquely named temp dir under the crate's `target/` dir.
fn fresh_temp_dir(name: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let dir = std::env::current_dir()?.join("target").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn edirstat_cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_edirstat"))
}

/// `--benchmark` without a positional path must fail with a usage error.
#[test]
fn test_benchmark_without_path_errors() -> Result<(), Box<dyn std::error::Error>> {
    let output = edirstat_cli().arg("--benchmark").output()?;

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("path must be provided"),
        "unexpected stderr: {stderr}"
    );
    Ok(())
}

/// `--benchmark` on a directory with two files succeeds and prints traversal stats.
#[test]
fn test_benchmark_on_dir_succeeds() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = fresh_temp_dir("test_cli_benchmark_dir")?;
    std::fs::write(temp_dir.join("file_a.txt"), b"hello")?;
    std::fs::write(temp_dir.join("file_b.txt"), b"world!")?;

    let output = edirstat_cli().arg("--benchmark").arg(&temp_dir).output()?;

    assert!(
        output.status.success(),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Files scanned: 2"),
        "unexpected stdout: {stdout}"
    );
    assert!(
        stdout.contains("Directories scanned:"),
        "unexpected stdout: {stdout}"
    );
    assert!(
        stdout.contains("Total bytes:"),
        "unexpected stdout: {stdout}"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(())
}

/// `--benchmark` on a regular file (not a dir, not `$MFT`) must fail.
#[test]
fn test_benchmark_on_regular_file_errors() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = fresh_temp_dir("test_cli_benchmark_file")?;
    let file_path = temp_dir.join("regular_file.txt");
    std::fs::write(&file_path, b"data")?;

    let output = edirstat_cli().arg("--benchmark").arg(&file_path).output()?;

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not a directory"),
        "unexpected stderr: {stderr}"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(())
}

/// `--to <dest>` without an extension saves a compressed `<dest>.edst.zst` snapshot
/// that round-trips through `load_snapshot`.
#[test]
fn test_to_dest_saves_compressed_snapshot() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = fresh_temp_dir("test_cli_to_compressed")?;
    let scan_dir = temp_dir.join("scan_input");
    std::fs::create_dir_all(&scan_dir)?;
    std::fs::write(scan_dir.join("known_file.txt"), b"known contents")?;
    let dest = temp_dir.join("snapshot_out");

    let output = edirstat_cli()
        .arg("--to")
        .arg(&dest)
        .arg(&scan_dir)
        .output()?;

    assert!(
        output.status.success(),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let snapshot_path = temp_dir.join("snapshot_out.edst.zst");
    let bytes = std::fs::read(&snapshot_path)?;
    assert_eq!(bytes.get(..4), Some(&[0x28, 0xB5, 0x2F, 0xFD][..]));

    let (arena, string_pool) = edirstat::snapshot::load_snapshot(&snapshot_path)?;
    assert!(
        arena
            .nodes()
            .iter()
            .any(|node| string_pool.get(node.name_id) == Some("known_file.txt")),
        "scanned file name missing from snapshot string pool"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(())
}

/// `--to <dest> --no-compression` saves an uncompressed `<dest>.edst` snapshot
/// with the raw `EDST` magic that round-trips through `load_snapshot`.
#[test]
fn test_to_dest_saves_uncompressed_snapshot() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = fresh_temp_dir("test_cli_to_uncompressed")?;
    let scan_dir = temp_dir.join("scan_input");
    std::fs::create_dir_all(&scan_dir)?;
    std::fs::write(scan_dir.join("known_file.txt"), b"known contents")?;
    let dest = temp_dir.join("snapshot_out");

    let output = edirstat_cli()
        .arg("--to")
        .arg(&dest)
        .arg("--no-compression")
        .arg(&scan_dir)
        .output()?;

    assert!(
        output.status.success(),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let snapshot_path = temp_dir.join("snapshot_out.edst");
    let bytes = std::fs::read(&snapshot_path)?;
    assert_eq!(bytes.get(..4), Some(b"EDST".as_slice()));

    let (arena, string_pool) = edirstat::snapshot::load_snapshot(&snapshot_path)?;
    assert!(
        arena
            .nodes()
            .iter()
            .any(|node| string_pool.get(node.name_id) == Some("known_file.txt")),
        "scanned file name missing from snapshot string pool"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(())
}

/// `--to <dest>` without a positional scan path exits with code 1 and a usage error.
#[test]
fn test_to_without_scan_path_errors() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = fresh_temp_dir("test_cli_to_no_path")?;
    let dest = temp_dir.join("snapshot_out");

    let output = edirstat_cli().arg("--to").arg(&dest).output()?;

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("path to scan must be provided"),
        "unexpected stderr: {stderr}"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(())
}

/// `--export` without a positional scan path exits with code 1.
#[test]
fn test_export_without_scan_path_errors() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = fresh_temp_dir("test_cli_export_no_path")?;
    let dest = temp_dir.join("out.csv");

    let output = edirstat_cli().arg("--export").arg(&dest).output()?;

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("path to scan must be provided"),
        "unexpected stderr: {stderr}"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(())
}

/// `--export` writes a WizTree-like CSV; `--files-only` omits directories.
#[test]
fn test_export_csv_tree_and_files_only() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = fresh_temp_dir("test_cli_export_csv")?;
    let scan_dir = temp_dir.join("scan_input");
    std::fs::create_dir_all(&scan_dir)?;
    std::fs::write(scan_dir.join("tiny.txt"), b"abcd")?;
    let tree_csv = temp_dir.join("tree.csv");
    let files_csv = temp_dir.join("files.csv");

    let output = edirstat_cli()
        .arg("--export")
        .arg(&tree_csv)
        .arg(&scan_dir)
        .output()?;
    assert!(
        output.status.success(),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let tree_text = std::fs::read_to_string(&tree_csv)?;
    assert!(
        tree_text.starts_with("File Name,Size,Allocated,Modified,Files,Folders"),
        "unexpected header: {tree_text}"
    );
    assert!(
        tree_text.contains("tiny.txt"),
        "file row missing: {tree_text}"
    );
    assert!(
        !tree_text.contains(r"\\?\"),
        "UNC prefix should be stripped: {tree_text}"
    );

    let output = edirstat_cli()
        .arg("--export")
        .arg(&files_csv)
        .arg("--files-only")
        .arg(&scan_dir)
        .output()?;
    assert!(
        output.status.success(),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let files_text = std::fs::read_to_string(&files_csv)?;
    assert_eq!(
        files_text.lines().count(),
        2,
        "header + one file: {files_text}"
    );
    assert!(
        files_text.contains("tiny.txt,4,"),
        "logical size missing: {files_text}"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(())
}

/// `--bench-ui` prints one JSON line with scan/layout/TTI timings.
#[test]
fn test_bench_ui_prints_json() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = fresh_temp_dir("test_cli_bench_ui")?;
    std::fs::write(temp_dir.join("tiny.txt"), b"abcd")?;

    let output = edirstat_cli().arg("--bench-ui").arg(&temp_dir).output()?;
    assert!(
        output.status.success(),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout.lines().last().unwrap_or("");
    assert!(
        line.contains("\"scan_ms\":")
            && line.contains("\"layout_ms\":")
            && line.contains("\"time_to_interactive_ms\":")
            && line.contains("\"engine\":"),
        "unexpected stdout: {stdout}"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(())
}
