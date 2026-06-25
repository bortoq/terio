use crate::cache::CachedStep;
use crate::config::{Config, UndoMode};
use crate::types::RiskLevel;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum UndoState {
    Applied,
    Undone,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoPathEntry {
    pub relative_path: String,
    pub existed_before: bool,
    pub existed_after: bool,
    pub is_dir_before: bool,
    pub is_dir_after: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoRecord {
    pub schema_version: u32,
    pub operation_id: String,
    pub created_at: String,
    pub cwd: String,
    pub request: String,
    pub summary: String,
    pub mode: UndoMode,
    pub state: UndoState,
    pub sandboxed: bool,
    pub warnings: Vec<String>,
    pub paths: Vec<UndoPathEntry>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct UndoStatus {
    pub can_undo: bool,
    pub can_redo: bool,
    pub summary: Option<String>,
}

pub struct UndoSession {
    record: UndoRecord,
    op_dir: PathBuf,
    cwd: PathBuf,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WrappedCommand {
    pub argv: Vec<String>,
    pub sandboxed: bool,
    pub warning: Option<String>,
}

pub fn should_track(config: &Config, risk: &RiskLevel) -> bool {
    config.undo.experimental_enabled
        && matches!(
            risk,
            RiskLevel::LocalWrite | RiskLevel::Destructive | RiskLevel::NetworkWrite
        )
}

pub fn start_session(
    config: &Config,
    request: &str,
    summary: &str,
    steps: &[CachedStep],
    cwd: &Path,
    risk: &RiskLevel,
) -> Result<Option<UndoSession>> {
    if !should_track(config, risk) {
        return Ok(None);
    }

    let candidate_paths = discover_candidate_paths(steps, cwd);
    let operation_id = crate::identity::Identity::new_interaction_id();
    let op_dir = undo_base_dir()?.join(&operation_id);
    std::fs::create_dir_all(&op_dir)?;

    let mut path_entries = Vec::new();
    for relative in &candidate_paths {
        let absolute = cwd.join(relative);
        path_entries.push(UndoPathEntry {
            relative_path: relative.to_string_lossy().to_string(),
            existed_before: absolute.exists(),
            existed_after: false,
            is_dir_before: absolute.is_dir(),
            is_dir_after: false,
        });
    }

    snapshot_paths(&op_dir.join("before"), cwd, &path_entries, true)?;

    let record = UndoRecord {
        schema_version: 1,
        operation_id,
        created_at: chrono::Utc::now().to_rfc3339(),
        cwd: cwd.to_string_lossy().to_string(),
        request: request.to_string(),
        summary: summary.to_string(),
        mode: config.undo.mode.clone(),
        state: UndoState::Applied,
        sandboxed: false,
        warnings: Vec::new(),
        paths: path_entries,
    };

    Ok(Some(UndoSession {
        record,
        op_dir,
        cwd: cwd.to_path_buf(),
    }))
}

impl UndoSession {
    pub fn wrap_command(&mut self, argv: &[String]) -> WrappedCommand {
        let wrapped = wrap_command_for_mode(
            &self.record.mode,
            argv,
            &self.cwd,
            &home_dir().unwrap_or_else(|_| self.cwd.clone()),
            find_bwrap_binary(),
        );
        if wrapped.sandboxed {
            self.record.sandboxed = true;
        }
        if let Some(warning) = &wrapped.warning {
            if !self.record.warnings.iter().any(|w| w == warning) {
                self.record.warnings.push(warning.clone());
            }
        }
        wrapped
    }

    pub fn finalize_success(mut self) -> Result<UndoRecord> {
        for path in &mut self.record.paths {
            let absolute = self.cwd.join(&path.relative_path);
            path.existed_after = absolute.exists();
            path.is_dir_after = absolute.is_dir();
        }
        snapshot_paths(
            &self.op_dir.join("after"),
            &self.cwd,
            &self.record.paths,
            false,
        )?;
        write_record(&self.op_dir.join("record.json"), &self.record)?;
        write_record(&undo_base_dir()?.join("latest.json"), &self.record)?;
        Ok(self.record)
    }
}

pub fn latest_status() -> Result<UndoStatus> {
    let Some(record) = load_latest_record()? else {
        return Ok(UndoStatus::default());
    };
    Ok(match record.state {
        UndoState::Applied => UndoStatus {
            can_undo: true,
            can_redo: false,
            summary: Some(record.summary),
        },
        UndoState::Undone => UndoStatus {
            can_undo: false,
            can_redo: true,
            summary: Some(record.summary),
        },
    })
}

pub fn undo_latest() -> Result<Option<UndoRecord>> {
    let Some(mut record) = load_latest_record()? else {
        return Ok(None);
    };
    if record.state != UndoState::Applied {
        return Ok(None);
    }
    let op_dir = undo_base_dir()?.join(&record.operation_id);
    let cwd = PathBuf::from(&record.cwd);
    restore_snapshot(&op_dir.join("before"), &cwd, &record.paths, true)?;
    record.state = UndoState::Undone;
    write_record(&op_dir.join("record.json"), &record)?;
    write_record(&undo_base_dir()?.join("latest.json"), &record)?;
    Ok(Some(record))
}

pub fn redo_latest() -> Result<Option<UndoRecord>> {
    let Some(mut record) = load_latest_record()? else {
        return Ok(None);
    };
    if record.state != UndoState::Undone {
        return Ok(None);
    }
    let op_dir = undo_base_dir()?.join(&record.operation_id);
    let cwd = PathBuf::from(&record.cwd);
    restore_snapshot(&op_dir.join("after"), &cwd, &record.paths, false)?;
    record.state = UndoState::Applied;
    write_record(&op_dir.join("record.json"), &record)?;
    write_record(&undo_base_dir()?.join("latest.json"), &record)?;
    Ok(Some(record))
}

pub fn direct_run_warning() -> &'static str {
    "terio: undo/redo не поддерживаются для `terio run -- ...`; используйте `terio ask` для snapshot-backed execution."
}

fn load_latest_record() -> Result<Option<UndoRecord>> {
    let path = undo_base_dir()?.join("latest.json");
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&path)?;
    let record = serde_json::from_str(&content)?;
    Ok(Some(record))
}

fn undo_base_dir() -> Result<PathBuf> {
    let home = home_dir()?;
    let dir = home.join(".terio").join("undo");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn home_dir() -> Result<PathBuf> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .context("HOME not set")?;
    Ok(PathBuf::from(home))
}

fn snapshot_paths(
    snapshot_dir: &Path,
    cwd: &Path,
    paths: &[UndoPathEntry],
    before_phase: bool,
) -> Result<()> {
    std::fs::create_dir_all(snapshot_dir)?;
    for entry in paths {
        let existed = if before_phase {
            entry.existed_before
        } else {
            entry.existed_after
        };
        if !existed {
            continue;
        }
        let rel = PathBuf::from(&entry.relative_path);
        let src = cwd.join(&rel);
        let dst = snapshot_dir.join("root").join(&rel);
        copy_path(&src, &dst)?;
    }
    Ok(())
}

fn restore_snapshot(
    snapshot_dir: &Path,
    cwd: &Path,
    paths: &[UndoPathEntry],
    before_phase: bool,
) -> Result<()> {
    for entry in paths {
        let rel = PathBuf::from(&entry.relative_path);
        let dst = cwd.join(&rel);
        if dst.exists() {
            remove_path(&dst)?;
        }
        let existed = if before_phase {
            entry.existed_before
        } else {
            entry.existed_after
        };
        if existed {
            let src = snapshot_dir.join("root").join(&rel);
            copy_path(&src, &dst)?;
        }
    }
    Ok(())
}

fn copy_path(src: &Path, dst: &Path) -> Result<()> {
    if src.is_dir() {
        std::fs::create_dir_all(dst)?;
        for child in std::fs::read_dir(src)? {
            let child = child?;
            copy_path(&child.path(), &dst.join(child.file_name()))?;
        }
        return Ok(());
    }
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(src, dst)?;
    Ok(())
}

fn remove_path(path: &Path) -> Result<()> {
    if path.is_dir() {
        std::fs::remove_dir_all(path)?;
    } else if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

fn write_record(path: &Path, record: &UndoRecord) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(record)?;
    std::fs::write(path, json)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

pub fn discover_candidate_paths(steps: &[CachedStep], cwd: &Path) -> Vec<PathBuf> {
    let mut paths = BTreeSet::new();
    for step in steps {
        for candidate in candidate_paths_for_step(step, cwd) {
            paths.insert(candidate);
        }
    }
    paths.into_iter().collect()
}

fn candidate_paths_for_step(step: &CachedStep, cwd: &Path) -> Vec<PathBuf> {
    let args = &step.argv[1..];
    let candidates: Vec<String> = match step.command.as_str() {
        "touch" | "mkdir" | "rm" => args
            .iter()
            .filter(|arg| !arg.starts_with('-'))
            .cloned()
            .collect(),
        "cp" | "mv" | "ln" => args
            .iter()
            .filter(|arg| !arg.starts_with('-'))
            .cloned()
            .collect(),
        "chmod" => args
            .iter()
            .skip_while(|arg| !arg.starts_with('.') && arg.chars().all(|c| c.is_ascii_digit()))
            .filter(|arg| !arg.starts_with('-'))
            .cloned()
            .collect(),
        "git" => args
            .iter()
            .skip(1)
            .filter(|arg| !arg.starts_with('-'))
            .cloned()
            .collect(),
        _ => args
            .iter()
            .filter(|arg| !arg.starts_with('-'))
            .filter(|arg| !matches!(arg.as_str(), "." | ".."))
            .cloned()
            .collect(),
    };

    candidates
        .into_iter()
        .filter_map(|arg| resolve_candidate_path(&arg, cwd))
        .collect()
}

fn resolve_candidate_path(arg: &str, cwd: &Path) -> Option<PathBuf> {
    if arg.contains('\0') || arg == "/" {
        return None;
    }
    let path = Path::new(arg);
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    };
    let parent = absolute.parent().unwrap_or(cwd);
    if !absolute.exists() && !parent.exists() {
        return None;
    }
    if !absolute.starts_with(cwd) {
        return None;
    }
    absolute.strip_prefix(cwd).ok().map(PathBuf::from)
}

pub fn wrap_command_for_mode(
    mode: &UndoMode,
    argv: &[String],
    cwd: &Path,
    home: &Path,
    bwrap_binary: Option<PathBuf>,
) -> WrappedCommand {
    // Load config for sandbox settings
    let config = crate::config::Config::load().unwrap_or_default();
    wrap_command_for_mode_impl(mode, argv, cwd, home, bwrap_binary, &config.sandbox)
}

fn wrap_command_for_mode_impl(
    mode: &UndoMode,
    argv: &[String],
    cwd: &Path,
    home: &Path,
    bwrap_binary: Option<PathBuf>,
    sandbox_config: &crate::config::SandboxConfig,
) -> WrappedCommand {
    match mode {
        UndoMode::Warn => WrappedCommand {
            argv: argv.to_vec(),
            sandboxed: false,
            warning: None,
        },
        UndoMode::Bubblewrap => {
            let Some(bwrap) = bwrap_binary else {
                return WrappedCommand {
                    argv: argv.to_vec(),
                    sandboxed: false,
                    warning: Some(
                        "terio: bubblewrap не найден, sandbox mode downgraded to warn.".to_string(),
                    ),
                };
            };
            let mut wrapped = vec![
                bwrap.to_string_lossy().to_string(),
                "--die-with-parent".to_string(),
                "--new-session".to_string(),
            ];

            if sandbox_config.read_isolation {
                // Строгая изоляция: пустой rootfs + точечные bind mounts
                // Без --share-net — сеть отключена
                wrapped.extend_from_slice(&[
                    "--ro-bind".to_string(),
                    "/bin".to_string(),
                    "/bin".to_string(),
                    "--ro-bind".to_string(),
                    "/usr".to_string(),
                    "/usr".to_string(),
                    "--ro-bind".to_string(),
                    "/lib".to_string(),
                    "/lib".to_string(),
                    "--ro-bind".to_string(),
                    "/lib64".to_string(),
                    "/lib64".to_string(),
                    "--ro-bind".to_string(),
                    "/libx32".to_string(),
                    "/libx32".to_string(),
                    "--ro-bind".to_string(),
                    "/etc/alternatives".to_string(),
                    "/etc/alternatives".to_string(),
                    "--dev".to_string(),
                    "/dev".to_string(),
                    "--proc".to_string(),
                    "/proc".to_string(),
                    "--tmpfs".to_string(),
                    "/tmp".to_string(),
                    "--bind".to_string(),
                    cwd.to_string_lossy().to_string(),
                    cwd.to_string_lossy().to_string(),
                    "--ro-bind".to_string(),
                    home.to_string_lossy().to_string(),
                    home.to_string_lossy().to_string(),
                ]);
                // Override no_read_paths with empty tmpfs
                for np in &sandbox_config.no_read_paths {
                    let resolved = resolve_no_read_path(np, home, cwd);
                    if let Some(path) = resolved {
                        wrapped.push("--tmpfs".to_string());
                        wrapped.push(path.to_string_lossy().to_string());
                    }
                }
            } else {
                // Legacy mode: full rootfs read-only + network
                wrapped.push("--share-net".to_string());
                wrapped.extend_from_slice(&[
                    "--ro-bind".to_string(),
                    "/".to_string(),
                    "/".to_string(),
                    "--dev".to_string(),
                    "/dev".to_string(),
                    "--proc".to_string(),
                    "/proc".to_string(),
                    "--tmpfs".to_string(),
                    "/tmp".to_string(),
                    "--bind".to_string(),
                    cwd.to_string_lossy().to_string(),
                    cwd.to_string_lossy().to_string(),
                    "--bind".to_string(),
                    home.to_string_lossy().to_string(),
                    home.to_string_lossy().to_string(),
                ]);
            }

            wrapped.push("--chdir".to_string());
            wrapped.push(cwd.to_string_lossy().to_string());
            wrapped.push("--".to_string());
            wrapped.extend(argv.iter().cloned());
            WrappedCommand {
                argv: wrapped,
                sandboxed: true,
                warning: None,
            }
        }
    }
}

/// Resolve a no_read_paths entry (supports ~/ and relative paths) to an absolute path.
fn resolve_no_read_path(path: &str, home: &Path, _cwd: &Path) -> Option<PathBuf> {
    let stripped = path.strip_prefix("~/").or_else(|| path.strip_prefix("~"))?;
    let p = if stripped.is_empty() {
        home.to_path_buf()
    } else {
        home.join(stripped)
    };
    // Only resolve if it actually exists
    if p.exists() {
        Some(p)
    } else {
        None
    }
}

pub fn find_bwrap_binary() -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join("bwrap");
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn step(command: &str, argv: &[&str], risk: RiskLevel) -> CachedStep {
        CachedStep {
            command: command.to_string(),
            argv: argv.iter().map(|arg| arg.to_string()).collect(),
            risk,
        }
    }

    #[test]
    fn test_discover_candidate_paths_for_local_write_commands() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("file.txt"), "x").unwrap();

        let paths = discover_candidate_paths(
            &[
                step("touch", &["touch", "file.txt"], RiskLevel::LocalWrite),
                step("mkdir", &["mkdir", "build"], RiskLevel::LocalWrite),
            ],
            dir.path(),
        );

        assert!(paths.contains(&PathBuf::from("file.txt")));
        assert!(paths.contains(&PathBuf::from("build")));
    }

    #[test]
    fn test_snapshot_roundtrip_undo_and_redo() {
        let _guard = crate::test_support::ENV_MUTEX.lock().unwrap();
        let prev_home = std::env::var("HOME").ok();
        let dir = TempDir::new().unwrap();
        std::env::set_var("HOME", dir.path());

        let cwd = dir.path().join("cwd");
        std::fs::create_dir_all(&cwd).unwrap();
        std::fs::write(cwd.join("note.txt"), "before").unwrap();

        let config = Config {
            undo: crate::config::UndoConfig {
                experimental_enabled: true,
                mode: UndoMode::Warn,
            },
            ..Config::default()
        };

        let mut session = start_session(
            &config,
            "update note",
            "Update note",
            &[step("touch", &["touch", "note.txt"], RiskLevel::LocalWrite)],
            &cwd,
            &RiskLevel::LocalWrite,
        )
        .unwrap()
        .unwrap();
        let wrapped = session.wrap_command(&["touch".into(), "note.txt".into()]);
        assert!(!wrapped.sandboxed);

        std::fs::write(cwd.join("note.txt"), "after").unwrap();
        session.finalize_success().unwrap();

        let status = latest_status().unwrap();
        assert!(status.can_undo);
        assert!(!status.can_redo);

        undo_latest().unwrap().unwrap();
        assert_eq!(
            std::fs::read_to_string(cwd.join("note.txt")).unwrap(),
            "before"
        );

        let status = latest_status().unwrap();
        assert!(!status.can_undo);
        assert!(status.can_redo);

        redo_latest().unwrap().unwrap();
        assert_eq!(
            std::fs::read_to_string(cwd.join("note.txt")).unwrap(),
            "after"
        );

        if let Some(prev) = prev_home {
            std::env::set_var("HOME", prev);
        } else {
            std::env::remove_var("HOME");
        }
    }

    #[test]
    fn test_bubblewrap_wrapper_uses_bwrap_when_available() {
        let sandbox_config = crate::config::SandboxConfig::default();
        let wrapped = wrap_command_for_mode_impl(
            &UndoMode::Bubblewrap,
            &["echo".into(), "hi".into()],
            Path::new("/tmp/project"),
            Path::new("/tmp/home"),
            Some(PathBuf::from("/usr/bin/bwrap")),
            &sandbox_config,
        );
        assert!(wrapped.sandboxed);
        assert_eq!(wrapped.argv[0], "/usr/bin/bwrap");
        // Legacy mode uses --ro-bind / /
        assert!(wrapped.argv.iter().any(|arg| arg == "--ro-bind"));
        assert_eq!(wrapped.argv.last().map(String::as_str), Some("hi"));
    }

    #[test]
    fn test_bubblewrap_wrapper_falls_back_to_warn_when_missing() {
        let sandbox_config = crate::config::SandboxConfig::default();
        let wrapped = wrap_command_for_mode_impl(
            &UndoMode::Bubblewrap,
            &["echo".into(), "hi".into()],
            Path::new("/tmp/project"),
            Path::new("/tmp/home"),
            None,
            &sandbox_config,
        );
        assert!(!wrapped.sandboxed);
        assert!(wrapped.warning.unwrap().contains("bubblewrap"));
        assert_eq!(wrapped.argv, vec!["echo".to_string(), "hi".to_string()]);
    }

    #[test]
    fn test_read_isolation_no_share_net() {
        let sandbox_config = crate::config::SandboxConfig {
            read_isolation: true,
            no_read_paths: vec![],
        };
        let wrapped = wrap_command_for_mode_impl(
            &UndoMode::Bubblewrap,
            &["echo".into(), "hi".into()],
            Path::new("/tmp/project"),
            Path::new("/tmp/home"),
            Some(PathBuf::from("/usr/bin/bwrap")),
            &sandbox_config,
        );
        assert!(wrapped.sandboxed);
        // No --share-net in strict mode
        assert!(!wrapped.argv.iter().any(|a| a == "--share-net"));
        // Has system bind mounts
        assert!(wrapped.argv.iter().any(|a| a == "/bin"));
        assert!(wrapped.argv.iter().any(|a| a == "/usr"));
    }

    #[test]
    fn test_read_isolation_with_no_read_paths() {
        use tempfile::TempDir;
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".ssh")).unwrap();
        std::fs::write(dir.path().join(".ssh/id_rsa"), "test").unwrap();

        let sandbox_config = crate::config::SandboxConfig {
            read_isolation: true,
            no_read_paths: vec!["~/.ssh".to_string()],
        };
        let wrapped = wrap_command_for_mode_impl(
            &UndoMode::Bubblewrap,
            &["echo".into(), "hi".into()],
            Path::new("/tmp/project"),
            dir.path(),
            Some(PathBuf::from("/usr/bin/bwrap")),
            &sandbox_config,
        );
        assert!(wrapped.sandboxed);
        // Should have a --tmpfs entry for .ssh (not just the standard /tmp)
        let tmpfs_indices: Vec<usize> = wrapped
            .argv
            .iter()
            .enumerate()
            .filter(|(_, a)| a.as_str() == "--tmpfs")
            .map(|(i, _)| i)
            .collect();
        assert!(
            tmpfs_indices.len() >= 2,
            "should have at least 2 --tmpfs entries (got {})",
            tmpfs_indices.len()
        );
        // At least one should cover .ssh (not just /tmp)
        let has_ssh_tmpfs = tmpfs_indices
            .iter()
            .any(|&i| i + 1 < wrapped.argv.len() && wrapped.argv[i + 1].contains(".ssh"));
        assert!(has_ssh_tmpfs, "should have --tmpfs covering .ssh");
    }
}
