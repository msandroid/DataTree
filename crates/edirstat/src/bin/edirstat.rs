#![forbid(unsafe_code)]
// -- Clippy Denies --
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
// --- Clippy Lint Groups & Specific Warnings ---
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::cargo)]
#![warn(clippy::needless_return)]
// --- Allowed Lints (Overrides) ---
#![allow(clippy::mod_module_files)]
#![allow(clippy::unseparated_literal_suffix)]
#![allow(clippy::missing_inline_in_public_items)]
#![allow(clippy::panic)]
#![allow(clippy::multiple_crate_versions)]
#![allow(clippy::blanket_clippy_restriction_lints)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cognitive_complexity)]
#![allow(clippy::cargo_common_metadata)]
#![allow(clippy::future_not_send)]
#![allow(clippy::used_underscore_binding)]
#![allow(clippy::crate_in_macro_def)]
#![allow(clippy::too_many_lines)]

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use clap::Parser;
use edirstat::{
    coordinator::SharedState, engine::scanner::EngineScanController, gui::GuiApp,
    traversal::TraversalEngine,
};

#[global_allocator]
static GLOBAL: mimalloc_rspack::MiMalloc = mimalloc_rspack::MiMalloc;

#[derive(Parser, Debug)]
#[command(
    name = "datatree",
    author,
    version,
    about = "DataTree — disk usage analyzer (eDirStat engine)"
)]
struct Args {
    /// Directory to scan or snapshot file to load
    path: Option<PathBuf>,

    /// Run in headless benchmark mode to measure scan time on the target directory and exit
    #[arg(long)]
    benchmark: bool,

    /// Measure scan, treemap layout, and time-to-interactive; print JSON and exit
    #[arg(long)]
    bench_ui: bool,

    /// Destination path or directory to save the scanned snapshot file (no-GUI/headless)
    #[arg(long)]
    to: Option<PathBuf>,

    /// Export a WizTree-like CSV of the scan and exit
    #[arg(long)]
    export: Option<PathBuf>,

    /// CSV export lists files only (File View)
    #[arg(long)]
    files_only: bool,

    /// Disable Zstd compression for the output snapshot file (saves as uncompressed .edst)
    #[arg(long)]
    no_compression: bool,

    /// Restrict directory traversal to the same filesystem/device boundary
    #[arg(long, short = 'x', alias = "one-file-system")]
    same_filesystem: bool,
}

fn run_benchmark(
    path_opt: Option<PathBuf>,
    same_filesystem: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let path = path_opt.ok_or("Error: A path must be provided for benchmarking.")?;
    if !path.exists() {
        return Err(format!("Error: Path does not exist: {}", path.display()).into());
    }
    let path = std::fs::canonicalize(&path)?;

    let is_mft = path
        .file_name()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s.eq_ignore_ascii_case("$mft"));

    if !is_mft && !path.is_dir() {
        return Err(format!("Error: Path is not a directory: {}", path.display()).into());
    }

    println!(
        "Running edirstat benchmark on {}: {}",
        if is_mft { "mft" } else { "dir" },
        path.display()
    );

    let shared_state = Arc::new(SharedState::new());
    let traversal_engine = Arc::new(TraversalEngine::new(shared_state.scan_stats.clone()));
    let (tx, rx) = crossbeam::channel::unbounded();

    let start = std::time::Instant::now();
    let handle = traversal_engine.start_traversal(
        path.clone(),
        same_filesystem,
        shared_state.scan_cancel.clone(),
        tx,
    )?;

    let mut coordinator = edirstat::coordinator::Coordinator::new(rx, shared_state);
    coordinator.run_coordinator_loop(&path.to_string_lossy());

    let _ = handle.join();
    let duration = start.elapsed();

    let stats = traversal_engine.stats();
    let files = stats
        .files_scanned
        .load(std::sync::atomic::Ordering::SeqCst);
    let dirs = stats.dirs_scanned.load(std::sync::atomic::Ordering::SeqCst);
    let bytes = stats
        .bytes_scanned
        .load(std::sync::atomic::Ordering::SeqCst);

    println!("----------------------------------------");
    println!("Time elapsed: {duration:?}");
    println!("Directories scanned: {dirs}");
    println!("Files scanned: {files}");
    println!("Total bytes: {bytes}");
    println!("----------------------------------------");
    Ok(())
}

