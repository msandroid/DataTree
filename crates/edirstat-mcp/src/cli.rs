use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};

use crate::error::ApiError;
use crate::install::{self, InstallTarget};
use crate::model::{CONFIRM_PERMANENT, CONFIRM_TRASH, DeleteModeArg, SortKey, json_error, json_ok};
use crate::session::Session;

#[derive(Parser, Debug)]
#[command(
    name = "datatree-mcp",
    about = "DataTree MCP server and JSON CLI for AI agents"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Speak MCP over stdin/stdout (default when no subcommand is given)
    Mcp,
    /// Scan a directory and print a JSON summary
    Scan {
        path: PathBuf,
        #[arg(long)]
        json: bool,
        #[arg(long, short = 'x')]
        same_filesystem: bool,
    },
    /// Search the scan with DataTree filter syntax
    Search {
        query: String,
        #[arg(long, short = 'p')]
        path: PathBuf,
        #[arg(long)]
        json: bool,
        #[arg(long, default_value_t = 0)]
        offset: u32,
        #[arg(long)]
        limit: Option<u32>,
        #[arg(long)]
        files_only: bool,
        #[arg(long, default_value = "allocated")]
        sort: String,
        #[arg(long, short = 'x')]
        same_filesystem: bool,
    },
    /// Immediate children of a path
    Children {
        #[arg(long, short = 'p')]
        path: PathBuf,
        #[arg(long)]
        dir: Option<String>,
        #[arg(long)]
        json: bool,
        #[arg(long, default_value_t = 0)]
        offset: u32,
        #[arg(long)]
        limit: Option<u32>,
        #[arg(long, default_value = "allocated")]
        sort: String,
        #[arg(long, short = 'x')]
        same_filesystem: bool,
    },
    /// Largest files and/or directories
    Top {
        #[arg(long, short = 'p')]
        path: PathBuf,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        n: Option<u32>,
        #[arg(long)]
        files: bool,
        #[arg(long)]
        dirs: bool,
        #[arg(long, default_value = "allocated")]
        sort: String,
        #[arg(long, short = 'x')]
        same_filesystem: bool,
    },
    /// Snapshot summary
    Summary {
        #[arg(long, short = 'p')]
        path: PathBuf,
        #[arg(long)]
        json: bool,
        #[arg(long, short = 'x')]
        same_filesystem: bool,
    },
    /// List volumes
    Volumes {
        #[arg(long)]
        json: bool,
    },
    /// Export CSV
    Export {
        #[arg(long, short = 'p')]
        path: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        files_only: bool,
        #[arg(long)]
        json: bool,
        #[arg(long, short = 'x')]
        same_filesystem: bool,
    },
    /// Prepare a delete plan (does not delete)
    PlanDelete {
        #[arg(long, short = 'p')]
        path: PathBuf,
        #[arg(long)]
        targets: Vec<String>,
        #[arg(long, default_value = "trash")]
        mode: String,
        #[arg(long)]
        json: bool,
        #[arg(long, short = 'x')]
        same_filesystem: bool,
    },
    /// Execute a delete plan from this process (CLI one-shot: use library tests for the handshake)
    ConfirmDelete {
        #[arg(long)]
        plan_id: String,
        #[arg(long)]
        token: String,
        #[arg(long)]
        confirm: String,
        #[arg(long)]
        json: bool,
    },
    /// Write native MCP config for Cursor, Claude, Codex, and/or Antigravity
    Install {
        #[arg(long)]
        cursor: bool,
        #[arg(long)]
        claude: bool,
        #[arg(long)]
        codex: bool,
        #[arg(long)]
        antigravity: bool,
        #[arg(long)]
        all: bool,
        #[arg(long)]
        global: bool,
    },
}

fn print_result<T: serde::Serialize>(value: Result<T, ApiError>, _json: bool) -> i32 {
    match value {
        Ok(v) => {
            println!("{}", json_ok(&v).unwrap_or_else(|e| json_error(&e)));
            0
        }
        Err(err) => {
            eprintln!("{}", json_error(&err));
            1
        }
    }
}

