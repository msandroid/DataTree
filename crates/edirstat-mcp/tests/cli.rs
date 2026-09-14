#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::{path::PathBuf, process::Command};

fn fresh_temp_dir(name: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let dir = std::env::current_dir()?.join("target").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn mcp_cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_datatree-mcp"))
}

fn parse_json(
    output: &std::process::Output,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(stdout.trim())?;
    Ok(parsed)
}

#[test]
fn test_scan_json_summary() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = fresh_temp_dir("mcp_cli_scan")?;
    std::fs::write(temp_dir.join("a.txt"), b"hello")?;
    std::fs::write(temp_dir.join("b.bin"), b"world!")?;

    let output = mcp_cli()
        .arg("scan")
        .arg(&temp_dir)
        .arg("--json")
        .output()?;
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json = parse_json(&output)?;
    assert!(json["files"].as_u64().unwrap_or(0) >= 2, "{json}");
    assert!(json["engine"].as_str().is_some(), "{json}");
    assert!(json.get("allocated").is_some(), "{json}");

    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[test]
fn test_search_json() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = fresh_temp_dir("mcp_cli_search")?;
    std::fs::write(temp_dir.join("report.txt"), b"abc")?;
    std::fs::write(temp_dir.join("skip.log"), b"xyz")?;

    let output = mcp_cli()
        .arg("search")
        .arg("report")
        .arg("--path")
        .arg(&temp_dir)
        .arg("--json")
        .output()?;
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json = parse_json(&output)?;
    let items = json["items"].as_array().cloned().unwrap_or_default();
    assert!(
        items
            .iter()
            .any(|item| item["name"].as_str() == Some("report.txt")),
        "{json}"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[test]
fn test_plan_delete_rejects_volume_root() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = fresh_temp_dir("mcp_cli_plan_root")?;
    std::fs::write(temp_dir.join("keep.txt"), b"x")?;

    #[cfg(windows)]
    let root = "C:\\";
    #[cfg(not(windows))]
    let root = "/";

    let output = mcp_cli()
        .arg("plan-delete")
        .arg("--path")
        .arg(&temp_dir)
        .arg("--targets")
        .arg(root)
        .arg("--json")
        .output()?;
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("volume root") || stderr.contains("error"),
        "stderr: {stderr}"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[test]
fn test_session_confirm_requires_token() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = fresh_temp_dir("mcp_session_delete")?;
    let target = temp_dir.join("gone.txt");
    std::fs::write(&target, b"delete me")?;

    let mut session = edirstat_mcp::Session::new();
    session.scan(&temp_dir, false)?;
    let plan = session.plan_delete(
        &[target.to_string_lossy().into_owned()],
        edirstat_mcp::model::DeleteModeArg::Trash,
    )?;
    let err = session.confirm_delete(&plan.plan_id, "wrong-token", "TRASH");
    assert!(err.is_err(), "wrong token must fail: {err:?}");
    assert!(target.exists(), "file must still exist without confirm");

    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[test]
#[cfg(windows)]
fn test_session_trash_deletes_file() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = fresh_temp_dir("mcp_session_trash")?;
    let target = temp_dir.join("gone.txt");
    std::fs::write(&target, b"delete me")?;

    let mut session = edirstat_mcp::Session::new();
    session.scan(&temp_dir, false)?;
    let plan = session.plan_delete(
        &[target.to_string_lossy().into_owned()],
        edirstat_mcp::model::DeleteModeArg::Trash,
    )?;
    let result = session.confirm_delete(&plan.plan_id, &plan.confirm_token, "TRASH")?;
    assert_eq!(result.failed.len(), 0, "{result:?}");
    assert!(!target.exists(), "file should be in trash");

    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(())
}
