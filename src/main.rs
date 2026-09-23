mod dashboard;
mod progress;

use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;
use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use dashboard::Dashboard;
use progress::ProgressView;

#[derive(Debug)]
struct Options {
    collection: String,
    root: PathBuf,
    state_dir: PathBuf,
    log_dir: PathBuf,
    forwarded: Vec<String>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct RunState {
    version: u8,
    collection: String,
    updated_at: u64,
    modules: BTreeMap<String, ModuleState>,
}

#[derive(Debug, Deserialize, Serialize)]
struct ModuleState {
    status: String,
    attempts: u32,
    updated_at: u64,
}

struct EventLog(File);

impl EventLog {
    fn emit(&mut self, event: serde_json::Value) -> io::Result<()> {
        serde_json::to_writer(&mut self.0, &event)?;
        self.0.write_all(b"\n")?;
        self.0.flush()
    }
}

fn usage() -> &'static str {
    "Usage: mistborn-bootstrap run COLLECTION [--root PATH] [--state-dir PATH] [--log-dir PATH] [--dry-run] [--yes] [--user NAME]\n       mistborn-bootstrap host --script PATH [COMMAND [ARGS...]]"
}

fn interactive_terminal() -> bool {
    io::stdin().is_terminal()
        && io::stdout().is_terminal()
        && env::var("TERM").is_ok_and(|term| term != "dumb")
        && env::var_os("NO_COLOR").is_none()
}

fn value(args: &[String], index: &mut usize, flag: &str) -> Result<String, String> {
    *index += 1;
    args.get(*index)
        .cloned()
        .ok_or_else(|| format!("{flag} requires a value"))
}

fn parse_args() -> Result<Options, String> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.len() < 2 || args[0] != "run" {
        return Err(usage().to_owned());
    }

    let collection = args[1].clone();
    if !collection
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '-' || character == '_')
    {
        return Err("collection contains unsupported characters".to_owned());
    }

    let mut root = env::current_dir().map_err(|error| error.to_string())?;
    let mut state_dir = PathBuf::from("/var/lib/mistborn-bootstrap");
    let mut log_dir = PathBuf::from("/var/log/mistborn-bootstrap");
    let mut forwarded = Vec::new();
    let mut index = 2;
    while index < args.len() {
        match args[index].as_str() {
            "--root" => root = PathBuf::from(value(&args, &mut index, "--root")?),
            "--state-dir" => state_dir = PathBuf::from(value(&args, &mut index, "--state-dir")?),
            "--log-dir" => log_dir = PathBuf::from(value(&args, &mut index, "--log-dir")?),
            "--user" => {
                forwarded.push("--user".to_owned());
                forwarded.push(value(&args, &mut index, "--user")?);
            }
            "--dry-run" | "--yes" => forwarded.push(args[index].clone()),
            "--help" | "-h" => return Err(usage().to_owned()),
            unknown => return Err(format!("unknown argument: {unknown}\n{}", usage())),
        }
        index += 1;
    }
    Ok(Options {
        collection,
        root,
        state_dir,
        log_dir,
        forwarded,
    })
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is before Unix epoch")
        .as_secs()
}

fn run_id() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is before Unix epoch")
        .as_nanos();
    format!("{timestamp}-{}", std::process::id())
}

fn read_modules(path: &Path) -> Result<Vec<String>, String> {
    let file =
        File::open(path).map_err(|error| format!("cannot open {}: {error}", path.display()))?;
    let mut modules = Vec::new();
    for line in BufReader::new(file).lines() {
        let line = line.map_err(|error| error.to_string())?;
        let module = line.trim();
        if !module.is_empty() && !module.starts_with('#') {
            modules.push(module.to_owned());
        }
    }
    Ok(modules)
}

fn load_state(path: &Path, collection: &str) -> Result<RunState, String> {
    if !path.exists() {
        return Ok(RunState {
            version: 1,
            collection: collection.to_owned(),
            ..RunState::default()
        });
    }
    let file = File::open(path).map_err(|error| error.to_string())?;
    let state: RunState =
        serde_json::from_reader(file).map_err(|error| format!("invalid state file: {error}"))?;
    if state.version != 1 || state.collection != collection {
        return Err("state file belongs to an incompatible run".to_owned());
    }
    Ok(state)
}