fn run_headless_scan_and_save(
    scan_path: &Path,
    mut to_path: PathBuf,
    no_compression: bool,
    same_filesystem: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if !scan_path.exists() {
        return Err(format!("Error: Scan path does not exist: {}", scan_path.display()).into());
    }
    let scan_path = std::fs::canonicalize(scan_path)?;

    let is_mft = scan_path
        .file_name()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s.eq_ignore_ascii_case("$mft"));

    if !is_mft && !scan_path.is_dir() {
        return Err(format!(
            "Error: Scan path is not a directory: {}",
            scan_path.display()
        )
        .into());
    }

    let ext = if no_compression { "edst" } else { "edst.zst" };
    if to_path.extension().is_none_or(|s| s != ext) {
        to_path = to_path.with_added_extension(ext);
    }

    println!("Headless scanning started for: {}", scan_path.display());

    let shared_state = Arc::new(SharedState::new());
    let traversal_engine = Arc::new(TraversalEngine::new(shared_state.scan_stats.clone()));
    let (tx, rx) = crossbeam::channel::unbounded();

    let handle = traversal_engine.start_traversal(
        scan_path.clone(),
        same_filesystem,
        shared_state.scan_cancel.clone(),
        tx,
    )?;

    let mut coordinator = edirstat::coordinator::Coordinator::new(rx, shared_state.clone());
    coordinator.run_coordinator_loop(&scan_path.to_string_lossy());

    let _ = handle.join();

    let snapshot = shared_state.current_snapshot.load();
    if snapshot.nodes.is_empty() {
        return Err("Error: The completed scan resulted in an empty snapshot.".into());
    }

    let mut dest_path = to_path;
    if dest_path.is_dir() {
        let folder_name = scan_path
            .file_name()
            .map_or_else(|| "root".to_string(), |s| s.to_string_lossy().into_owned());
        dest_path.push(format!("{folder_name}.{ext}"));
    }

    println!("Saving snapshot to: {}", dest_path.display());
    edirstat::snapshot::save_snapshot(
        &snapshot.nodes,
        &snapshot.string_pool,
        &dest_path,
        !no_compression,
    )?;
    println!("Snapshot saved successfully.");

    Ok(())
}

fn scan_directory(
    path: &Path,
    same_filesystem: bool,
) -> Result<Arc<SharedState>, Box<dyn std::error::Error>> {
    let path = std::fs::canonicalize(path)?;
    let is_mft = path
        .file_name()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s.eq_ignore_ascii_case("$mft"));
    if !is_mft && !path.is_dir() {
        return Err(format!("Error: Path is not a directory: {}", path.display()).into());
    }

    let shared_state = Arc::new(SharedState::new());
    let traversal_engine = Arc::new(TraversalEngine::new(shared_state.scan_stats.clone()));
    let (tx, rx) = crossbeam::channel::unbounded();
    let handle = traversal_engine.start_traversal(
        path.clone(),
        same_filesystem,
        shared_state.scan_cancel.clone(),
        tx,
    )?;
    let mut coordinator = edirstat::coordinator::Coordinator::new(rx, shared_state.clone());
    coordinator.run_coordinator_loop(&path.to_string_lossy());
    let _ = handle.join();
    Ok(shared_state)
}

fn run_export(
    scan_path: &Path,
    export_path: &Path,
    files_only: bool,
    same_filesystem: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if !scan_path.exists() {
        return Err(format!("Error: Scan path does not exist: {}", scan_path.display()).into());
    }
    let shared_state = scan_directory(scan_path, same_filesystem)?;
    let snapshot = shared_state.current_snapshot.load();
    if snapshot.nodes.is_empty() {
        return Err("Error: The completed scan resulted in an empty snapshot.".into());
    }
    edirstat::csv::export_csv(&snapshot, export_path, files_only)?;
    println!("Exported CSV: {}", export_path.display());
    Ok(())
}

