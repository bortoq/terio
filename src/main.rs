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
#[cfg(feature = "desktop")]
use terio::ui::app::UiCommand;
use terio::undo;

fn main() -> anyhow::Result<()> {
    // Signal handler для Ctrl+C — ДО любой другой инициализации
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

        Some(Command::Undo) => {
            handle_undo(&identity, &log_dir)?;
        }

        Some(Command::Redo) => {
            handle_redo(&identity, &log_dir)?;
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

        Some(Command::Learn { program }) => {
            handle_learn(&program)?;
        }

        Some(Command::Integrations) => {
            handle_integrations()?;
        }

        Some(Command::Forget { program }) => {
            handle_forget(&program)?;
        }

        Some(Command::Share { output, count }) => {
            handle_share(&log_dir, output.as_deref(), count)?;
        }

        Some(Command::Receive { input }) => {
            handle_receive(&log_dir, &input)?;
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
    if matches!(
        risk,
        terio::types::RiskLevel::LocalWrite
            | terio::types::RiskLevel::Destructive
            | terio::types::RiskLevel::NetworkWrite
    ) {
        eprintln!("{}", undo::direct_run_warning());
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

fn handle_undo(identity: &Identity, log_dir: &std::path::Path) -> anyhow::Result<()> {
    let writer = Box::new(JsonlLogWriter::new(log_dir)?);
    let reader = Box::new(JsonlLogReader::new(log_dir));
    let store = LogStore::new(writer, reader, 256);
    match undo::undo_latest()? {
        Some(record) => {
            eprintln!("terio: undo выполнен для \"{}\".", record.summary);
            append_system_event(
                identity,
                &store,
                &format!("undo applied: {}", record.summary),
            )?;
        }
        None => eprintln!("terio: нет доступного undo."),
    }
    store.flush()?;
    Ok(())
}

fn handle_redo(identity: &Identity, log_dir: &std::path::Path) -> anyhow::Result<()> {
    let writer = Box::new(JsonlLogWriter::new(log_dir)?);
    let reader = Box::new(JsonlLogReader::new(log_dir));
    let store = LogStore::new(writer, reader, 256);
    match undo::redo_latest()? {
        Some(record) => {
            eprintln!("terio: redo выполнен для \"{}\".", record.summary);
            append_system_event(
                identity,
                &store,
                &format!("redo applied: {}", record.summary),
            )?;
        }
        None => eprintln!("terio: нет доступного redo."),
    }
    store.flush()?;
    Ok(())
}

/// Phase 7: Learn a program via --help
fn handle_learn(program: &str) -> anyhow::Result<()> {
    let mut mgr = terio::integration::IntegrationManager::new()?;
    eprintln!("terio: learning '{}'...", program);
    let record = mgr.learn_program(program)?;
    match &record.status {
        terio::integration::LearningStatus::Learned => {
            eprintln!("terio: learned '{}' ✓", program);
            if let Some(snippet) = &record.help_snippet {
                let preview: String = snippet.chars().take(200).collect();
                println!("{}", preview);
            }
        }
        terio::integration::LearningStatus::Failed(reason) => {
            eprintln!("terio: failed to learn '{}': {}", program, reason);
        }
        _ => {
            eprintln!("terio: learning '{}' in unexpected state", program);
        }
    }
    Ok(())
}

/// Phase 7: Show integration status
fn handle_integrations() -> anyhow::Result<()> {
    let mgr = terio::integration::IntegrationManager::new()?;
    terio::integration::print_integration_status(&mgr);
    Ok(())
}

/// Phase 7: Forget a program
fn handle_forget(program: &str) -> anyhow::Result<()> {
    let mut mgr = terio::integration::IntegrationManager::new()?;
    mgr.forget_program(program)?;
    eprintln!("terio: forgot '{}'", program);
    Ok(())
}

/// Phase 7: Share window data
fn handle_share(
    log_dir: &std::path::Path,
    output: Option<&str>,
    count: usize,
) -> anyhow::Result<()> {
    let reader = JsonlLogReader::new(log_dir);
    let entries = reader.recent(count)?;
    let cache = ScriptCache::new()?;
    let json = terio::integration::export_share_data(entries, &cache)?;

    match output {
        Some(path) => {
            std::fs::write(path, &json)?;
            eprintln!("terio: shared window saved to {}", path);
        }
        None => {
            println!("{}", json);
        }
    }
    Ok(())
}

/// Phase 7: Receive shared window data
fn handle_receive(log_dir: &std::path::Path, input: &str) -> anyhow::Result<()> {
    let json_data = if input == "-" {
        use std::io::Read;
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        buf
    } else {
        std::fs::read_to_string(input)
            .map_err(|e| anyhow::anyhow!("failed to read {}: {}", input, e))?
    };

    let writer = Box::new(JsonlLogWriter::new(log_dir)?);
    let reader = Box::new(JsonlLogReader::new(log_dir));
    let store = LogStore::new(writer, reader, 256);

    let count = terio::integration::import_share_data(&json_data, &store)?;
    eprintln!("terio: received {} entries from shared window", count);
    Ok(())
}

fn append_system_event(
    identity: &Identity,
    store: &LogStore,
    description: &str,
) -> anyhow::Result<()> {
    let entry = terio::types::LogEntry::new_system_event(
        &identity.instance_id,
        &identity.session_id,
        description,
    );
    store.append(entry)?;
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
            plan_hash,
            source,
            plan_summary,
            execution,
        } => {
            let _ = ask::save_pending_confirmation(
                &ask::PendingConfirmationState {
                    plan_hash,
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
        let ts = terio::run::truncate_safe(&entry.ts, 19);
        let kind = format!("{:?}", entry.kind);
        let desc = entry
            .command
            .as_ref()
            .map(|c| terio::run::truncate_safe(&c.display, 60))
            .or_else(|| {
                entry
                    .description
                    .as_ref()
                    .map(|d| terio::run::truncate_safe(d, 60))
            })
            .unwrap_or_else(|| "—".to_string());
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
    use std::sync::mpsc;

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

    let identity = Identity::load_or_create().unwrap_or_else(|e| {
        eprintln!("terio: не удалось загрузить identity: {e}");
        std::process::exit(1);
    });
    let live_stream = store.stream();
    let entries = store.recent(50).unwrap_or_default();
    let (tx, rx) = mpsc::channel::<UiCommand>();

    std::thread::spawn(move || {
        process_ui_commands(identity, store, rx);
    });

    terio::ui::app::run_with_entries_and_runtime(entries, Some(live_stream), Some(tx));
}

#[cfg(not(feature = "desktop"))]
fn launch_ui() {
    eprintln!("terio: UI недоступен — соберите с '--features desktop' (требуются GTK3/webkit2gtk на Linux)");
}

#[cfg(feature = "desktop")]
fn process_ui_commands(
    identity: Identity,
    store: LogStore,
    rx: std::sync::mpsc::Receiver<UiCommand>,
) {
    for command in rx {
        if let Err(err) = process_one_ui_command(&identity, &store, command) {
            let _ = append_system_event(&identity, &store, &format!("ui action failed: {err}"));
            let _ = store.flush();
        }
    }
}

#[cfg(feature = "desktop")]
fn process_one_ui_command(
    identity: &Identity,
    store: &LogStore,
    command: UiCommand,
) -> anyhow::Result<()> {
    match command {
        UiCommand::Ask(request) => {
            let config = Config::load().unwrap_or_default();
            let cache = ScriptCache::new()?;
            let provider = create_provider(&config.provider);
            match ask::process_request(&request, identity, store, &cache, &*provider, false)? {
                AskResult::PendingConfirmation {
                    plan_hash,
                    source,
                    plan_summary,
                    execution,
                } => {
                    ask::save_pending_confirmation(
                        &ask::PendingConfirmationState {
                            plan_hash,
                            source,
                            plan_summary: plan_summary.clone(),
                        },
                        &execution,
                    )?;
                    append_system_event(
                        identity,
                        store,
                        &format!("pending confirmation: {}", plan_summary.summary),
                    )?;
                }
                AskResult::Unknown => {
                    append_system_event(identity, store, &format!("unknown request: {request}"))?;
                }
                AskResult::Declined => {
                    let _ = ask::clear_pending_confirmation();
                    append_system_event(identity, store, "request declined")?;
                }
                AskResult::CacheHit { .. } | AskResult::FromAgent { .. } => {
                    let _ = ask::clear_pending_confirmation();
                }
            }
        }
        UiCommand::Confirm => {
            let cache = ScriptCache::new()?;
            match ask::confirm_pending(identity, store, &cache)? {
                AskResult::Unknown => {
                    append_system_event(identity, store, "no pending confirmation")?;
                }
                AskResult::Declined => {
                    append_system_event(identity, store, "pending plan declined")?;
                }
                _ => {
                    let _ = ask::clear_pending_confirmation();
                }
            }
        }
        UiCommand::Undo => match undo::undo_latest()? {
            Some(record) => {
                append_system_event(
                    identity,
                    store,
                    &format!("undo applied: {}", record.summary),
                )?;
            }
            None => {
                append_system_event(identity, store, "no undo available")?;
            }
        },
        UiCommand::Redo => match undo::redo_latest()? {
            Some(record) => {
                append_system_event(
                    identity,
                    store,
                    &format!("redo applied: {}", record.summary),
                )?;
            }
            None => {
                append_system_event(identity, store, "no redo available")?;
            }
        },
    }
    store.flush()?;
    Ok(())
}
