// terio entry point

fn main() -> anyhow::Result<()> {
    let cli = terio::cli::Cli::parse();

    match cli.command {
        terio::cli::Command::Run { command } => {
            // Phase 1: выполнение shell-команды
            eprintln!("terio run -- {}", command.join(" "));
        }
        terio::cli::Command::Ask { request } => {
            // Phase 2: запрос к AI-модели
            eprintln!("terio ask: {}", request);
        }
        terio::cli::Command::Log { json } => {
            if json {
                println!("[]");
            } else {
                eprintln!("terio log — откроется в UI");
            }
        }
        terio::cli::Command::Ui => {
            launch_ui();
        }
        _ => {
            launch_ui();
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
    eprintln!("terio: UI недоступен — соберите с '--features desktop' (требуются GTK3/webkit2gtk)");
}
