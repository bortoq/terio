// terio entry point

use clap::Parser;
use terio::cli::{Cli, Command};
use terio::identity::Identity;
use terio::log::reader::JsonlLogReader;
use terio::log::writer::JsonlLogWriter;
use terio::log::{LogReader, LogStore};
use terio::run;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Identity обязателен для всех операций
    let identity = Identity::load_or_create()?;
    let log_dir = JsonlLogWriter::default_dir()?;

    match cli.command {
        None | Some(Command::Ui) => {
            // cargo run или terio ui → открыть Dioxus окно
            launch_ui();
        }

        Some(Command::Run { command }) => {
            // terio run -- <command>
            if command.is_empty() {
                eprintln!("error: команда не указана. Использование: terio run -- <command>");
                std::process::exit(1);
            }

            // Выполнить
            let result = run::execute(&command)?;

            // Создать LogEntry
            let interaction_id = Identity::new_interaction_id();
            let request = command.join(" ");
            let cwd = std::env::current_dir()?.to_string_lossy().to_string();

            // Вывести stdout/stderr пользователю
            print!("{}", result.stdout);
            if !result.stderr.is_empty() {
                eprint!("{}", result.stderr);
            }

            let entry = run::make_command_run_entry(
                &identity.instance_id,
                &identity.session_id,
                Some(interaction_id),
                &request,
                &cwd,
                &command,
                &result,
            );

            // Записать в лог
            let writer = Box::new(JsonlLogWriter::new(&log_dir)?);
            let reader = Box::new(JsonlLogReader::new(&log_dir));
            let store = LogStore::new(writer, reader, 256);
            store.append(entry)?;
            store.flush()?;

            std::process::exit(result.exit_code);
        }

        Some(Command::Log { json }) => {
            // terio log — показать лог
            let reader = JsonlLogReader::new(&log_dir);
            let entries = reader.recent(50)?;

            if json {
                println!("{}", serde_json::to_string_pretty(&entries)?);
            } else {
                // plain text в stderr (stdout зарезервирован для pipe)
                if entries.is_empty() {
                    eprintln!("(лог пуст)");
                } else {
                    for entry in &entries {
                        let ts = &entry.ts[..19]; // YYYY-MM-DDTHH:MM:SS
                        let kind = serde_json::to_string(&entry.kind).unwrap_or_default();
                        let desc = entry
                            .command
                            .as_ref()
                            .map(|c| &c.display[..std::cmp::min(60, c.display.len())])
                            .or(entry.description.as_deref())
                            .unwrap_or("—");
                        eprintln!("{ts} {kind} {desc}");
                    }
                }
                eprintln!(
                    "---\n{} записей. terio log --json для полного вывода",
                    entries.len()
                );
            }
        }

        Some(Command::Ask { request }) => {
            // Phase 2: заглушка
            eprintln!("terio ask \"{request}\" — запрос к AI. Реализация в Phase 2.");
        }

        Some(Command::Stats) => {
            // Phase 2: заглушка
            eprintln!("terio stats — метрики. Реализация в Phase 2.");
        }

        Some(Command::Cancel) => {
            // Phase 3: заглушка
            eprintln!("terio cancel — отмена. Реализация в Phase 3.");
        }

        Some(Command::Config) => {
            // Phase 4: заглушка
            eprintln!("terio config — настройки. Реализация в Phase 4.");
        }
    }

    Ok(())
}

#[cfg(feature = "desktop")]
fn launch_ui() {
    terio::ui::app::run();
}

#[cfg(not(feature = "desktop"))]
fn launch_ui() {
    eprintln!("terio: UI недоступен — соберите с '--features desktop' (требуются GTK3/webkit2gtk на Linux)");
}