fn scan_session(path: &Path, same_filesystem: bool) -> Result<Session, ApiError> {
    let mut session = Session::new();
    session.scan(path, same_filesystem)?;
    Ok(session)
}

/// Parse argv and run. Returns a process exit code.
pub fn run() -> anyhow::Result<i32> {
    let cli = Cli::parse();
    match cli.command {
        None | Some(Command::Mcp) => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(crate::mcp::serve_stdio())?;
            Ok(0)
        }
        Some(Command::Scan {
            path,
            json,
            same_filesystem,
        }) => {
            let mut session = Session::new();
            Ok(print_result(session.scan(&path, same_filesystem), json))
        }
        Some(Command::Search {
            query,
            path,
            json,
            offset,
            limit,
            files_only,
            sort,
            same_filesystem,
        }) => {
            let session = scan_session(&path, same_filesystem)?;
            Ok(print_result(
                session.search(
                    &query,
                    offset,
                    limit,
                    files_only,
                    SortKey::parse(Some(&sort)),
                ),
                json,
            ))
        }
        Some(Command::Children {
            path,
            dir,
            json,
            offset,
            limit,
            sort,
            same_filesystem,
        }) => {
            let session = scan_session(&path, same_filesystem)?;
            Ok(print_result(
                session.children(dir.as_deref(), offset, limit, SortKey::parse(Some(&sort))),
                json,
            ))
        }
        Some(Command::Top {
            path,
            json,
            n,
            files,
            dirs,
            sort,
            same_filesystem,
        }) => {
            let session = scan_session(&path, same_filesystem)?;
            Ok(print_result(
                session.top(n, files, dirs, SortKey::parse(Some(&sort))),
                json,
            ))
        }
        Some(Command::Summary {
            path,
            json,
            same_filesystem,
        }) => {
            let session = scan_session(&path, same_filesystem)?;
            Ok(print_result(session.summary(), json))
        }
        Some(Command::Volumes { json }) => Ok(print_result(Ok(Session::list_volumes()), json)),
        Some(Command::Export {
            path,
            out,
            files_only,
            json,
            same_filesystem,
        }) => {
            let session = scan_session(&path, same_filesystem)?;
            match session.export_csv(&out, files_only) {
                Ok(()) => {
                    println!(
                        "{}",
                        serde_json::json!({"exported": out.display().to_string()})
                    );
                    let _ = json;
                    Ok(0)
                }
                Err(err) => {
                    eprintln!("{}", json_error(&err));
                    Ok(1)
                }
            }
        }
        Some(Command::PlanDelete {
            path,
            targets,
            mode,
            json,
            same_filesystem,
        }) => {
            let mode = DeleteModeArg::parse(&mode)
                .ok_or_else(|| anyhow::anyhow!("mode must be trash or permanent"))?;
            let mut session = scan_session(&path, same_filesystem)?;
            Ok(print_result(session.plan_delete(&targets, mode), json))
        }
        Some(Command::ConfirmDelete {
            plan_id,
            token,
            confirm,
            json,
        }) => {
            let _ = (
                plan_id,
                token,
                confirm,
                json,
                CONFIRM_TRASH,
                CONFIRM_PERMANENT,
            );
            eprintln!(
                "{}",
                json_error(&ApiError::msg(
                    "confirm_delete is session-based; use the MCP tools or keep a single datatree-mcp mcp process."
                ))
            );
            Ok(1)
        }
        Some(Command::Install {
            cursor,
            claude,
            codex,
            antigravity,
            all,
            global,
        }) => {
            let mut targets = Vec::new();
            if all || cursor {
                targets.push(InstallTarget::Cursor);
            }
            if all || claude {
                targets.push(InstallTarget::Claude);
            }
            if all || codex {
                targets.push(InstallTarget::Codex);
            }
            if all || antigravity {
                targets.push(InstallTarget::Antigravity);
            }
            if targets.is_empty() {
                anyhow::bail!("Specify --cursor, --claude, --codex, --antigravity, or --all");
            }
            let results = install::install(&targets, global)?;
            println!("{}", serde_json::to_string(&results)?);
            Ok(0)
        }
    }
}
