// terio entry point

use clap::Parser;
use terio::ask::{self, AskResult};
use terio::cache::ScriptCache;
use terio::cli::{Cli, Command};
use terio::identity::Identity;
use terio::log::reader::JsonlLogReader;
use terio::log::writer::JsonlLogWriter;
use terio::log::{LogReader, LogStore};
use terio::run;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let identity = Identity::load_or_create()?;
    let log_dir = JsonlLogWriter::default_dir()?;

    match cli.command {
        None | Some(Command::Ui) => {
            launch_ui();
        }

        Some(Command::Run { command }) => {
            handle_run(&identity, &log_dir, &command)?;
        }

        Some(Command::Ask { request }) => {
            handle_ask(&identity, &log_dir, &request)?;
        }

        Some(Command::Log { json }) => {
            let reader = JsonlLogReader::new(&log_dir);
            let entries = reader.recent(50)?;

            if json {
                println!("{}", serde_json::to_string_pretty(&entries)?);
            } else {
                print_log_plain(&entries);
            }
        }

        Some(Command::Stats) => {
            let reader = JsonlLogReader::new(&log_dir);
            let stats = ask::compute_stats(&reader)?;

            println!("Model calls:   {}", stats.model_calls);
            println!("Cache hits:    {}", stats.cache_hits);
            println!("Tokens total:  {}", stats.total_tokens);
            println!("Duration (ms): {}", stats.total_duration_ms);
            println!("Commands:      {}", stats.total_commands);
        }

        Some(Command::Cancel) => {
            eprintln!("terio cancel — отмена. Реализация в Phase 3.");
        }

        Some(Command::Config) => {
            eprintln!("terio config — настройки. Реализация в Phase 4.");
        }
    }

    Ok(())
}

fn handle_run(
    identity: &Identity,
    log_dir: &std::path::Path,
    command: &[String],
) -> anyhow::Result<()> {
    if command.is_empty() {
        eprintln!("error: команда не указана. Использование: terio run -- <command>");
        std::process::exit(1);
    }

    // Предупреждение для destructive/network_write
    let risk = run::compute_risk(&command[0], &command[1..]);
    if risk == terio::types::RiskLevel::Destructive {
        eprintln!("⚠️  ВНИМАНИЕ: destructive команда: {}", command.join(" "));
    } else if risk == terio::types::RiskLevel::NetworkWrite {
        eprintln!("⚠️  ВНИМАНИЕ: сетевая запись: {}", command.join(" "));
    } else if risk == terio::types::RiskLevel::CredentialAccess {
        eprintln!("⚠️  ВНИМАНИЕ: доступ к credentials: {}", command.join(" "));
    }

    let result = match run::execute(command) {
        Ok(r) => r,
        Err(e) => {
            // Логируем failed spawn
            let writer = Box::new(JsonlLogWriter::new(log_dir)?);
            let reader = Box::new(JsonlLogReader::new(log_dir));
            let store = LogStore::new(writer, reader, 256);

            let interaction_id = Identity::new_interaction_id();
            let request = command.join(" ");
            let cwd = std::env::current_dir()?.to_string_lossy().to_string();

            let entry = run::make_spawn_failed_entry(
                &identity.instance_id,
                &identity.session_id,
                Some(interaction_id),
                &request,
                &cwd,
                command,
                &e.to_string(),
            );
            store.append(entry)?;
            store.flush()?;

            eprintln!("error: {e}");
            std::process::exit(127);
        }
    };

    print!("{}", result.stdout);
    if !result.stderr.is_empty() {
        eprint!("{}", result.stderr);
    }

    let interaction_id = Identity::new_interaction_id();
    let request = command.join(" ");
    let cwd = std::env::current_dir()?.to_string_lossy().to_string();

    let entry = run::make_command_run_entry(
        &identity.instance_id,
        &identity.session_id,
        Some(interaction_id),
        &request,
        &cwd,
        command,
        &result,
    );

    let writer = Box::new(JsonlLogWriter::new(log_dir)?);
    let reader = Box::new(JsonlLogReader::new(log_dir));
    let store = LogStore::new(writer, reader, 256);
    store.append(entry)?;
    store.flush()?;

    std::process::exit(result.exit_code);
}