fn save_state(path: &Path, state: &RunState) -> Result<(), String> {
    let temporary = path.with_extension("json.tmp");
    let mut file = File::create(&temporary).map_err(|error| error.to_string())?;
    serde_json::to_writer_pretty(&mut file, state).map_err(|error| error.to_string())?;
    file.write_all(b"\n").map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())?;
    fs::rename(&temporary, path).map_err(|error| error.to_string())
}

fn execute(options: Options) -> Result<(), String> {
    let collection_file = options
        .root
        .join("collections")
        .join(format!("{}.modules", options.collection));
    let installer = options
        .root
        .join("dist")
        .join(format!("{}.sh", options.collection));
    let modules = read_modules(&collection_file)?;
    if modules.is_empty() {
        return Err(format!(
            "collection {} contains no modules",
            options.collection
        ));
    }
    if !installer.is_file() {
        return Err(format!("installer not found: {}", installer.display()));
    }

    fs::create_dir_all(&options.state_dir).map_err(|error| error.to_string())?;
    fs::create_dir_all(&options.log_dir).map_err(|error| error.to_string())?;
    let state_path = options
        .state_dir
        .join(format!("{}.json", options.collection));
    let mut state = load_state(&state_path, &options.collection)?;
    let completed_flags: Vec<bool> = modules
        .iter()
        .map(|module| {
            state
                .modules
                .get(module)
                .is_some_and(|entry| entry.status == "completed")
        })
        .collect();
    let already_completed = completed_flags.iter().filter(|done| **done).count();
    let use_dashboard = interactive_terminal();
    let mut dashboard = if use_dashboard {
        Some(Dashboard::new(
            &options.collection,
            &modules,
            &completed_flags,
        )?)
    } else {
        None
    };
    let mut progress = if use_dashboard {
        None
    } else {
        Some(ProgressView::new(
            &options.collection,
            already_completed,
            modules.len(),
        ))
    };
    let run_id = run_id();
    let log_path = options.log_dir.join(format!("{run_id}.jsonl"));
    let file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&log_path)
        .map_err(|error| error.to_string())?;
    let mut log = EventLog(file);
    log.emit(json!({"at": now(), "event": "run_started", "run_id": run_id, "collection": options.collection}))
        .map_err(|error| error.to_string())?;

    for module in modules {
        if state
            .modules
            .get(&module)
            .is_some_and(|entry| entry.status == "completed")
        {
            if let Some(progress) = &progress {
                progress.skipped(&module);
            }
            log.emit(json!({"at": now(), "event": "module_skipped", "module": module, "reason": "completed"}))
                .map_err(|error| error.to_string())?;
            continue;
        }

        let attempts = state
            .modules
            .get(&module)
            .map_or(1, |entry| entry.attempts + 1);
        if let Some(progress) = &progress {
            progress.started(&module, attempts);
        }
        log.emit(
            json!({"at": now(), "event": "module_started", "module": module, "attempt": attempts}),
        )
        .map_err(|error| error.to_string())?;
        state.modules.insert(
            module.clone(),
            ModuleState {
                status: "running".to_owned(),
                attempts,
                updated_at: now(),
            },
        );
        state.updated_at = now();
        save_state(&state_path, &state)?;

        let started_at = Instant::now();
        let (succeeded, exit_code) = if let Some(dashboard) = &mut dashboard {
            dashboard.run_module(&installer, &options.forwarded, &module)?
        } else {
            let status = Command::new("bash")
                .arg(&installer)
                .args(&options.forwarded)
                .arg("--only")
                .arg(&module)
                .env(
                    "MISTBORN_RUNNER_BINARY",
                    env::current_exe().map_err(|error| error.to_string())?,
                )
                .status()
                .map_err(|error| format!("failed to start {module}: {error}"))?;
            (status.success(), status.code())
        };
        let elapsed = started_at.elapsed();
        state.modules.insert(
            module.clone(),
            ModuleState {
                status: if succeeded { "completed" } else { "failed" }.to_owned(),
                attempts,
                updated_at: now(),
            },
        );
        state.updated_at = now();
        save_state(&state_path, &state)?;
        log.emit(json!({"at": now(), "event": if succeeded { "module_completed" } else { "module_failed" }, "module": module, "exit_code": exit_code, "elapsed_ms": elapsed.as_millis()}))
            .map_err(|error| error.to_string())?;
        if !succeeded {
            if let Some(progress) = &progress {
                progress.failed(&module, elapsed);
            }
            return Err(format!(
                "module {module} failed; rerun the same command to resume"
            ));
        }
        if let Some(progress) = &mut progress {
            progress.completed(&module, elapsed);
        }
    }

    log.emit(json!({"at": now(), "event": "run_completed", "run_id": run_id, "collection": options.collection}))
        .map_err(|error| error.to_string())?;
    drop(dashboard);
    if let Some(progress) = &progress {
        progress.finish();
    }
    println!("  state: {}", state_path.display());
    println!("  log:   {}", log_path.display());
    Ok(())
}