fn run_ui_bench(
    path_opt: Option<PathBuf>,
    same_filesystem: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let path = path_opt.ok_or("Error: A path must be provided for --bench-ui.")?;
    if !path.exists() {
        return Err(format!("Error: Path does not exist: {}", path.display()).into());
    }

    let mut scan_ms = Vec::new();
    let mut layout_ms = Vec::new();
    let mut engine_code = 0u8;
    let mut files = 0usize;
    let mut dirs = 0usize;

    // 2 warmup + 3 measured passes
    for iteration in 0..5 {
        let start_scan = std::time::Instant::now();
        let shared_state = scan_directory(&path, same_filesystem)?;
        let scan_elapsed = start_scan.elapsed();

        let snapshot = shared_state.current_snapshot.load();
        let start_layout = std::time::Instant::now();
        let mut chart = edirstat::gui::stats::treemap::TreemapChart::new();
        let _blocks = chart.bench_layout(&snapshot, 1200.0, 600.0);
        let layout_elapsed = start_layout.elapsed();

        engine_code = shared_state
            .scan_stats
            .scan_engine
            .load(std::sync::atomic::Ordering::SeqCst);
        files = shared_state
            .scan_stats
            .files_scanned
            .load(std::sync::atomic::Ordering::SeqCst);
        dirs = shared_state
            .scan_stats
            .dirs_scanned
            .load(std::sync::atomic::Ordering::SeqCst);

        if iteration >= 2 {
            scan_ms.push(scan_elapsed.as_secs_f64() * 1000.0);
            layout_ms.push(layout_elapsed.as_secs_f64() * 1000.0);
        }
    }

    let avg_scan = scan_ms.iter().sum::<f64>() / scan_ms.len() as f64;
    let avg_layout = layout_ms.iter().sum::<f64>() / layout_ms.len() as f64;
    let tti = avg_scan + avg_layout;
    let engine = match engine_code {
        1 => "MFT",
        2 => "Walk",
        _ => "none",
    };
    println!(
        "{{\"scan_ms\":{avg_scan:.3},\"layout_ms\":{avg_layout:.3},\"time_to_interactive_ms\":{tti:.3},\"engine\":\"{engine}\",\"files\":{files},\"dirs\":{dirs},\"warmup\":2,\"measured\":3}}"
    );
    Ok(())
}

fn launch_datatree_mcp() -> anyhow::Result<()> {
    let extra: Vec<std::ffi::OsString> = std::env::args_os().skip(2).collect();
    let mut candidates = Vec::new();
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        candidates.push(dir.join("datatree-mcp"));
        candidates.push(dir.join("datatree-mcp.exe"));
    }
    for bin in &candidates {
        if bin.exists() {
            let status = std::process::Command::new(bin).args(&extra).status()?;
            std::process::exit(status.code().unwrap_or(1));
        }
    }
    match std::process::Command::new("datatree-mcp")
        .args(&extra)
        .status()
    {
        Ok(status) => std::process::exit(status.code().unwrap_or(1)),
        Err(err) => Err(anyhow::anyhow!(
            "datatree-mcp was not found next to this executable or on PATH ({err})"
        )),
    }
}

fn main() -> anyhow::Result<()> {
    #[cfg(feature = "profile-tracy")]
    let _client = tracy_client::Client::start();

    if std::env::args().nth(1).as_deref() == Some("mcp") {
        return launch_datatree_mcp();
    }

    if !cli_or_gui::is_launched_from_terminal() {
        cli_or_gui::hide_console_window();
    }

    let args = Args::parse();

    if args.benchmark {
        run_benchmark(args.path, args.same_filesystem).map_err(|e| anyhow::anyhow!("{e}"))?;

        return Ok(());
    }

    if args.bench_ui {
        run_ui_bench(args.path, args.same_filesystem).map_err(|e| anyhow::anyhow!("{e}"))?;
        return Ok(());
    }

    if let Some(export_path) = args.export {
        let scan_path = args.path.unwrap_or_else(|| {
            eprintln!("Error: A path to scan must be provided when utilizing the --export option.");
            std::process::exit(1);
        });
        run_export(
            &scan_path,
            &export_path,
            args.files_only,
            args.same_filesystem,
        )
        .map_err(|e| anyhow::anyhow!("{e}"))?;
        return Ok(());
    }

    if let Some(to_path) = args.to {
        let scan_path = args.path.unwrap_or_else(|| {
            eprintln!("Error: A path to scan must be provided when utilizing the --to option.");
            std::process::exit(1);
        });

        run_headless_scan_and_save(
            &scan_path,
            to_path,
            args.no_compression,
            args.same_filesystem,
        )
        .map_err(|e| anyhow::anyhow!("{e}"))?;

        return Ok(());
    }

    // Create system states
    let shared_state = Arc::new(SharedState::new());
    let traversal_engine = Arc::new(TraversalEngine::new(shared_state.scan_stats.clone()));
    let scanner = Arc::new(EngineScanController::new(
        traversal_engine,
        shared_state.clone(),
    ));

    // Native boot options for eframe
    let native_options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("DataTree - Disk Usage Analyzer")
            .with_icon(
                eframe::icon_data::from_png_bytes(
                    &include_bytes!("../../assets/img/icon_512x.png")[..],
                )
                .map_err(|e| {
                    eprintln!("Failed to load icon: {e}");
                    eframe::Error::AppCreation(Box::new(e))
                })?,
            )
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([800.0, 600.0]),
        ..Default::default()
    };

    let initial_path = args.path;

    eframe::run_native(
        "datatree",
        native_options,
        Box::new(move |_cc| {
            Ok(Box::new(GuiApp::new(
                shared_state,
                Some(scanner),
                initial_path,
                args.same_filesystem,
            )))
        }),
    )?;

    Ok(())
}
