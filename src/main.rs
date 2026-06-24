// terio entry point

use clap::Parser;
use terio::ask::{self, AskResult};
use terio::cache::ScriptCache;
use terio::cli::{Cli, Command, ConfigCmd};
use terio::config::Config;
use terio::identity::Identity;
use terio::log::reader::JsonlLogReader;
use terio::log::writer::JsonlLogWriter;
use terio::log::{LogReader, LogStore};
use terio::provider::create_provider;
use terio::run;

fn main() -> anyhow::Result<()> {
    // Signal handler для Ctrl+C — ДО任何 другой инициализации
    run::setup_ctrlc_handler();

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

        Some(Command::Ask { request, yes }) => {
            handle_ask(&identity, &log_dir, &request, yes)?;
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
            if run::cancel_current() {
                eprintln!("terio: запрос отмены отправлен.");
            } else {
                eprintln!("terio: нет активного процесса для отмены.");
            }
        }

        Some(Command::Confirm) => {
            handle_confirm(&identity, &log_dir)?;
        }

        Some(Command::Config(cmd)) => {
            let mut config = Config::load().unwrap_or_default();
            match cmd {
                ConfigCmd::Show => {
                    config.print();
                }
                ConfigCmd::Set { key, value } => {
                    config.set(&key, &value)?;
                    config.save()?;
                    eprintln!("terio: config {key} установлен.");
                }
            }
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

    // Предупреждение для destructive/network_write/credential_access
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

fn handle_ask(
    identity: &Identity,
    log_dir: &std::path::Path,
    request: &str,
    yes: bool,
) -> anyhow::Result<()> {
    let config = Config::load().unwrap_or_default();
    let cache = ScriptCache::new()?;
    let writer = Box::new(JsonlLogWriter::new(log_dir)?);
    let reader = Box::new(JsonlLogReader::new(log_dir));
    let store = LogStore::new(writer, reader, 256);

    let provider = create_provider(&config.provider);

    render_ask_result(
        ask::process_request(request, identity, &store, &cache, &*provider, yes)?,
        &store,
        request,
    )?;

    store.flush()?;
    Ok(())
}

fn handle_confirm(identity: &Identity, log_dir: &std::path::Path) -> anyhow::Result<()> {
    let cache = ScriptCache::new()?;
    let writer = Box::new(JsonlLogWriter::new(log_dir)?);
    let reader = Box::new(JsonlLogReader::new(log_dir));
    let store = LogStore::new(writer, reader, 256);

    render_ask_result(
        ask::confirm_pending(identity, &store, &cache)?,
        &store,
        "<pending>",
    )?;

    store.flush()?;
    Ok(())
}

fn render_ask_result(result: AskResult, store: &LogStore, request: &str) -> anyhow::Result<()> {
    match result {
        AskResult::CacheHit {
            entry,
            results,
            total_duration,
            all_exit_zero,
        } => {
            let _ = ask::clear_pending_confirmation();
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
            plan,
        } => {
            let _ = ask::clear_pending_confirmation();
            let cached = if entry.is_some() {
                ", cached"
            } else {
                " (not cached)"
            };
            let status = if all_exit_zero { "ok" } else { "FAIL" };
            eprintln!("[agent] {} [{}]{}", plan.summary, status, cached);
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
        }
        AskResult::PendingConfirmation {
            source,
            plan_summary,
            execution,
        } => {
            let _ = ask::save_pending_confirmation(
                &ask::PendingConfirmationState {
                    source: source.clone(),
                    plan_summary: plan_summary.clone(),
                },
                &execution,
            );
            eprintln!(
                "[pending {:?}] {} (risk: {:?})",
                source, plan_summary.summary, plan_summary.risk
            );
            for cmd in &plan_summary.commands {
                eprintln!("   > {}", cmd.argv.join(" "));
            }
            if let Some(trust) = &plan_summary.trust {
                eprintln!("   trust: {} [{}]", trust.trust_label, trust.reason);
            }
            eprintln!("terio: требуется подтверждение. Выполните `terio confirm`.");
        }
        AskResult::Declined => {
            let _ = ask::clear_pending_confirmation();
            eprintln!("terio: отменено пользователем.");
        }
    }
    let _ = request;
    let _ = store;
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