fn execute_host(arguments: &[String]) -> Result<(), String> {
    if arguments.len() < 3 || arguments[1] != "--script" {
        return Err(format!(
            "Usage: mistborn-bootstrap host --script PATH [COMMAND [ARGS...]]\n{}",
            usage()
        ));
    }
    let script = PathBuf::from(&arguments[2]);
    if !script.is_file() {
        return Err(format!(
            "host command script not found: {}",
            script.display()
        ));
    }
    let command_arguments = &arguments[3..];
    let operation = command_arguments
        .first()
        .map_or("help", |argument| argument.as_str());
    if interactive_terminal() {
        let steps = vec![operation.to_owned()];
        let mut dashboard = Dashboard::new("host", &steps, &[false])?;
        let (succeeded, code) =
            dashboard.run_host_command(&script, command_arguments, operation)?;
        let output = dashboard.screen_contents();
        drop(dashboard);
        let output = output.trim_end();
        if !output.is_empty() {
            println!("{output}");
        }
        if succeeded {
            Ok(())
        } else {
            Err(format!(
                "mistborn {operation} failed with exit code {}",
                code.map_or_else(|| "unknown".to_owned(), |value| value.to_string())
            ))
        }
    } else {
        let status = Command::new("bash")
            .arg(&script)
            .args(command_arguments)
            .status()
            .map_err(|error| format!("failed to start mistborn {operation}: {error}"))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!(
                "mistborn {operation} failed with exit code {}",
                status
                    .code()
                    .map_or_else(|| "unknown".to_owned(), |value| value.to_string())
            ))
        }
    }
}

fn main() -> ExitCode {
    let arguments: Vec<String> = env::args().collect();
    let result = if arguments.get(1).is_some_and(|argument| argument == "host") {
        execute_host(&arguments[1..])
    } else {
        parse_args().and_then(execute)
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("mistborn-bootstrap: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary_directory() -> PathBuf {
        let path = env::temp_dir().join(format!("mistborn-bootstrap-test-{}", run_id()));
        fs::create_dir(&path).unwrap();
        path
    }

    #[test]
    fn failed_collection_resumes_without_repeating_completed_modules() {
        let base = temporary_directory();
        let root = base.join("root");
        let state_dir = base.join("state");
        let log_dir = base.join("logs");
        fs::create_dir_all(root.join("collections")).unwrap();
        fs::create_dir(root.join("dist")).unwrap();
        fs::write(root.join("collections/test.modules"), "first\nsecond\n").unwrap();
        fs::write(
            root.join("dist/test.sh"),
            format!(
                "#!/usr/bin/env bash\nprintf '%s\\n' \"$2\" >>'{}'\nif [[ \"$2\" == second && ! -e '{}' ]]; then touch '{}'; exit 42; fi\n",
                base.join("calls").display(),
                base.join("failed-once").display(),
                base.join("failed-once").display()
            ),
        )
        .unwrap();

        let options = || Options {
            collection: "test".to_owned(),
            root: root.clone(),
            state_dir: state_dir.clone(),
            log_dir: log_dir.clone(),
            forwarded: Vec::new(),
        };
        assert!(execute(options()).is_err());
        execute(options()).unwrap();

        let calls = fs::read_to_string(base.join("calls")).unwrap();
        assert_eq!(
            calls.lines().collect::<Vec<_>>(),
            ["first", "second", "second"]
        );
        let state: RunState =
            serde_json::from_reader(File::open(state_dir.join("test.json")).unwrap()).unwrap();
        assert_eq!(state.modules["first"].status, "completed");
        assert_eq!(state.modules["second"].attempts, 2);
        assert_eq!(fs::read_dir(&log_dir).unwrap().count(), 2);
        assert!(fs::read_dir(&log_dir).unwrap().any(|entry| {
            fs::read_to_string(entry.unwrap().path())
                .unwrap()
                .contains("\"elapsed_ms\"")
        }));

        fs::remove_dir_all(base).unwrap();
    }
}
