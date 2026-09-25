mod command_dashboard;
mod dashboard;
mod input;
mod plan;
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

use command_dashboard::CommandDashboard;
use dashboard::Dashboard;
use progress::ProgressView;

#[derive(Debug)]
struct Options {
    command: RunCommand,
    collection: String,
    root: PathBuf,
    state_dir: PathBuf,
    log_dir: PathBuf,
    forwarded: Vec<String>,
    target: Option<String>,
}

#[derive(Debug, PartialEq)]
enum RunCommand {
    Apply,
    Plan,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct RunState {
    version: u8,
    collection: String,
    updated_at: u64,
    #[serde(default)]
    config_adoption: ConfigAdoption,
    modules: BTreeMap<String, ModuleState>,
}

#[derive(Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum ConfigAdoption {
    Pending,
    Published,
    #[default]
    Ineligible,
}

#[derive(Debug, Deserialize, Serialize)]
struct ModuleState {
    status: String,
    attempts: u32,
    updated_at: u64,
    #[serde(default, deserialize_with = "deserialize_tasks")]
    tasks: BTreeMap<String, TaskState>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct TaskState {
    status: String,
    #[serde(default)]
    applied_revision: u32,
    #[serde(default)]
    input_fingerprint: String,
    #[serde(default)]
    updated_at: u64,
}

fn deserialize_tasks<'de, D>(deserializer: D) -> Result<BTreeMap<String, TaskState>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let values = BTreeMap::<String, serde_json::Value>::deserialize(deserializer)?;
    values
        .into_iter()
        .map(|(id, value)| {
            let state = match value {
                serde_json::Value::String(status) => TaskState {
                    status,
                    applied_revision: 1,
                    ..TaskState::default()
                },
                value => serde_json::from_value(value).map_err(serde::de::Error::custom)?,
            };
            Ok((id, state))
        })
        .collect()
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
    "Usage: mistborn-bootstrap {run|apply|plan} COLLECTION [TARGET] [--root PATH] [--state-dir PATH] [--log-dir PATH] [--dry-run] [--yes] [--user NAME]\n       TARGET is STAGE or STAGE/TASK\n       mistborn-bootstrap host --script PATH [COMMAND [ARGS...]]\n       mistborn-bootstrap validate-config PATH"
}