fn handle_ask(identity: &Identity, log_dir: &std::path::Path, request: &str) -> anyhow::Result<()> {
    let cache = ScriptCache::new()?;
    let writer = Box::new(JsonlLogWriter::new(log_dir)?);
    let reader = Box::new(JsonlLogReader::new(log_dir));
    let store = LogStore::new(writer, reader, 256);

    match ask::process_request(request, identity, &store, &cache)? {
        AskResult::CacheHit {
            entry,
            results,
            total_duration,
            all_exit_zero,
        } => {
            let status = if all_exit_zero { "ok" } else { "FAIL" };
            eprintln!(
                "[cache hit] {} [{}] (risk: {:?})",
                entry.normalized_request, status, entry.risk
            );
            for (i, result) in results.iter().enumerate() {
                if i > 0 {
                    println!("---");
                }
                print!("{}", result.stdout);
                if !result.stderr.is_empty() {
                    eprint!("{}", result.stderr);
                }
            }
            eprintln!("[done in {} ms]", total_duration.as_millis());
        }
        AskResult::FromAgent {
            entry,
            results,
            total_duration,
            all_exit_zero,
        } => {
            let cached = if entry.is_some() {
                ", cached"
            } else {
                " (not cached)"
            };
            let status = if all_exit_zero { "ok" } else { "FAIL" };
            eprintln!("[mock agent] {} [{}]{}", request, status, cached);
            for (i, result) in results.iter().enumerate() {
                if i > 0 {
                    println!("---");
                }
                print!("{}", result.stdout);
                if !result.stderr.is_empty() {
                    eprint!("{}", result.stderr);
                }
            }
            eprintln!("[done in {} ms{}]", total_duration.as_millis(), cached);
        }
        AskResult::Unknown => {
            eprintln!("terio: не знаю, как ответить на \"{request}\".");
            eprintln!("  Mock agent знает: list files, current directory, who am i, date and time, disk usage");
        }
    }

    store.flush()?;
    Ok(())
}

fn print_log_plain(entries: &[terio::types::LogEntry]) {
    if entries.is_empty() {
        eprintln!("(лог пуст)");
        return;
    }
    for entry in entries {
        let ts = &entry.ts[..19];
        let kind = format!("{:?}", entry.kind);
        let desc = entry
            .command
            .as_ref()
            .map(|c| &c.display[..std::cmp::min(60, c.display.len())])
            .or(entry.description.as_deref())
            .unwrap_or("—");
        let status = entry
            .status
            .as_ref()
            .map(|s| format!("{:?}", s))
            .unwrap_or_default();
        eprintln!("{ts} [{status}] {kind} {desc}");
    }
    eprintln!(
        "---\n{} записей. terio log --json для полного вывода",
        entries.len()
    );
}

#[cfg(feature = "desktop")]
fn launch_ui() {
    let log_dir = match JsonlLogWriter::default_dir() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("terio: не удалось определить директорию лога: {e}");
            std::process::exit(1);
        }
    };

    let store = LogStore::new(
        Box::new(JsonlLogWriter::new(&log_dir).unwrap_or_else(|e| {
            eprintln!("terio: не удалось открыть лог: {e}");
            std::process::exit(1);
        })),
        Box::new(JsonlLogReader::new(&log_dir)),
        256,
    );

    let entries = store.recent(50).unwrap_or_default();
    terio::ui::app::run_with_entries(entries);
}

#[cfg(not(feature = "desktop"))]
fn launch_ui() {
    eprintln!("terio: UI недоступен — соберите с '--features desktop' (требуются GTK3/webkit2gtk на Linux)");
}
