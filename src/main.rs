// terio entry point — Phase 0: terminal-like UI

use anyhow::Context;
use clap::Parser;
use std::sync::Arc;
use terio::ask::{self, AskResult};
use terio::cache::ScriptCache;
use terio::cli::{AliasCmd, Cli, Command, ConfigCmd, RegistryCmd, SandboxCmd, ScriptCmd};
use terio::config::Config;
use terio::identity::Identity;
use terio::log::reader::JsonlLogReader;
use terio::log::writer::JsonlLogWriter;
use terio::log::{LogReader, LogStore};
use terio::proactive::ProactiveEngine;
use terio::provider::create_provider;
use terio::run;
use terio::script_engine::{self, CliApiBackend, ScriptEngine};
#[cfg(feature = "desktop")]
use terio::ui::state::UiCommand;
use terio::undo;
#[allow(unused_imports)]
use terio::window;

fn main() -> anyhow::Result<()> {
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
            if !yes {
                // 1) Check synonym index first (learned from past LLM success)
                if let Some(entry) = try_synonym_command(&request)? {
                    if entry {
                        // handled by synonym
                    } else {
                        handle_ask(&identity, &log_dir, &request, yes)?;
                    }
                // 2) Check ScriptEngine
                } else if let Some(true) = try_script_command(&request)? {
                    // handled by script
                } else {
                    handle_ask(&identity, &log_dir, &request, yes)?;
                }
            } else {
                handle_ask(&identity, &log_dir, &request, yes)?;
            }
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

        // --- Phase 2: Script commands ---
        Some(Command::Script(cmd)) => {
            handle_script(cmd)?;
        }

        // --- Phase 3: Synonym commands ---
        Some(Command::Alias(cmd)) => {
            handle_alias(cmd)?;
        }

        // --- Phase 0: Terminal commands (routed through ScriptEngine) ---
        Some(Command::Help) => {
            if let Some(true) = try_script_command("help")? {
                // handled by script
            } else {
                print_help();
            }
        }

        Some(Command::Mode { mode }) => {
            // Route through script: "mode <value>"
            if try_script_command(&format!("mode {mode}"))? == Some(true) {
                // handled by script
            } else {
                handle_mode(&mode)?;
            }
        }

        Some(Command::Focus { direction }) => {
            if try_script_command(&format!("focus {direction}"))? == Some(true) {
                // handled by script
            } else {
                handle_focus(&direction);
            }
        }

        Some(Command::Scroll { lines }) => {
            if try_script_command(&format!("scroll {lines}"))? == Some(true) {
                // handled by script
            } else {
                handle_scroll(lines);
            }
        }

        Some(Command::Repeat) => {
            if try_script_command("repeat")? == Some(true) {
                // handled by script
            } else {
                handle_repeat(&identity, &log_dir)?;
            }
        }

        Some(Command::Sandbox(cmd)) => {
            handle_sandbox(cmd)?;
        }

        // --- Phase 5: Cost report ---
        Some(Command::Cost) => {
            handle_cost(&log_dir)?;
        }

        // --- Phase 6: Community registry ---
        Some(Command::Registry(cmd)) => {
            handle_registry(cmd)?;
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Existing handlers (unchanged)
// ---------------------------------------------------------------------------

fn handle_run(
    identity: &Identity,
    log_dir: &std::path::Path,
    command: &[String],
) -> anyhow::Result<()> {
    if command.is_empty() {
        eprintln!("error: команда не указана. Использование: terio run -- <command>");
        std::process::exit(1);
    }

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

fn handle_cost(log_dir: &std::path::Path) -> anyhow::Result<()> {
    let reader = JsonlLogReader::new(log_dir);
    let entries = reader.recent(usize::MAX)?;
    let config = Config::load().unwrap_or_default();

    // Per-entry complete cost computation
    let mut total_entries: u64 = 0;
    let mut total_tokens: u64 = 0;
    let mut total_llm_duration: u64 = 0;
    let mut total_exec_duration: u64 = 0;
    let mut total_commands: u64 = 0;
    let mut count_requests: u64 = 0;
    let mut cache_hits: u64 = 0;
    let mut cache_misses: u64 = 0;
    let mut per_entry_breakdowns: Vec<terio::accounting::CostBreakdown> = Vec::new();

    for entry in &entries {
        let counters = &entry.cost_counters;
        total_tokens += counters.llm_cost.tokens;
        total_llm_duration += counters.llm_cost.duration_ms;
        total_exec_duration += counters.execution_cost.duration_ms;
        total_commands += counters.execution_cost.commands_executed;
        if counters.cache_cost.hit {
            cache_hits += 1;
        } else {
            cache_misses += 1;
        }
        if entry.request.is_some() {
            count_requests += 1;
        }
        // Per-entry complete cost: compute per-entry instead of mixing global + per-entry risk
        let entry_risk = entry
            .risk
            .as_ref()
            .unwrap_or(&terio::types::RiskLevel::ReadOnly);
        let entry_cost = terio::accounting::compute_total_cost(
            counters,
            entry_risk,
            counters.llm_cost.duration_ms,
            &config.cost,
        );
        per_entry_breakdowns.push(entry_cost);
        total_entries += 1;
    }

    // Aggregate per-entry costs
    let mut total_llm = 0.0;
    let mut total_attention = 0.0;
    let mut total_risk = 0.0;
    let mut total_all = 0.0;
    for b in &per_entry_breakdowns {
        total_llm += b.llm_cost;
        total_attention += b.attention_cost;
        total_risk += b.risk_cost;
        total_all += b.total;
    }

    println!("=== terio cost report ===");
    println!("Total entries:         {}", total_entries);
    println!("Requests:              {}", count_requests);
    println!("LLM tokens:            {}", total_tokens);
    println!("LLM duration (ms):     {}", total_llm_duration);
    println!("Exec duration (ms):    {}", total_exec_duration);
    println!("Commands run:          {}", total_commands);
    println!("Cache hits:            {}", cache_hits);
    println!("Cache misses:          {}", cache_misses);
    println!("  LLM tokens:           ${:.6}", total_llm);
    println!("  Attention:            ${:.6}", total_attention);
    println!("  Risk:                 ${:.6}", total_risk);
    println!("  ─────────────────");
    println!("  Total C_total:        ${:.6}", total_all);
    println!();
    println!("Estimated savings from scripting:");
    if let Ok(synonyms) = terio::synonym::SynonymIndex::load_default() {
        for (query, entry) in synonyms.entries() {
            let savings =
                terio::accounting::estimated_savings(query, &entry.script_id, &config.cost);
            println!("  '{}' -> ${:.6}", query, savings);
        }
    }
    Ok(())
}

fn handle_registry(cmd: RegistryCmd) -> anyhow::Result<()> {
    match cmd {
        RegistryCmd::Search { query } => {
            let results = terio::registry::search_registry(&query).unwrap_or_default();
            if results.is_empty() {
                eprintln!("terio: no scripts found matching '{}'", query);
                return Ok(());
            }
            println!("=== terio registry search: '{}' ===", query);
            for script in &results {
                let tags = if script.tags.is_empty() {
                    String::new()
                } else {
                    format!(" [{}]", script.tags.join(", "))
                };
                println!(
                    "  {:20} {} (v{}) by {}{}",
                    script.id,
                    script.description,
                    script.version,
                    script.author.as_deref().unwrap_or("unknown"),
                    tags,
                );
            }
        }
        RegistryCmd::Install { id } => {
            // Confirm before installing
            match terio::registry::inspect_script(&id) {
                Ok(meta) => {
                    let caps = if meta.capabilities.is_empty() {
                        "none".to_string()
                    } else {
                        meta.capabilities.join(", ")
                    };
                    eprintln!("terio: installing '{}' from registry:", meta.id);
                    eprintln!("  Description: {}", meta.description);
                    eprintln!(
                        "  Author:      {}",
                        meta.author.as_deref().unwrap_or("unknown")
                    );
                    eprintln!("  Risk:        {}", meta.risk);
                    eprintln!("  Capabilities: {}", caps);
                    eprint!("Proceed? [y/N] ");
                    use std::io::Read;
                    let mut input = String::new();
                    std::io::stdin().read_to_string(&mut input)?;
                    if !input.trim().eq_ignore_ascii_case("y") {
                        eprintln!("terio: installation cancelled.");
                        return Ok(());
                    }
                }
                Err(e) => {
                    eprintln!("terio: {} — can't install", e);
                    return Ok(());
                }
            }
            match terio::registry::download_and_install(
                &id,
                false,
                None::<&terio::registry::ConfirmCallback>,
            ) {
                Ok(path) => {
                    eprintln!("terio: script '{}' installed from registry", id);
                    eprintln!("terio:   -> {}", path);
                }
                Err(e) => {
                    eprintln!("terio: failed to install '{}': {}", id, e);
                }
            }
        }
        RegistryCmd::Publish { id, api_key } => {
            // Load the script from local filesystem
            let dirs = script_engine::default_script_dirs()?;
            let backend = Arc::new(CliApiBackend);
            let mut engine = ScriptEngine::new(backend);
            engine.load_all(&dirs)?;
            let script = engine
                .find_script(&id)
                .ok_or_else(|| anyhow::anyhow!("script '{}' not found locally", id))?
                .clone();

            // Read script content from ScriptSource
            let content = match &script.source {
                terio::script_engine::ScriptSource::Rhai(c) => c.clone(),
                terio::script_engine::ScriptSource::Toml(c) => c.clone(),
            };

            let key_info = if let Some(_key) = api_key {
                // In a real implementation, would POST to registry API
                " (API key provided)"
            } else {
                " (no API key — dry run)"
            };

            // Build capabilities from script triggers + risk heuristic
            let capabilities: Vec<String> = vec![]; // TODO: derive from script analysis

            let meta = terio::registry::prepare_publish(
                &script.id,
                &content,
                &script.id,
                &script.description,
                script.triggers.clone(),
                None,
                None,
                capabilities,
            )?;

            println!("=== terio publish preview ===");
            println!("ID:          {}", meta.id);
            println!("Name:        {}", meta.name);
            println!("Description: {}", meta.description);
            println!("Tags:        {}", meta.tags.join(", "));
            println!(
                "SHA-256:     {}",
                meta.sha256.as_deref().unwrap_or("(none)")
            );
            println!("Risk:        {}", meta.risk);
            println!("{}", key_info);
            eprintln!("terio: publish preview generated. Use --api-key to submit.");
        }
        RegistryCmd::Inspect { id } => match terio::registry::inspect_script(&id) {
            Ok(meta) => {
                let caps = if meta.capabilities.is_empty() {
                    "none".to_string()
                } else {
                    meta.capabilities.join(", ")
                };
                println!("=== terio registry: {} ===", meta.id);
                println!("  Name:        {}", meta.name);
                println!("  Description: {}", meta.description);
                println!(
                    "  Author:      {}",
                    meta.author.as_deref().unwrap_or("unknown")
                );
                println!("  Version:     {}", meta.version);
                println!("  Risk:        {}", meta.risk);
                println!("  Capabilities: {}", caps);
                println!("  Tags:        {}", meta.tags.join(", "));
                println!(
                    "  SHA-256:     {}",
                    meta.sha256.as_deref().unwrap_or("(none)")
                );
            }
            Err(e) => {
                eprintln!("terio: {}", e);
            }
        },
    }
    Ok(())
}

fn handle_ask(
    identity: &Identity,
    log_dir: &std::path::Path,
    request: &str,
    yes: bool,
) -> anyhow::Result<()> {
    let config = Config::load().unwrap_or_default();
    let cache = ScriptCache::new()?;

    // Phase 5: Route optimizer — log decision in debug mode
    if config.attention_mode == terio::config::AttentionMode::Debug {
        // Check if there's a synonym for this request
        let synonym_id = terio::synonym::SynonymIndex::load_default()
            .ok()
            .and_then(|s| {
                // Clone out of borrowed context before s is dropped
                s.lookup(request).map(|r| r.entry.script_id.clone())
            });

        // Check if there's a matching script
        let script_id = {
            if let Ok(dirs) = script_engine::default_script_dirs() {
                let backend = Arc::new(CliApiBackend);
                let mut engine = ScriptEngine::new(backend);
                if engine.load_all(&dirs).is_ok() {
                    engine.match_input(request).map(|(s, _)| s.id.clone())
                } else {
                    None
                }
            } else {
                None
            }
        };

        let advice = terio::accounting::optimize_route(
            request,
            synonym_id.as_deref(),
            None,
            &terio::types::RiskLevel::ReadOnly,
            script_id.as_deref(),
            &config,
        );
        eprintln!(
            "[route] {} ({})",
            advice.reason,
            match &advice.decision {
                terio::accounting::RouteDecision::Script(id) => format!("script:{}", id),
                terio::accounting::RouteDecision::Llm => "LLM".to_string(),
            }
        );
    }
    let writer = Box::new(JsonlLogWriter::new(log_dir)?);
    let reader = Box::new(JsonlLogReader::new(log_dir));
    let store = LogStore::new(writer, reader, 256);
    let provider = create_provider(&config.provider);
    let result = ask::process_request(request, identity, &store, &cache, &*provider, yes)?;
    // Record synonym from successful LLM response (CacheHit or FromAgent)
    if matches!(
        result,
        AskResult::CacheHit { .. } | AskResult::FromAgent { .. }
    ) {
        let script_id = match &result {
            AskResult::CacheHit { entry, .. } => Some(entry.normalized_request.clone()),
            AskResult::FromAgent { plan, .. } => Some(plan.summary.clone()),
            _ => None,
        };
        if let Some(id) = script_id {
            if let Ok(mut synonyms) = terio::synonym::SynonymIndex::load_default() {
                synonyms.add(request, &id);
                let _ = synonyms.save();
            }
        }
    }
    render_ask_result(result, &store, request)?;
    store.flush()?;

    // Phase 4: Proactive prediction
    ProactiveEngine::record_last_request(request)?;
    // Recursion guard: prevent cascading auto-executions (depth via env var)
    let depth: u32 = std::env::var("TERIO_PROACTIVE_DEPTH")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    if depth > config.proactive.max_auto_exec_depth {
        return Ok(()); // max depth reached, stop chaining
    }
    if let Ok(engine) = ProactiveEngine::load_from_log(log_dir, 200) {
        if let Some(pred) = engine.predict(request) {
            // Check if auto-execution is configured AND safe
            let can_auto = config.proactive.auto_execute
                && config.attention_mode == terio::config::AttentionMode::Quiet
                && pred.confidence > 0.95
                && depth < config.proactive.max_auto_exec_depth;
            if can_auto {
                // Only auto-execute if prediction is read-only safe
                // (actual risk check happens inside handle_ask / process_request)
                std::env::set_var("TERIO_PROACTIVE_DEPTH", (depth + 1).to_string());
                let new_count = ProactiveEngine::increment_auto_executed()?;
                let _ = handle_ask(identity, log_dir, &pred.request, true);
                eprintln!("terio: +{} auto-executed ('{}')", new_count, pred.request);
            } else if pred.confidence > 0.5 {
                // Show suggestion
                eprintln!("# terio: {}? [наберите команду]", pred.request);
                ProactiveEngine::save_prediction(&pred)?;
            }
        }
        let count = engine.auto_executed_count();
        if count > 0 {
            eprintln!("terio: +{} command(s) auto-executed this session", count);
        }
    }

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

fn handle_integrations() -> anyhow::Result<()> {
    let mgr = terio::integration::IntegrationManager::new()?;
    terio::integration::print_integration_status(&mgr);
    Ok(())
}

fn handle_forget(program: &str) -> anyhow::Result<()> {
    let mut mgr = terio::integration::IntegrationManager::new()?;
    mgr.forget_program(program)?;
    eprintln!("terio: forgot '{}'", program);
    Ok(())
}

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
        None => println!("{}", json),
    }
    Ok(())
}

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

// ---------------------------------------------------------------------------
// Phase 0: new handlers
// ---------------------------------------------------------------------------

fn print_help() {
    println!("terio — интегратор интерфейсов");
    println!();
    println!("Использование: terio [КОМАНДА]");
    println!();
    println!("Команды:");
    println!("  ask <запрос>        запрос на естественном языке");
    println!("  run -- <команда>    выполнить shell-команду");
    println!("  log [--json]        показать лог");
    println!("  stats               метрики и cost_counters");
    println!("  confirm             подтвердить ожидающий план");
    println!("  undo                откатить последнюю операцию");
    println!("  redo                повторить отменённую операцию");
    println!("  cancel              отменить текущую операцию");
    println!("  config show         показать настройки");
    println!("  config set <key> <val>  установить настройку");
    println!("  learn <program>     обучить интеграцию");
    println!("  integrations        статус интеграций");
    println!("  forget <program>    забыть интеграцию");
    println!("  share [file]        экспорт окна");
    println!("  receive <file>      импорт окна");
    println!("  mode <mode>         режим внимания: quiet | normal | debug");
    println!("  focus <up|down>     переключить окно вывода (UI)");
    println!("  scroll <N>          прокрутить окна (UI)");
    println!("  repeat              повторить последний запрос");
    println!("  help                эта справка");
    println!("  ui                  открыть UI (по умолчанию)");
    println!();
    println!("Подробнее: https://github.com/bortoq/terio");
}

fn handle_mode(mode: &str) -> anyhow::Result<()> {
    let mut config = Config::load().unwrap_or_default();
    match mode {
        "quiet" | "normal" | "debug" => {
            config.set("attention_mode", mode)?;
            config.save()?;
            eprintln!("terio: режим внимания: {}", mode);
        }
        other => {
            anyhow::bail!("неизвестный режим: {other}. Используйте: quiet, normal, debug");
        }
    }
    Ok(())
}

fn handle_focus(direction: &str) {
    match direction {
        "up" | "down" | "↑" | "↓" => {
            // В CLI-режиме фокус не имеет смысла — направляем в UI
            eprintln!("terio: переключение фокуса работает в UI. Запустите terio без аргументов.");
        }
        _ => {
            eprintln!("terio: используйте focus up или focus down");
        }
    }
}

fn handle_scroll(lines: i32) {
    if lines != 0 {
        eprintln!("terio: скролл работает в UI. Запустите terio без аргументов.");
    }
}

fn handle_repeat(identity: &Identity, log_dir: &std::path::Path) -> anyhow::Result<()> {
    // Загружаем последние записи и ищем последний user request
    let reader = JsonlLogReader::new(log_dir);
    let entries = reader.recent(50)?;
    let last_req: Option<String> = entries.iter().rev().find_map(|e| e.request.clone());

    match last_req {
        Some(ref request) => {
            eprintln!("terio: повтор запроса: \"{}\"", request);
            handle_ask(identity, log_dir, request, false)?;
        }
        None => {
            eprintln!("terio: нет предыдущих запросов для повторения.");
        }
    }
    Ok(())
}

/// Проверить синоним: если запрос найден в SynonymIndex, выполнить скрипт.
/// Returns Some(true) если синоним сработал, Some(false) если индекс не загрузился, None если нет совпадения.
fn try_synonym_command(query: &str) -> anyhow::Result<Option<bool>> {
    let synonyms = match terio::synonym::SynonymIndex::load_default() {
        Ok(s) => s,
        Err(_) => return Ok(Some(false)),
    };
    match synonyms.lookup(query) {
        Some(lookup_result) => {
            let entry = lookup_result.entry;
            // Validate synonym target: check if script exists
            let dirs = script_engine::default_script_dirs()?;
            let backend = std::sync::Arc::new(CliApiBackend);
            let mut engine = ScriptEngine::new(backend);
            if engine.load_all(&dirs).is_err() {
                return Ok(Some(false));
            }
            if !engine.has_script(&entry.script_id) {
                // Script not found — cannot use this synonym
                eprintln!(
                    "terio: synonym points to missing script '{}'",
                    entry.script_id
                );
                return Ok(None);
            }
            // For prefix/bag matches on non-read-only, require confirmation
            // (we don't have the script's risk level here, so we check match kind)
            if lookup_result.match_kind != terio::synonym::MatchKind::Exact {
                eprintln!(
                    "terio: fuzzy synonym match '{}', need confirmation. Run: terio ask '{}'",
                    lookup_result.match_kind.name(),
                    query
                );
                return Ok(None);
            }
            match engine.run_script_by_id(&entry.script_id, vec![]) {
                Ok(output) => {
                    if !output.is_empty() {
                        println!("{output}");
                    }
                    // Update frequency
                    if let Ok(mut syn) = terio::synonym::SynonymIndex::load_default() {
                        syn.add(query, &entry.script_id);
                        let _ = syn.save();
                    }
                    Ok(Some(true))
                }
                Err(_) => Ok(Some(false)),
            }
        }
        None => Ok(None),
    }
}

fn handle_alias(cmd: AliasCmd) -> anyhow::Result<()> {
    match cmd {
        AliasCmd::List => {
            let synonyms = terio::synonym::SynonymIndex::load_default()?;
            if synonyms.is_empty() {
                println!("(синонимов нет)");
            } else {
                println!("=== terio aliases ===");
                let header = format!(
                    "{:40} {:30} {:>8} {}",
                    "Normalized", "Script ID", "Freq", "Last used"
                );
                println!("{header}");
                println!("{}", "-".repeat(95));
                for (norm, entry) in synonyms.entries() {
                    let last = &entry.last_used[..19.min(entry.last_used.len())];
                    println!(
                        "{:40} {:30} {:>8} {}",
                        norm, entry.script_id, entry.frequency, last
                    );
                }
            }
        }
        AliasCmd::Remove { query } => {
            let mut synonyms = terio::synonym::SynonymIndex::load_default()?;
            if synonyms.remove(&query) {
                synonyms.save()?;
                eprintln!("terio: синоним '{}' удалён.", query);
            } else {
                eprintln!("terio: синоним '{}' не найден.", query);
            }
        }
    }
    Ok(())
}

/// Создать ScriptEngine с default backend и загрузить все скрипты.
fn create_script_engine() -> anyhow::Result<script_engine::ScriptEngine> {
    let dirs = script_engine::default_script_dirs()?;
    ScriptEngine::ensure_dirs(&dirs)?;
    let backend = std::sync::Arc::new(CliApiBackend);
    let mut engine = ScriptEngine::new(backend);
    engine.load_all(&dirs)?;
    Ok(engine)
}

/// Попробовать обработать команду через скрипты.
/// Возвращает Some(true) если скрипт выполнен, Some(false) если скрипты не загрузились,
/// None если нет подходящего скрипта.
fn try_script_command(input: &str) -> anyhow::Result<Option<bool>> {
    let mut engine = match create_script_engine() {
        Ok(e) => e,
        Err(_) => return Ok(Some(false)),
    };
    // Clone script and args to avoid borrow conflict with execute_script
    let matched: Option<(script_engine::Script, Vec<String>)> =
        engine.match_input(input).map(|(s, a)| (s.clone(), a));
    match matched {
        Some((script, args)) => {
            let output = engine.execute_script(&script, args)?;
            if !output.is_empty() {
                println!("{output}");
            }
            Ok(Some(true))
        }
        None => Ok(None),
    }
}

fn handle_script(cmd: ScriptCmd) -> anyhow::Result<()> {
    let dirs = script_engine::default_script_dirs()?;
    ScriptEngine::ensure_dirs(&dirs)?;
    let backend = Arc::new(CliApiBackend);

    match cmd {
        ScriptCmd::List => {
            let mut engine = ScriptEngine::new(backend);
            engine.load_all(&dirs)?;
            println!("=== terio scripts ===");
            for script in engine.scripts() {
                let kind = match script.kind {
                    script_engine::ScriptKind::Builtin => "builtin",
                    script_engine::ScriptKind::Core => "core",
                    script_engine::ScriptKind::User => "user",
                    script_engine::ScriptKind::Learned => "learned",
                };
                let triggers = script.triggers.join(", ");
                println!(
                    "  {:12} {:12} {}  [{}]",
                    script.id, kind, script.description, triggers
                );
            }
        }
        ScriptCmd::Install { path } => {
            let id = ScriptEngine::install_script(std::path::Path::new(&path), &dirs)?;
            eprintln!("terio: script '{id}' installed to ~/.terio/scripts/user/");
        }
        ScriptCmd::Run { id, args } => {
            let mut engine = ScriptEngine::new(backend);
            engine.load_all(&dirs)?;
            let output = engine.run_script_by_id(&id, args)?;
            if !output.is_empty() {
                println!("{output}");
            }
        }
        ScriptCmd::Export { id, output } => {
            handle_script_export(&dirs, &id, output.as_deref())?;
        }
        ScriptCmd::Import { input } => {
            handle_script_import(&dirs, &input)?;
        }
    }
    Ok(())
}

fn handle_script_export(
    dirs: &script_engine::ScriptDirs,
    id: &str,
    output: Option<&str>,
) -> anyhow::Result<()> {
    let backend = Arc::new(CliApiBackend);
    let mut engine = ScriptEngine::new(backend);
    engine.load_all(dirs)?;

    // Collect scripts to export
    let scripts: Vec<terio::script_engine::Script> = if id == "all" {
        engine.scripts().to_vec()
    } else {
        let s = engine
            .find_script(id)
            .ok_or_else(|| anyhow::anyhow!("script '{}' not found", id))?;
        vec![s.clone()]
    };

    // Build export format: id + content
    #[derive(serde::Serialize)]
    struct ExportItem {
        id: String,
        content: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        source: Option<String>,
    }

    let items: Vec<ExportItem> = scripts
        .iter()
        .map(|s| {
            let content = match &s.source {
                terio::script_engine::ScriptSource::Rhai(c) => Some(c.clone()),
                terio::script_engine::ScriptSource::Toml(c) => Some(c.clone()),
            };
            ExportItem {
                id: s.id.clone(),
                content,
                source: None,
            }
        })
        .collect();

    let json =
        serde_json::to_string_pretty(&items).with_context(|| "failed to serialize scripts")?;

    match output {
        Some(path) => {
            std::fs::write(path, &json).with_context(|| format!("failed to write to {}", path))?;
            eprintln!("terio: exported {} script(s) to {}", items.len(), path);
        }
        None => println!("{}", json),
    }
    Ok(())
}

fn handle_script_import(dirs: &script_engine::ScriptDirs, input: &str) -> anyhow::Result<()> {
    let json_data = if input == "-" {
        use std::io::Read;
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        buf
    } else {
        std::fs::read_to_string(input)
            .map_err(|e| anyhow::anyhow!("failed to read {}: {}", input, e))?
    };

    #[derive(serde::Deserialize)]
    struct ExportScript {
        id: String,
        #[serde(default)]
        content: Option<String>,
        #[serde(default)]
        source: Option<String>,
    }

    let imported: Vec<ExportScript> =
        serde_json::from_str(&json_data).with_context(|| "invalid script export JSON")?;

    let user_dir = &dirs.user;

    let mut count = 0;
    for entry in &imported {
        let script_content = entry
            .content
            .as_deref()
            .or(entry.source.as_deref())
            .ok_or_else(|| anyhow::anyhow!("script '{}' has no content", entry.id))?;

        let path = user_dir.join(format!("{}.rhai", entry.id));
        std::fs::write(&path, script_content)
            .with_context(|| format!("failed to write {}", path.display()))?;
        count += 1;
    }

    eprintln!("terio: imported {} script(s)", count);
    Ok(())
}

fn handle_sandbox(cmd: SandboxCmd) -> anyhow::Result<()> {
    match cmd {
        SandboxCmd::Status => {
            let config = Config::load().unwrap_or_default();
            println!("=== terio sandbox status ===");
            println!(
                "Read isolation:      {}",
                if config.sandbox.read_isolation {
                    "strict (empty rootfs + bind mounts)"
                } else {
                    "legacy (--ro-bind / /)"
                }
            );
            println!(
                "Bubblewrap binary:   {}",
                undo::find_bwrap_binary()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "NOT FOUND".to_string())
            );
            let no_read = if config.sandbox.no_read_paths.is_empty() {
                "none".to_string()
            } else {
                config.sandbox.no_read_paths.join(", ")
            };
            println!("No-read paths:       {}", no_read);
            println!("Auto-trust:          read_only={} success, local_write={} success, destructive=never",
                config.auto_trust.read_only, config.auto_trust.local_write);
            println!("Undo mode:           {:?}", config.undo.mode);
            println!("Undo enabled:        {}", config.undo.experimental_enabled);
            println!("Attention mode:      {:?}", config.attention_mode);
            println!("Last undo status:    {:?}", undo::latest_status().ok());
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

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
        UiCommand::Focus(direction) => {
            append_system_event(identity, store, &format!("focus {direction}"))?;
        }
        UiCommand::Scroll(lines) => {
            append_system_event(identity, store, &format!("scroll {lines}"))?;
        }
        UiCommand::Help => {
            // Help handled directly in UI layer, but if it arrives here, log it
            append_system_event(identity, store, "help requested")?;
        }
        UiCommand::Mode(mode) => {
            let mut config = Config::load().unwrap_or_default();
            match config.set("attention_mode", &mode) {
                Ok(_) => {
                    config.save()?;
                    append_system_event(identity, store, &format!("attention mode: {mode}"))?;
                }
                Err(e) => {
                    append_system_event(identity, store, &format!("mode error: {e}"))?;
                }
            }
        }
        UiCommand::Repeat => {
            let entries = store.recent(50)?;
            let last_req = entries.iter().rev().find_map(|e| e.request.clone());
            if let Some(request) = last_req {
                append_system_event(identity, store, &format!("repeat: {request}"))?;
                let config = Config::load().unwrap_or_default();
                let cache = ScriptCache::new()?;
                let provider = create_provider(&config.provider);
                if let AskResult::PendingConfirmation {
                    plan_hash,
                    source,
                    plan_summary,
                    execution,
                } = ask::process_request(&request, identity, store, &cache, &*provider, false)?
                {
                    ask::save_pending_confirmation(
                        &ask::PendingConfirmationState {
                            plan_hash,
                            source,
                            plan_summary: plan_summary.clone(),
                        },
                        &execution,
                    )?;
                }
            } else {
                append_system_event(identity, store, "no requests to repeat")?;
            }
        }
    }
    store.flush()?;
    Ok(())
}