fn validate_config(arguments: &[String]) -> Result<(), String> {
    if arguments.len() != 2 {
        return Err("Usage: mistborn-bootstrap validate-config PATH".to_owned());
    }
    mistborn_bootstrap::config::DesiredState::load(Path::new(&arguments[1]))
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn interactive_terminal() -> bool {
    env::var("MISTBORN_TUI").as_deref() != Ok("0")
        && io::stdin().is_terminal()
        && io::stdout().is_terminal()
}

fn value(args: &[String], index: &mut usize, flag: &str) -> Result<String, String> {
    *index += 1;
    args.get(*index)
        .cloned()
        .ok_or_else(|| format!("{flag} requires a value"))
}

fn parse_args() -> Result<Options, String> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.len() < 2 || !matches!(args[0].as_str(), "run" | "apply" | "plan") {
        return Err(usage().to_owned());
    }

    let command = if args[0] == "plan" {
        RunCommand::Plan
    } else {
        RunCommand::Apply
    };
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
    let mut target = None;
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
            value if !value.starts_with('-') && target.is_none() => target = Some(value.to_owned()),
            unknown => return Err(format!("unknown argument: {unknown}\n{}", usage())),
        }
        index += 1;
    }
    Ok(Options {
        command,
        collection,
        root,
        state_dir,
        log_dir,
        forwarded,
        target,
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

struct TaskEvent {
    stage: String,
    task: String,
    state: String,
}

fn read_task_events(path: &Path) -> Result<Vec<TaskEvent>, String> {
    let contents = fs::read_to_string(path).map_err(|error| error.to_string())?;
    Ok(contents
        .lines()
        .filter_map(|line| {
            let mut fields = line.split('\t');
            Some(TaskEvent {
                stage: fields.next()?.to_owned(),
                task: fields.next()?.to_owned(),
                state: fields.next()?.to_owned(),
            })
        })
        .collect())
}

fn load_state(path: &Path, collection: &str) -> Result<(RunState, bool), String> {
    if !path.exists() {
        return Ok((
            RunState {
                version: 3,
                collection: collection.to_owned(),
                config_adoption: ConfigAdoption::Pending,
                ..RunState::default()
            },
            false,
        ));
    }
    let file = File::open(path).map_err(|error| error.to_string())?;
    let mut state: RunState =
        serde_json::from_reader(file).map_err(|error| format!("invalid state file: {error}"))?;
    if !(1..=3).contains(&state.version) || state.collection != collection {
        return Err("state file belongs to an incompatible run".to_owned());
    }
    let migrated = state.version == 1;
    state.version = 3;
    Ok((state, migrated))
}

fn task_fingerprint(task: &plan::Task) -> String {
    if task.inputs.is_empty() {
        return String::new();
    }
    let mut hash = 0xcbf29ce484222325_u64;
    for key in &task.inputs {
        for byte in key
            .bytes()
            .chain([b'='])
            .chain(env::var(key).unwrap_or_default().bytes())
            .chain([0])
        {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    format!("{hash:016x}")
}

fn task_is_current(state: Option<&TaskState>, task: &plan::Task) -> bool {
    state.is_some_and(|state| {
        matches!(state.status.as_str(), "completed" | "skipped")
            && state.applied_revision == task.revision
            && (task.inputs.is_empty() || state.input_fingerprint == task_fingerprint(task))
    })
}

fn target_matches(target: Option<&str>, stage: &str, task: Option<&str>) -> bool {
    match target {
        None => true,
        Some(value) if value == stage => true,
        Some(value) => task.is_some_and(|task| value == format!("{stage}/{task}")),
    }
}

fn config_applied_tasks(state: &RunState) -> String {
    [
        ("security", "ssh"),
        ("security", "firewall"),
        ("security", "plex-firewall"),
        ("tailscale", "connect"),
        ("tailscale", "auto-update"),
    ]
    .into_iter()
    .filter(|(stage, task)| {
        state
            .modules
            .get(*stage)
            .and_then(|module| module.tasks.get(*task))
            .is_some_and(|task| task.status == "completed")
    })
    .map(|(stage, task)| format!("{stage}/{task}"))
    .collect::<Vec<_>>()
    .join(",")
}

fn describe_plan(plan: &plan::Plan, state: &RunState, target: Option<&str>) {
    for stage in &plan.stages {
        if !target_matches(target, &stage.id, None)
            && !target.is_some_and(|v| v.starts_with(&format!("{}/", stage.id)))
        {
            continue;
        }
        println!("{}:", stage.id);
        for task in &stage.tasks {
            if !target_matches(target, &stage.id, Some(&task.id)) {
                continue;
            }
            let saved = state
                .modules
                .get(&stage.id)
                .and_then(|module| module.tasks.get(&task.id));
            let status = if task_is_current(saved, task) {
                "current"
            } else if saved.is_none() {
                "pending"
            } else if saved.is_some_and(|s| s.applied_revision != task.revision) {
                "stale"
            } else {
                "changed"
            };
            let confirmation = if task.requires_confirmation {
                " (explicit apply required)"
            } else {
                ""
            };
            println!("  {:<18} {status}{confirmation}", task.id);
        }
    }
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
    let plan_path = options
        .root
        .join("plans")
        .join(format!("{}.toml", options.collection));
    let plan = plan::load(&plan_path, &options.collection)?;
    let planned_modules: Vec<String> = plan.stages.iter().map(|stage| stage.id.clone()).collect();
    if modules != planned_modules {
        return Err(format!(
            "plan {} does not match {}",
            plan_path.display(),
            collection_file.display()
        ));
    }
    if plan.stages.is_empty() {
        return Err(format!(
            "collection {} contains no modules",
            options.collection
        ));
    }
    if !installer.is_file() {
        return Err(format!("installer not found: {}", installer.display()));
    }

    if let Some(target) = options.target.as_deref() {
        let valid = plan.stages.iter().any(|stage| {
            target == stage.id
                || stage
                    .tasks
                    .iter()
                    .any(|task| target == format!("{}/{}", stage.id, task.id))
        });
        if !valid {
            return Err(format!("unknown target: {target}"));
        }
    }

    let state_path = options
        .state_dir
        .join(format!("{}.json", options.collection));
    let (mut state, migrated) = load_state(&state_path, &options.collection)?;
    if migrated {
        for stage in &plan.stages {
            if let Some(module) = state.modules.get_mut(&stage.id) {
                for task in &stage.tasks {
                    if let Some(saved) = module.tasks.get_mut(&task.id) {
                        saved.input_fingerprint = task_fingerprint(task);
                    }
                }
            }
        }
    }
    if options.command == RunCommand::Plan {
        describe_plan(&plan, &state, options.target.as_deref());
        return Ok(());
    }

    fs::create_dir_all(&options.state_dir).map_err(|error| error.to_string())?;
    fs::create_dir_all(&options.log_dir).map_err(|error| error.to_string())?;
    if migrated {
        let backup = state_path.with_extension("json.v1.bak");
        if !backup.exists() {
            fs::copy(&state_path, &backup)
                .map_err(|error| format!("cannot back up v1 state: {error}"))?;
        }
    }
    let completed_flags: Vec<bool> = plan
        .stages
        .iter()
        .map(|stage| {
            state.modules.get(&stage.id).is_some_and(|entry| {
                entry.status == "completed"
                    && stage
                        .tasks
                        .iter()
                        .all(|task| task_is_current(entry.tasks.get(&task.id), task))
            })
        })
        .collect();
    let already_completed = completed_flags.iter().filter(|done| **done).count();
    let use_dashboard = interactive_terminal();
    let mut dashboard = if use_dashboard {
        Some(Dashboard::new(
            &options.collection,
            &plan.stages,
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
            plan.stages.len(),
        ))
    };
    let run_token = run_id();
    let log_path = options.log_dir.join(format!("{run_token}.jsonl"));
    let file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&log_path)
        .map_err(|error| error.to_string())?;
    let mut log = EventLog(file);
    log.emit(json!({"at": now(), "event": "run_started", "run_id": run_token, "collection": options.collection}))
        .map_err(|error| error.to_string())?;

    for (stage_index, stage) in plan.stages.iter().enumerate() {
        let module = &stage.id;
        let existed = state.modules.contains_key(module);
        let selected_tasks: Vec<String> = stage
            .tasks
            .iter()
            .filter(|task| {
                if !target_matches(options.target.as_deref(), module, Some(&task.id)) {
                    return false;
                }
                let current = state
                    .modules
                    .get(module)
                    .and_then(|entry| entry.tasks.get(&task.id));
                let requested = options.target.is_some();
                (requested || module == "toolset" || !task_is_current(current, task))
                    && (requested || !task.requires_confirmation || !existed)
            })
            .map(|task| task.id.clone())
            .collect();
        let empty_stage_needs_run = stage.tasks.is_empty()
            && !state
                .modules
                .get(module)
                .is_some_and(|entry| entry.status == "completed");
        if selected_tasks.is_empty() && !empty_stage_needs_run {
            let has_outstanding_tasks = stage.tasks.iter().any(|task| {
                !task_is_current(
                    state
                        .modules
                        .get(module)
                        .and_then(|entry| entry.tasks.get(&task.id)),
                    task,
                )
            });
            if has_outstanding_tasks && let Some(module_state) = state.modules.get_mut(module) {
                module_state.status = "partial".to_owned();
                module_state.updated_at = now();
                state.updated_at = now();
                save_state(&state_path, &state)?;
            }
            if let Some(progress) = &progress {
                progress.skipped(&module);
            }
            log.emit(json!({"at": now(), "event": "module_skipped", "module": module, "reason": "completed"}))
                .map_err(|error| error.to_string())?;
            continue;
        }

        let attempts = state
            .modules
            .get(module)
            .map_or(1, |entry| entry.attempts + 1);
        if let Some(progress) = &progress {
            progress.started(&module, attempts);
        }
        log.emit(
            json!({"at": now(), "event": "module_started", "module": module, "attempt": attempts}),
        )
        .map_err(|error| error.to_string())?;
        let module_state = state
            .modules
            .entry(module.clone())
            .or_insert_with(|| ModuleState {
                status: "pending".to_owned(),
                attempts: 0,
                updated_at: 0,
                tasks: BTreeMap::new(),
            });
        module_state.status = "running".to_owned();
        module_state.attempts = attempts;
        module_state.updated_at = now();
        for task in &selected_tasks {
            module_state.tasks.entry(task.clone()).or_default().status = "pending".to_owned();
        }
        state.updated_at = now();
        save_state(&state_path, &state)?;

        let started_at = Instant::now();
        let progress_path = env::temp_dir().join(format!("mistborn-progress-{}", run_id()));
        File::create(&progress_path).map_err(|error| error.to_string())?;
        let (succeeded, exit_code) = if let Some(dashboard) = &mut dashboard {
            dashboard.run_module(
                &installer,
                &options.forwarded,
                stage_index,
                &progress_path,
                &selected_tasks,
                state.config_adoption == ConfigAdoption::Pending,
                config_applied_tasks(&state),
            )?
        } else {
            let status = Command::new("bash")
                .arg(&installer)
                .args(&options.forwarded)
                .arg("--only")
                .arg(&module)
                .args(if selected_tasks.is_empty() {
                    Vec::new()
                } else {
                    vec!["--tasks".to_owned(), selected_tasks.join(",")]
                })
                .env(
                    "MISTBORN_RUNNER_BINARY",
                    env::current_exe().map_err(|error| error.to_string())?,
                )
                .env("MISTBORN_PROGRESS_FILE", &progress_path)
                .env("MISTBORN_PROGRESS_STAGE", module)
                .env(
                    "MISTBORN_CONFIG_ADOPTION_ALLOWED",
                    if state.config_adoption == ConfigAdoption::Pending {
                        "1"
                    } else {
                        "0"
                    },
                )
                .env(
                    "MISTBORN_CONFIG_APPLIED_TASKS",
                    config_applied_tasks(&state),
                )
                .status()
                .map_err(|error| format!("failed to start {module}: {error}"))?;
            (status.success(), status.code())
        };
        let task_events = read_task_events(&progress_path)?;
        let _ = fs::remove_file(&progress_path);
        if let Some(module_state) = state.modules.get_mut(module) {
            for event in task_events {
                if event.stage != *module {
                    continue;
                }
                module_state
                    .tasks
                    .entry(event.task.clone())
                    .or_default()
                    .status = event.state.clone();
                let action = stage
                    .tasks
                    .iter()
                    .find(|task| task.id == event.task)
                    .map(|task| task.action.as_str())
                    .unwrap_or("");
                log.emit(json!({"at": now(), "event": format!("task_{}", event.state), "module": module, "task": event.task, "action": action}))
                    .map_err(|error| error.to_string())?;
            }
            if !succeeded {
                for (task_id, task_state) in &mut module_state.tasks {
                    if task_state.status == "started" {
                        task_state.status = "failed".to_owned();
                        log.emit(json!({"at": now(), "event": "task_failed", "module": module, "task": task_id}))
                            .map_err(|error| error.to_string())?;
                    }
                }
                if module_state
                    .tasks
                    .iter()
                    .filter(|(id, _)| selected_tasks.contains(id))
                    .all(|(_, task_state)| task_state.status == "pending")
                {
                    if let Some((task_id, task_state)) = module_state
                        .tasks
                        .iter_mut()
                        .find(|(id, _)| selected_tasks.contains(id))
                    {
                        task_state.status = "failed".to_owned();
                        log.emit(json!({"at": now(), "event": "task_failed", "module": module, "task": task_id}))
                            .map_err(|error| error.to_string())?;
                    }
                }
            }
        }
        let elapsed = started_at.elapsed();
        let module_state = state
            .modules
            .get_mut(module)
            .expect("module state was just inserted");
        if succeeded {
            for task in &stage.tasks {
                if selected_tasks.contains(&task.id) {
                    let saved = module_state.tasks.entry(task.id.clone()).or_default();
                    if saved.status == "pending" || saved.status == "started" {
                        saved.status = "skipped".to_owned();
                    }
                    saved.applied_revision = task.revision;
                    saved.input_fingerprint = task_fingerprint(task);
                    saved.updated_at = now();
                }
            }
        }
        module_state.status = if !succeeded {
            "failed"
        } else if stage
            .tasks
            .iter()
            .all(|task| task_is_current(module_state.tasks.get(&task.id), task))
        {
            "completed"
        } else {
            "partial"
        }
        .to_owned();
        module_state.attempts = attempts;
        module_state.updated_at = now();
        if succeeded
            && module == "toolset"
            && module_state
                .tasks
                .get("command")
                .is_some_and(|task| task.status == "completed")
            && state.config_adoption == ConfigAdoption::Pending
        {
            state.config_adoption = ConfigAdoption::Published;
        }
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

    log.emit(json!({"at": now(), "event": "run_completed", "run_id": run_token, "collection": options.collection}))
        .map_err(|error| error.to_string())?;
    let home_available =
        options.collection == "server" && Path::new("/usr/local/lib/mistborn/host.sh").is_file();
    let return_to_home = if let Some(dashboard) = &mut dashboard {
        dashboard.wait_for_exit(home_available)?
    } else {
        false
    };
    drop(dashboard);
    if let Some(progress) = &progress {
        progress.finish();
    }
    println!("  state: {}", state_path.display());
    println!("  log:   {}", log_path.display());
    if return_to_home {
        execute_host(&[
            "host".to_owned(),
            "--script".to_owned(),
            "/usr/local/lib/mistborn/host.sh".to_owned(),
        ])?;
    }
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
    let mut command_arguments = arguments[3..].to_vec();
    let use_dashboard = interactive_terminal();
    let shows_menu = command_arguments.is_empty()
        || command_arguments
            .first()
            .is_some_and(|command| matches!(command.as_str(), "help" | "-h" | "--help"));
    if use_dashboard {
        let mut menu_open = shows_menu;
        loop {
            if menu_open {
                let Some(operation) = CommandDashboard::select_command()? else {
                    return Ok(());
                };
                command_arguments = vec![operation.to_owned()];
            }
            let operation = command_arguments
                .first()
                .map_or("help", |argument| argument.as_str());
            let command = std::iter::once("mistborn".to_owned())
                .chain(command_arguments.iter().cloned())
                .collect::<Vec<_>>()
                .join(" ");
            let mut dashboard = CommandDashboard::new(operation, &command)?;
            let progress_path = env::temp_dir().join(format!("mistborn-progress-{}", run_id()));
            File::create(&progress_path).map_err(|error| error.to_string())?;
            let (succeeded, code) = dashboard.run(&script, &command_arguments, &progress_path)?;
            let return_to_home = dashboard.wait_for_exit()?;
            drop(dashboard);
            let _ = fs::remove_file(&progress_path);
            if return_to_home {
                menu_open = true;
                continue;
            }
            return if succeeded {
                Ok(())
            } else {
                Err(format!(
                    "mistborn {operation} failed with exit code {}",
                    code.map_or_else(|| "unknown".to_owned(), |value| value.to_string())
                ))
            };
        }
    } else {
        let command_arguments = command_arguments.as_slice();
        let operation = command_arguments
            .first()
            .map_or("help", |argument| argument.as_str());
        let progress_path = env::temp_dir().join(format!("mistborn-progress-{}", run_id()));
        File::create(&progress_path).map_err(|error| error.to_string())?;
        let status = Command::new("bash")
            .arg(&script)
            .args(command_arguments)
            .env("MISTBORN_PROGRESS_FILE", &progress_path)
            .env("MISTBORN_PROGRESS_STAGE", "bootstrap")
            .env(
                "MISTBORN_BOOTSTRAP_VERSION",
                format!("v{}", env!("CARGO_PKG_VERSION")),
            )
            .status()
            .map_err(|error| format!("failed to start mistborn {operation}: {error}"))?;
        let _ = fs::remove_file(progress_path);
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
    } else if arguments
        .get(1)
        .is_some_and(|argument| argument == "validate-config")
    {
        validate_config(&arguments[1..])
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
        fs::create_dir(root.join("plans")).unwrap();
        fs::write(root.join("collections/test.modules"), "first\nsecond\n").unwrap();
        fs::write(
            root.join("plans/test.toml"),
            "version=1\ncollection='test'\n[[stages]]\nid='first'\ntitle='First'\nhelp='First test stage'\n[[stages]]\nid='second'\ntitle='Second'\nhelp='Second test stage'\n",
        )
        .unwrap();
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
            command: RunCommand::Apply,
            collection: "test".to_owned(),
            root: root.clone(),
            state_dir: state_dir.clone(),
            log_dir: log_dir.clone(),
            forwarded: Vec::new(),
            target: None,
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

    #[test]
    fn version_one_state_is_migrated_without_losing_task_results() {
        let base = temporary_directory();
        let path = base.join("server.json");
        fs::write(
            &path,
            r#"{"version":1,"collection":"server","updated_at":1,"modules":{"security":{"status":"completed","attempts":1,"updated_at":1,"tasks":{"ssh":"completed"}}}}"#,
        )
        .unwrap();

        let (state, migrated) = load_state(&path, "server").unwrap();
        assert!(migrated);
        assert_eq!(state.version, 3);
        assert_eq!(state.config_adoption, ConfigAdoption::Ineligible);
        assert_eq!(state.modules["security"].tasks["ssh"].status, "completed");
        assert_eq!(state.modules["security"].tasks["ssh"].applied_revision, 1);
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn interrupted_fresh_install_remains_eligible_for_config_adoption() {
        let base = temporary_directory();
        let path = base.join("server.json");
        let (mut state, migrated) = load_state(&path, "server").unwrap();
        assert!(!migrated);
        assert_eq!(state.config_adoption, ConfigAdoption::Pending);

        state.modules.insert(
            "security".to_owned(),
            ModuleState {
                status: "failed".to_owned(),
                attempts: 1,
                updated_at: 1,
                tasks: BTreeMap::new(),
            },
        );
        save_state(&path, &state).unwrap();

        let (resumed, migrated) = load_state(&path, "server").unwrap();
        assert!(!migrated);
        assert_eq!(resumed.config_adoption, ConfigAdoption::Pending);
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn legacy_upgrade_is_ineligible_for_config_adoption() {
        let base = temporary_directory();
        let path = base.join("server.json");
        fs::write(
            &path,
            r#"{"version":2,"collection":"server","updated_at":1,"modules":{}}"#,
        )
        .unwrap();

        let (state, migrated) = load_state(&path, "server").unwrap();
        assert!(!migrated);
        assert_eq!(state.version, 3);
        assert_eq!(state.config_adoption, ConfigAdoption::Ineligible);
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn task_revision_invalidates_a_previous_pass() {
        let task = plan::Task {
            id: "firewall".to_owned(),
            title: "Firewall".to_owned(),
            action: "configure".to_owned(),
            weight: 1,
            revision: 2,
            inputs: Vec::new(),
            requires_confirmation: true,
        };
        let state = TaskState {
            status: "completed".to_owned(),
            applied_revision: 1,
            ..TaskState::default()
        };

        assert!(!task_is_current(Some(&state), &task));
    }
}
