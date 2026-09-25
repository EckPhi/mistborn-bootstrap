mod command_dashboard;
mod dashboard;
mod host_diagnostics;
mod input;
mod package_service_adapter;
mod plan;
mod progress;
mod ssh_adapter;

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
    let mut command_arguments = arguments[3..].to_vec();
    if command_arguments.first().is_some_and(|command| {
        matches!(command.as_str(), "status" | "doctor" | "plan" | "reconcile")
    }) && !command_arguments.iter().any(|arg| arg == "--legacy")
    {
        return execute_host_management_command(&command_arguments);
    }
    if command_arguments.iter().any(|arg| arg == "--legacy") {
        command_arguments.retain(|arg| arg != "--legacy");
    }
    let script = PathBuf::from(&arguments[2]);
    if !script.is_file() {
        return Err(format!(
            "host command script not found: {}",
            script.display()
        ));
    }
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

fn execute_host_management_command(arguments: &[String]) -> Result<(), String> {
    let command = arguments.first().map(String::as_str).unwrap_or_default();
    let kind = if command == "status" {
        host_diagnostics::ReportKind::Status
    } else if command == "doctor" {
        host_diagnostics::ReportKind::Doctor
    } else if command == "plan" {
        host_diagnostics::ReportKind::Doctor
    } else if command == "reconcile" {
        host_diagnostics::ReportKind::Doctor
    } else {
        return Err(inspection_cli_error("unsupported host command"));
    };
    let mut fix = false;
    let mut safe = false;
    let mut target = None;
    let mut explicit_yes = false;
    let mut format = "text";
    let mut index = 1;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--format" if index + 1 < arguments.len() => {
                format = arguments[index + 1].as_str();
                index += 2;
            }
            "--format" => return Err(inspection_cli_error("--format requires text or json")),
            "--fix" if command == "doctor" => {
                fix = true;
                index += 1;
            }
            "--safe" if command == "doctor" || command == "reconcile" => {
                safe = true;
                index += 1;
            }
            "--yes" if command == "reconcile" || (command == "doctor" && fix) => {
                explicit_yes = true;
                index += 1;
            }
            "--legacy" => {
                return Err(inspection_cli_error(
                    "internal error: legacy command was routed to Rust",
                ));
            }
            value
                if !value.starts_with('-')
                    && target.is_none()
                    && matches!(command, "plan" | "reconcile") =>
            {
                target = Some(
                    value
                        .parse::<mistborn_bootstrap::domain::RemediationId>()
                        .map_err(|error| inspection_cli_error(&error))?,
                );
                index += 1;
            }
            option => {
                return Err(inspection_cli_error(&format!(
                    "unknown {command} option: {option}"
                )));
            }
        }
    }
    if !matches!(format, "text" | "json") {
        return Err(inspection_cli_error(&format!(
            "unsupported output format: {format}"
        )));
    }
    if command == "plan" || command == "reconcile" || fix {
        return execute_reconciliation_command(
            command,
            kind,
            target,
            safe,
            explicit_yes,
            fix,
            format,
        );
    }

    let report = host_diagnostics::inspect(kind, &host_diagnostics::SystemCommands);
    if format == "json" {
        println!(
            "{}",
            serde_json::to_string_pretty(&report)
                .map_err(|error| inspection_cli_error(&error.to_string()))?
        );
    } else if command == "status" {
        println!("Mistborn {}", report.version);
        println!("{}", status_config_line(&report.config));
        if let Some(host_diagnostics::InspectionStatus::Available(versions)) =
            report.observed.facts.get("versions")
        {
            println!("Components");
            if let Some(versions) = versions.as_object() {
                for (name, version) in versions {
                    println!(
                        "  {name}: {}",
                        version
                            .as_str()
                            .filter(|s| !s.is_empty())
                            .unwrap_or("installed; version unavailable")
                    );
                }
            }
        }
        println!("Services");
        for service in ["docker", "tailscaled", "fail2ban"] {
            match report.observed.facts.get(&format!("service.{service}")) {
                Some(host_diagnostics::InspectionStatus::Available(value)) => println!(
                    "  {service}: {}",
                    if value["active"] == true {
                        "active"
                    } else {
                        "inactive"
                    }
                ),
                Some(host_diagnostics::InspectionStatus::Unavailable { .. }) => {
                    println!("  {service}: systemd unavailable")
                }
                Some(host_diagnostics::InspectionStatus::Error { message }) => {
                    println!("  {service}: inspection error ({message})")
                }
                _ => println!("  {service}: unknown"),
            }
        }
        println!("{}", status_drift_line(&report.summary));
    } else {
        print_doctor_report(&report);
    }
    if report
        .diagnostics
        .iter()
        .any(|item| item.severity == mistborn_bootstrap::domain::DiagnosticSeverity::Fail)
    {
        return Err(format!("mistborn {command} found host drift"));
    }
    if report.inspection_incomplete {
        return Err(format!(
            "exit-code-2: mistborn {command} could not inspect all host facts"
        ));
    }
    Ok(())
}

fn print_doctor_report(report: &host_diagnostics::Report) {
    for diagnostic in &report.diagnostics {
        println!(
            "{:4}  {}",
            format!("{:?}", diagnostic.severity).to_uppercase(),
            diagnostic.summary
        );
        if !diagnostic.evidence.is_empty() {
            println!("      evidence: {}", diagnostic.evidence.join("; "));
        }
        if let Some(remediation) = &diagnostic.remediation_id {
            println!("      remediation: {remediation}");
        }
        if let Some(note) = confirmation_note(diagnostic) {
            println!("      {note}");
        }
    }
    for (key, state) in &report.observed.facts {
        match state {
            host_diagnostics::InspectionStatus::Unavailable { reason } => {
                println!("WARN  {key}: unavailable ({reason})")
            }
            host_diagnostics::InspectionStatus::Error { message } => {
                println!("WARN  {key}: inspection error ({message})")
            }
            _ => {}
        }
    }
}

fn execute_reconciliation_command(
    command: &str,
    kind: host_diagnostics::ReportKind,
    target: Option<mistborn_bootstrap::domain::RemediationId>,
    safe: bool,
    explicit_yes: bool,
    fix: bool,
    format: &str,
) -> Result<(), String> {
    use mistborn_bootstrap::reconciliation::{self, ApplyOptions};
    if format == "json" && (command == "reconcile" || fix) {
        return Err(
            "exit-code-2: --format json is not supported for mutating commands; use text"
                .to_owned(),
        );
    }
    let report = host_diagnostics::inspect(kind, &host_diagnostics::SystemCommands);
    let snapshot = serde_json::to_value(&report.observed).map_err(|error| error.to_string())?;
    let proposed = reconciliation::plan(&snapshot, &report.diagnostics, target)?;
    if format == "json" {
        println!(
            "{}",
            serde_json::to_string_pretty(&proposed).map_err(|error| error.to_string())?
        );
    } else {
        println!(
            "Host reconciliation plan (snapshot {})",
            proposed.snapshot_id
        );
        if proposed.remediations.is_empty() {
            println!("No remediation needed.");
        }
        for item in &proposed.remediations {
            println!(
                "{}  [{:?}]{}",
                item.id,
                item.risk,
                if item.confirmation == mistborn_bootstrap::domain::ConfirmationPolicy::Explicit {
                    "  confirmation required"
                } else {
                    ""
                }
            );
            for action in &item.actions {
                println!("  - {}", action.description);
            }
            if !item.dependencies.is_empty() {
                println!("  depends on: {}", item.dependencies.join(", "));
            }
            println!("  verify: {}", item.verification);
            if !item.available {
                println!(
                    "  unavailable: {}",
                    item.unavailable_reason
                        .as_deref()
                        .unwrap_or("no adapter registered")
                );
            }
        }
    }
    if command == "plan" {
        return Ok(());
    }
    if proposed.remediations.is_empty() {
        return Ok(());
    }
    if let Some(unavailable) = proposed.remediations.iter().find(|item| {
        !item.available && !(safe && item.risk != mistborn_bootstrap::domain::RiskClass::Low)
    }) {
        return Err(format!(
            "{} is not actionable in this release: {}",
            unavailable.id,
            unavailable
                .unavailable_reason
                .as_deref()
                .unwrap_or("no adapter registered")
        ));
    }

    // The selected ID is itself an explicit per-target approval. `--yes` alone
    // never approves access or destructive changes.
    let mut confirmed = std::collections::BTreeSet::new();
    if let Some(target) = target {
        confirmed.insert(target);
    }
    let interactive = interactive_terminal();
    if !safe && interactive && !explicit_yes {
        println!(
            "Applying this plan may change host configuration. Type `apply` to approve the displayed plan:"
        );
        let mut answer = String::new();
        io::stdin()
            .read_line(&mut answer)
            .map_err(|error| error.to_string())?;
        if answer.trim() != "apply" {
            return Err("reconciliation cancelled".to_owned());
        }
        confirmed.extend(proposed.remediations.iter().filter_map(|item| {
            item.id
                .parse::<mistborn_bootstrap::domain::RemediationId>()
                .ok()
        }));
    }
    if !safe && !interactive && target.is_none() {
        return Err("exit-code-2: non-interactive reconciliation requires --safe or an explicit remediation target".to_owned());
    }
    let history = Path::new("/var/lib/mistborn-bootstrap/reconciliation-history.json");
    let mut adapter = SystemReconcileAdapter;
    if safe {
        for item in &proposed.remediations {
            if item.risk != mistborn_bootstrap::domain::RiskClass::Low {
                println!("{}: skipped by --safe (risk: {:?})", item.id, item.risk);
            }
        }
    }
    let results = reconciliation::reconcile(
        &proposed,
        &report.diagnostics,
        &mut adapter,
        history,
        &ApplyOptions {
            safe,
            confirmed,
            non_interactive: !interactive || explicit_yes,
        },
    )
    .map_err(|error| {
        if error.contains("confirmation required") {
            format!("exit-code-2: {error}")
        } else {
            error
        }
    })?;
    if safe && results.is_empty() {
        println!(
            "No low-risk allowlisted remediation was selected; the listed changes remain unapplied."
        );
    }
    for result in results {
        println!(
            "{}: {}",
            result.remediation_id,
            if result.verified {
                "verified"
            } else {
                "failed"
            }
        );
    }
    Ok(())
}

struct SystemReconcileAdapter;

fn apply_security_remediation(
    remediation: mistborn_bootstrap::domain::RemediationId,
) -> Result<(), String> {
    use mistborn_bootstrap::domain::RemediationId;
    let desired =
        mistborn_bootstrap::config::DesiredState::load(Path::new("/etc/mistborn/config.toml"))
            .map_err(|error| format!("cannot load desired configuration: {error}"))?;
    match remediation {
        RemediationId::SecurityUfw => apply_ufw_policy(&desired),
        RemediationId::SecurityPlexFirewall => apply_plex_firewall(&desired),
        RemediationId::SecurityFail2banPolicy => apply_fail2ban_policy(&desired),
        RemediationId::SecuritySsh => {
            let ssh = desired
                .ssh
                .as_ref()
                .ok_or("desired [ssh] section is missing")?;
            let mut adapter = ssh_adapter::SshAdapter {
                root: PathBuf::from("/"),
                runner: ssh_adapter::SystemRunner,
            };
            adapter.apply(ssh)
        }
        RemediationId::SecurityTailscaleSsh
        | RemediationId::SecurityTailscaleExitNode
        | RemediationId::SecurityTailscaleAutoUpdate => {
            apply_tailscale_policy(remediation, &desired)
        }
        other => Err(format!(
            "no host adapter is registered for {} yet",
            other.as_str()
        )),
    }
}

const FAIL2BAN_DROPIN_MARKER: &str = "# Managed by Mistborn: security/fail2ban-policy";

fn fail2ban_policy_port(desired: &mistborn_bootstrap::config::DesiredState) -> Result<u16, String> {
    if let Some(port) = desired.ssh.as_ref().map(|ssh| ssh.port.get()) {
        return Ok(port);
    }
    let output = Command::new("sshd")
        .arg("-T")
        .output()
        .map_err(|error| format!("cannot inspect effective SSH port with sshd -T: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "sshd -T exited with {}",
            output.status.code().unwrap_or(128)
        ));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    parse_effective_ssh_port(&text)
}

fn parse_effective_ssh_port(text: &str) -> Result<u16, String> {
    let ports = text
        .lines()
        .filter_map(|line| {
            let (key, value) = line.split_once(' ')?;
            (key == "port")
                .then(|| value.trim().parse::<u16>().ok())
                .flatten()
        })
        .collect::<Vec<_>>();
    if ports.len() != 1 || ports[0] == 0 {
        return Err(
            "effective SSH port is unknown or ambiguous; refusing to configure fail2ban".to_owned(),
        );
    }
    Ok(ports[0])
}

fn render_fail2ban_dropin(
    policy: &mistborn_bootstrap::config::Fail2banConfig,
    port: u16,
) -> Result<String, String> {
    if !policy.enabled || !policy.sshd.enabled {
        return Err(
            "automatic fail2ban policy application only supports an enabled service and sshd jail"
                .to_owned(),
        );
    }
    if policy.sshd.maxretry == 0 || policy.sshd.bantime == 0 || port == 0 {
        return Err(
            "fail2ban maxretry, bantime, and SSH port must be greater than zero".to_owned(),
        );
    }
    Ok(format!(
        "{FAIL2BAN_DROPIN_MARKER}\n[sshd]\nenabled = true\nport = {port}\nmaxretry = {}\nbantime = {}\n",
        policy.sshd.maxretry, policy.sshd.bantime
    ))
}

fn apply_fail2ban_policy(desired: &mistborn_bootstrap::config::DesiredState) -> Result<(), String> {
    let policy = desired
        .fail2ban
        .as_ref()
        .ok_or("fail2ban policy is unmanaged")?;
    let port = fail2ban_policy_port(desired)?;
    let contents = render_fail2ban_dropin(policy, port)?;
    let directory = Path::new("/etc/fail2ban/jail.d");
    let target = directory.join("99-mistborn-bootstrap.local");
    ensure_real_directory(directory)?;
    let previous = read_owned_fail2ban_dropin(&target)?;
    atomic_replace_owned_file(&target, contents.as_bytes())?;
    let validation = Command::new("fail2ban-client")
        .arg("-t")
        .output()
        .map_err(|error| format!("cannot start fail2ban-client -t: {error}"));
    match validation {
        Ok(output) if output.status.success() => {}
        Ok(output) => {
            let rollback = restore_owned_file(&target, previous.as_deref(), contents.as_bytes());
            return Err(format!(
                "fail2ban-client -t rejected desired jail config: {}; {}",
                String::from_utf8_lossy(&output.stderr).trim(),
                rollback.map_or_else(
                    |error| format!("rollback failed: {error}"),
                    |_| "prior config state restored".to_owned()
                )
            ));
        }
        Err(error) => {
            let rollback = restore_owned_file(&target, previous.as_deref(), contents.as_bytes());
            return Err(format!(
                "{error}; {}",
                rollback.map_or_else(
                    |error| format!("rollback failed: {error}"),
                    |_| "prior config state restored".to_owned()
                )
            ));
        }
    }
    let service_result = run_systemctl(&["enable", "--now", "fail2ban"])
        .and_then(|()| run_systemctl(&["restart", "fail2ban"]));
    if let Err(error) = service_result {
        let rollback = restore_owned_file(&target, previous.as_deref(), contents.as_bytes());
        let restart_previous = run_systemctl(&["try-restart", "fail2ban"]);
        return Err(format!(
            "{error}; policy rollback: {}; previous service config restart: {}",
            rollback.err().unwrap_or_else(|| "restored".to_owned()),
            restart_previous.err().unwrap_or_else(|| "ok".to_owned())
        ));
    }
    Ok(())
}

fn ensure_real_directory(path: &Path) -> Result<(), String> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        if current.as_os_str().is_empty() {
            continue;
        }
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(format!(
                    "{} is a symlink; refusing to write through it",
                    current.display()
                ));
            }
            Ok(metadata) if !metadata.is_dir() => {
                return Err(format!("{} is not a directory", current.display()));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                match fs::create_dir(&current) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(error) => {
                        return Err(format!("cannot create {}: {error}", current.display()));
                    }
                }
                let metadata = fs::symlink_metadata(&current)
                    .map_err(|error| format!("cannot inspect {}: {error}", current.display()))?;
                if metadata.file_type().is_symlink() || !metadata.is_dir() {
                    return Err(format!(
                        "{} changed to a non-directory or symlink during creation",
                        current.display()
                    ));
                }
            }
            Err(error) => {
                return Err(format!("cannot inspect {}: {error}", current.display()));
            }
        }
    }
    Ok(())
}

fn read_owned_fail2ban_dropin(path: &Path) -> Result<Option<Vec<u8>>, String> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("cannot inspect {}: {error}", path.display())),
    };
    if metadata.file_type().is_symlink() {
        return Err(format!(
            "{} is a symlink; refusing to replace it",
            path.display()
        ));
    }
    let bytes =
        fs::read(path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    if !String::from_utf8_lossy(&bytes).contains(FAIL2BAN_DROPIN_MARKER) {
        return Err(format!(
            "{} exists but is not Mistborn-owned; refusing to overwrite it",
            path.display()
        ));
    }
    Ok(Some(bytes))
}

fn atomic_replace_owned_file(path: &Path, contents: &[u8]) -> Result<(), String> {
    atomic_replace_with_sync(path, contents, sync_parent_directory)
}

fn atomic_replace_with_sync(
    path: &Path,
    contents: &[u8],
    sync_parent: impl Fn(&Path) -> std::io::Result<()>,
) -> Result<(), String> {
    let parent = path.parent().ok_or("drop-in has no parent directory")?;
    let previous = match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(format!(
                "{} is a symlink; refusing to replace it",
                path.display()
            ));
        }
        Ok(metadata) if !metadata.is_file() => {
            return Err(format!("{} is not a regular file", path.display()));
        }
        Ok(_) => Some(
            fs::read(path).map_err(|error| format!("cannot read {}: {error}", path.display()))?,
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(format!("cannot inspect {}: {error}", path.display())),
    };
    let temporary = stage_fail2ban_file(parent, contents)?;
    fs::rename(&temporary, path).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        format!("cannot publish fail2ban drop-in: {error}")
    })?;
    if let Err(sync_error) = sync_parent(parent) {
        let rollback = rollback_published_file(path, previous.as_deref(), contents, &sync_parent);
        return Err(format!(
            "cannot sync {} after publishing drop-in: {sync_error}; {}",
            parent.display(),
            rollback.map_or_else(
                |error| format!("rollback failed: {error}"),
                |_| "prior file state restored".to_owned()
            )
        ));
    }
    Ok(())
}

fn stage_fail2ban_file(parent: &Path, contents: &[u8]) -> Result<PathBuf, String> {
    use std::io::Write;
    for attempt in 0..100 {
        let candidate = parent.join(format!(".mistborn-{}-{attempt}.tmp", std::process::id()));
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(mut file) => {
                use std::os::unix::fs::PermissionsExt;
                let staged = file
                    .write_all(contents)
                    .and_then(|()| file.set_permissions(fs::Permissions::from_mode(0o644)))
                    .and_then(|()| file.sync_all());
                if let Err(error) = staged {
                    drop(file);
                    let _ = fs::remove_file(&candidate);
                    return Err(format!("cannot stage fail2ban drop-in: {error}"));
                }
                return Ok(candidate);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(format!("cannot stage fail2ban drop-in: {error}")),
        }
    }
    Err("cannot allocate temporary fail2ban drop-in".to_owned())
}

fn rollback_published_file(
    path: &Path,
    previous: Option<&[u8]>,
    expected_current: &[u8],
    sync_parent: &impl Fn(&Path) -> std::io::Result<()>,
) -> Result<(), String> {
    let current = fs::read(path).map_err(|error| format!("cannot read published file: {error}"))?;
    if current != expected_current {
        return Err("drop-in changed concurrently; refusing to overwrite it".to_owned());
    }
    match previous {
        Some(contents) => {
            let parent = path.parent().ok_or("drop-in has no parent directory")?;
            let temporary = stage_fail2ban_file(parent, contents)?;
            fs::rename(&temporary, path).map_err(|error| {
                let _ = fs::remove_file(&temporary);
                format!("cannot restore previous drop-in: {error}")
            })?;
            sync_parent(parent)
                .map_err(|error| format!("cannot sync restored drop-in directory: {error}"))
        }
        None => {
            fs::remove_file(path)
                .map_err(|error| format!("cannot remove newly published drop-in: {error}"))?;
            let parent = path.parent().ok_or("drop-in has no parent directory")?;
            sync_parent(parent)
                .map_err(|error| format!("cannot sync removal of new drop-in: {error}"))
        }
    }
}

fn sync_parent_directory(parent: &Path) -> std::io::Result<()> {
    fs::File::open(parent).and_then(|directory| directory.sync_all())
}

fn restore_owned_file(
    path: &Path,
    previous: Option<&[u8]>,
    expected_current: &[u8],
) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("cannot inspect {} before rollback: {error}", path.display()))?;
    if metadata.file_type().is_symlink() {
        return Err(format!(
            "{} became a symlink; refusing rollback",
            path.display()
        ));
    }
    let current = fs::read(path)
        .map_err(|error| format!("cannot read {} before rollback: {error}", path.display()))?;
    if current != expected_current {
        return Err(format!(
            "{} changed during reconciliation; refusing to overwrite concurrent edits",
            path.display()
        ));
    }
    match previous {
        Some(contents) => atomic_replace_owned_file(path, contents),
        None => {
            fs::remove_file(path)
                .map_err(|error| format!("cannot remove failed new drop-in: {error}"))?;
            let parent = path.parent().ok_or("drop-in has no parent directory")?;
            sync_parent_directory(parent)
                .map_err(|error| format!("cannot sync removal of new drop-in: {error}"))
        }
    }
}

fn run_systemctl(args: &[&str]) -> Result<(), String> {
    let status = Command::new("systemctl")
        .args(args)
        .status()
        .map_err(|error| format!("cannot start systemctl: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "systemctl {} exited with {}",
            args.join(" "),
            status.code().unwrap_or(128)
        ))
    }
}

fn tailscale_set_args(
    remediation: mistborn_bootstrap::domain::RemediationId,
    config: &mistborn_bootstrap::config::TailscaleConfig,
) -> Vec<String> {
    use mistborn_bootstrap::domain::RemediationId;
    let flag = match remediation {
        RemediationId::SecurityTailscaleSsh => format!("--ssh={}", config.ssh),
        RemediationId::SecurityTailscaleExitNode => {
            format!("--advertise-exit-node={}", config.advertise_exit_node)
        }
        RemediationId::SecurityTailscaleAutoUpdate => {
            format!("--auto-update={}", config.auto_update)
        }
        _ => return Vec::new(),
    };
    vec!["set".to_owned(), flag]
}

fn apply_tailscale_policy(
    remediation: mistborn_bootstrap::domain::RemediationId,
    desired: &mistborn_bootstrap::config::DesiredState,
) -> Result<(), String> {
    let config = desired
        .tailscale
        .as_ref()
        .ok_or("Tailscale policy is unmanaged")?;
    let args = tailscale_set_args(remediation, config);
    if args.is_empty() {
        return Err(format!(
            "{} is not a Tailscale preference remediation",
            remediation.as_str()
        ));
    }
    let status = Command::new("tailscale")
        .args(&args)
        .status()
        .map_err(|error| format!("cannot start tailscale: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "tailscale {} exited with {}",
            args.join(" "),
            status.code().unwrap_or(128)
        ))
    }
}

fn run_ufw(args: &[&str]) -> Result<(), String> {
    let status = Command::new("ufw")
        .args(args)
        .status()
        .map_err(|error| format!("cannot start ufw: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "ufw {} exited with {}",
            args.join(" "),
            status.code().unwrap_or(128)
        ))
    }
}

fn apply_ufw_policy(desired: &mistborn_bootstrap::config::DesiredState) -> Result<(), String> {
    use mistborn_bootstrap::config::IncomingPolicy;
    let firewall = desired
        .firewall
        .as_ref()
        .ok_or("firewall policy is unmanaged")?;
    if firewall.enabled && desired.ssh.is_none() {
        return Err(
            "refusing to enable default-deny UFW without a managed SSH port; configure [ssh] first"
                .into(),
        );
    }
    // Preserve remote administration before changing the default incoming policy.
    if firewall.enabled {
        let ssh_port = desired
            .ssh
            .as_ref()
            .expect("checked above")
            .port
            .get()
            .to_string();
        run_ufw(&[
            "allow",
            &format!("{ssh_port}/tcp"),
            "comment",
            "mistborn:security/ufw:ssh",
        ])?;
        for port in &firewall.public_tcp_ports {
            let port = format!("{}/tcp", port.get());
            run_ufw(&["allow", &port, "comment", "mistborn:security/ufw:public"])?;
        }
    }
    let policy = match firewall.default_incoming {
        IncomingPolicy::Allow => "allow",
        IncomingPolicy::Deny => "deny",
        IncomingPolicy::Reject => "reject",
    };
    run_ufw(&["default", policy, "incoming"])?;
    if firewall.enabled {
        run_ufw(&["--force", "enable"])
    } else {
        run_ufw(&["--force", "disable"])
    }
}

fn install_owned_plex_profile(path: &Path) -> Result<(), String> {
    if path.exists() {
        let existing = fs::read_to_string(path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        if !existing.starts_with(host_diagnostics::PLEX_UFW_PROFILE_MARKER) {
            if host_diagnostics::plex_profile_is_expected(&existing) {
                return Ok(());
            }
            return Err(format!(
                "{} exists but is not marked as Mistborn-owned; refusing to replace it",
                path.display()
            ));
        }
    }
    let parent = path
        .parent()
        .ok_or("Plex profile has no parent directory")?;
    let file_name = path
        .file_name()
        .ok_or("Plex profile has no filename")?
        .to_string_lossy();
    let temporary = parent.join(format!(".{file_name}.mistborn-{}.tmp", std::process::id()));
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o644)
        .open(&temporary)
        .map_err(|error| format!("cannot create {}: {error}", temporary.display()))?;
    let result = (|| {
        file.write_all(host_diagnostics::PLEX_UFW_PROFILE.as_bytes())
            .map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
        fs::rename(&temporary, path).map_err(|error| error.to_string())?;
        File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| error.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result.map_err(|error| format!("cannot publish owned Plex profile: {error}"))
}

fn apply_plex_firewall(desired: &mistborn_bootstrap::config::DesiredState) -> Result<(), String> {
    let firewall = desired
        .firewall
        .as_ref()
        .ok_or("firewall policy is unmanaged")?;
    let plex = firewall
        .plex
        .as_ref()
        .ok_or("Plex firewall policy is unmanaged")?;
    if plex.enabled {
        install_owned_plex_profile(Path::new("/etc/ufw/applications.d/plexmediaserver"))?;
        run_ufw(&["app", "update", "plexmediaserver"])?;
        if plex.public_remote_access {
            run_ufw(&[
                "allow",
                "32400/tcp",
                "comment",
                "mistborn:security/plex-firewall:public",
            ])?;
        }
        if let Some(cidr) = &plex.lan_cidr {
            run_ufw(&[
                "allow",
                "from",
                cidr.as_str(),
                "to",
                "any",
                "app",
                "plexmediaserver-all",
                "comment",
                "mistborn:security/plex-firewall:lan",
            ])?;
        }
        if plex.tailscale {
            run_ufw(&[
                "allow",
                "in",
                "on",
                "tailscale0",
                "to",
                "any",
                "app",
                "plexmediaserver-all",
                "comment",
                "mistborn:security/plex-firewall:tailscale",
            ])?;
        }
        Ok(())
    } else {
        Err("Plex policy is disabled but this adapter never deletes firewall rules or profiles; remove Mistborn-owned Plex access rules explicitly after reviewing the plan".into())
    }
}

impl mistborn_bootstrap::reconciliation::ReconcileAdapter for SystemReconcileAdapter {
    fn inspect(
        &mut self,
    ) -> Result<mistborn_bootstrap::reconciliation::InspectionSnapshot, String> {
        let report = host_diagnostics::inspect(
            host_diagnostics::ReportKind::Doctor,
            &host_diagnostics::SystemCommands,
        );
        let observed = serde_json::to_value(&report.observed).map_err(|error| error.to_string())?;
        Ok(mistborn_bootstrap::reconciliation::InspectionSnapshot {
            observed,
            diagnostics: report.diagnostics,
        })
    }
    fn apply(&mut self, action: &mistborn_bootstrap::domain::ActionKind) -> Result<(), String> {
        use mistborn_bootstrap::domain::ActionKind;
        match action {
            ActionKind::InstallPackage { package } => {
                if !is_root() {
                    return Err("reconciliation actions require root".to_owned());
                }
                package_service_adapter::PackageServiceAdapter {
                    runner: package_service_adapter::ProcessRunner,
                }
                .install_package(*package)
            }
            ActionKind::EnableService { service } => {
                if !is_root() {
                    return Err("reconciliation actions require root".to_owned());
                }
                package_service_adapter::PackageServiceAdapter {
                    runner: package_service_adapter::ProcessRunner,
                }
                .enable_and_start(*service)
            }
            ActionKind::RestartService { .. } => {
                Err("restart actions are not available through safe reconciliation".into())
            }
            ActionKind::ApplyBoundedRemediation { remediation } => {
                if !is_root() {
                    return Err("reconciliation actions require root".to_owned());
                }
                apply_security_remediation(*remediation)
            }
        }
    }
    fn verify(
        &mut self,
        id: mistborn_bootstrap::domain::RemediationId,
    ) -> Result<(bool, Vec<String>), String> {
        let report = host_diagnostics::inspect(
            host_diagnostics::ReportKind::Doctor,
            &host_diagnostics::SystemCommands,
        );
        use mistborn_bootstrap::domain::RemediationId;
        if id == RemediationId::SecurityFail2ban {
            let active = verify_observed_fact(&report.observed, "service.fail2ban", "active")?;
            let enabled =
                verify_observed_fact(&report.observed, "service.fail2ban.enabled", "enabled")?;
            return Ok((
                active && enabled,
                vec![
                    format!("service.fail2ban.active={active}"),
                    format!("service.fail2ban.enabled={enabled}"),
                ],
            ));
        }
        let (fact, predicate) = match id {
            RemediationId::PackagesDocker => ("package.docker", "installed"),
            RemediationId::PackagesTailscale => ("package.tailscale", "installed"),
            RemediationId::PackagesUfw => ("package.ufw", "installed"),
            RemediationId::PackagesFail2banClient => ("package.fail2ban-client", "installed"),
            RemediationId::SecurityFail2banPolicy => ("fail2ban.policy", "compliant"),
            RemediationId::SecurityUfw => ("firewall.policy", "compliant"),
            RemediationId::SecurityPlexFirewall => ("plex.policy", "compliant"),
            RemediationId::SecuritySsh => {
                let desired = mistborn_bootstrap::config::DesiredState::load(Path::new(
                    "/etc/mistborn/config.toml",
                ))
                .map_err(|error| format!("cannot load desired configuration: {error}"))?;
                let ssh = desired
                    .ssh
                    .as_ref()
                    .ok_or("desired [ssh] section is missing")?;
                let effective = Command::new("sshd")
                    .arg("-T")
                    .output()
                    .map_err(|error| format!("cannot inspect effective SSH policy: {error}"))?;
                if !effective.status.success() {
                    return Err("sshd -T failed during SSH verification".to_owned());
                }
                let text = String::from_utf8_lossy(&effective.stdout);
                let verified = ssh_adapter::verify_effective(&text, ssh);
                return Ok((
                    verified.is_ok(),
                    vec![verified.err().unwrap_or_else(|| {
                        "sshd -T effective port/password/root policy matches".to_owned()
                    })],
                ));
            }
            RemediationId::SecurityTailscaleSsh => ("tailscale.ssh", "compliant"),
            RemediationId::SecurityTailscaleExitNode => ("tailscale.exit_node", "compliant"),
            RemediationId::SecurityTailscaleAutoUpdate => ("tailscale.auto_update", "compliant"),
            _ => return Err(format!("no verifier is registered for {}", id.as_str())),
        };
        let satisfied = verify_observed_fact(&report.observed, fact, predicate)?;
        Ok((satisfied, vec![format!("{fact}.{predicate}={satisfied}")]))
    }
}

fn verify_observed_fact(
    observed: &mistborn_bootstrap::domain::ObservedState,
    fact: &str,
    predicate: &str,
) -> Result<bool, String> {
    use mistborn_bootstrap::domain::InspectionStatus;
    match observed.facts.get(fact) {
        Some(InspectionStatus::Available(value)) => value
            .get(predicate)
            .and_then(serde_json::Value::as_bool)
            .ok_or_else(|| {
                format!(
                    "required verification predicate is missing or malformed: {fact}.{predicate}"
                )
            }),
        Some(InspectionStatus::Unavailable { reason }) => {
            Err(format!("{fact} unavailable: {reason}"))
        }
        Some(InspectionStatus::Error { message }) => {
            Err(format!("{fact} inspection failed: {message}"))
        }
        None => Err(format!("required verification fact is missing: {fact}")),
    }
}

fn is_root() -> bool {
    #[cfg(unix)]
    {
        unsafe extern "C" {
            fn geteuid() -> u32;
        }
        unsafe { geteuid() == 0 }
    }
    #[cfg(not(unix))]
    {
        false
    }
}

fn inspection_cli_error(message: &str) -> String {
    format!("exit-code-2: {message}")
}

fn status_drift_line(summary: &host_diagnostics::ReportSummary) -> String {
    format!("Drift count: {}", summary.drift_count)
}

fn status_config_line(config: &host_diagnostics::ConfigSummary) -> String {
    match config.schema_version {
        Some(version) => format!(
            "Config: {} (schema v{version}; {})",
            config.path, config.status
        ),
        None => format!("Config: {} ({})", config.path, config.status),
    }
}

fn confirmation_note(diagnostic: &mistborn_bootstrap::domain::Diagnostic) -> Option<&'static str> {
    diagnostic
        .confirmation_required
        .then_some("confirmation required")
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
            if let Some(message) = error.strip_prefix("exit-code-2: ") {
                eprintln!("mistborn-bootstrap: {message}");
                ExitCode::from(2)
            } else {
                eprintln!("mistborn-bootstrap: {error}");
                ExitCode::FAILURE
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEMP_DIRECTORY_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temporary_directory() -> PathBuf {
        let path = env::temp_dir().join(format!(
            "mistborn-bootstrap-test-{}-{}",
            run_id(),
            TEMP_DIRECTORY_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        path
    }

    #[test]
    fn plex_profile_refuses_to_replace_unowned_file_and_publishes_owned_profile_atomically() {
        let directory = temporary_directory();
        let path = directory.join("plexmediaserver");
        fs::write(&path, "# administrator profile\n").unwrap();
        assert!(
            install_owned_plex_profile(&path)
                .unwrap_err()
                .contains("not marked as Mistborn-owned")
        );
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "# administrator profile\n"
        );
        let unowned_expected_profile = host_diagnostics::PLEX_UFW_PROFILE
            .strip_prefix(host_diagnostics::PLEX_UFW_PROFILE_MARKER)
            .unwrap()
            .trim_start_matches('\n');
        fs::write(&path, unowned_expected_profile).unwrap();
        install_owned_plex_profile(&path).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), unowned_expected_profile);
        fs::write(
            &path,
            format!(
                "{}\nold managed profile\n",
                host_diagnostics::PLEX_UFW_PROFILE_MARKER
            ),
        )
        .unwrap();
        install_owned_plex_profile(&path).unwrap();
        let contents = fs::read_to_string(&path).unwrap();
        assert!(contents.starts_with(host_diagnostics::PLEX_UFW_PROFILE_MARKER));
        assert!(contents.contains("ports=32400/tcp"));
        assert!(
            !directory
                .join(format!(
                    ".plexmediaserver.mistborn-{}.tmp",
                    std::process::id()
                ))
                .exists()
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn ufw_enable_refuses_when_ssh_is_unmanaged_before_running_any_command() {
        let desired = mistborn_bootstrap::config::DesiredState::from_toml(
            "version=1\nprofile='vps'\n[firewall]\nenabled=true\ndefault_incoming='deny'\npublic_tcp_ports=[80,443]\n",
        )
        .unwrap();
        let error = apply_ufw_policy(&desired).unwrap_err();
        assert!(error.contains("without a managed SSH port"));
    }

    #[test]
    fn status_routes_to_rust_before_legacy_script_lookup() {
        let error = execute_host(&[
            "host".to_owned(),
            "--script".to_owned(),
            "/missing/legacy/script".to_owned(),
            "status".to_owned(),
            "--format".to_owned(),
            "yaml".to_owned(),
        ])
        .unwrap_err();
        assert!(error.contains("exit-code-2: unsupported output format"));
        assert!(!error.contains("host command script not found"));
    }

    #[test]
    fn read_only_cli_argument_errors_use_exit_code_two() {
        let error = execute_host(&[
            "host".to_owned(),
            "--script".to_owned(),
            "/missing/legacy/script".to_owned(),
            "doctor".to_owned(),
            "--unknown".to_owned(),
        ])
        .unwrap_err();
        assert!(error.starts_with("exit-code-2: unknown doctor option"));
    }

    #[test]
    fn plan_routes_to_rust_before_legacy_script_lookup() {
        let error = execute_host(&[
            "host".to_owned(),
            "--script".to_owned(),
            "/missing/legacy/script".to_owned(),
            "plan".to_owned(),
            "--format".to_owned(),
            "yaml".to_owned(),
        ])
        .unwrap_err();
        assert!(error.starts_with("exit-code-2: unsupported output format"));
        assert!(!error.contains("host command script not found"));
    }

    #[test]
    fn reconcile_rejects_unknown_options_before_host_mutation() {
        let error = execute_host(&[
            "host".to_owned(),
            "--script".to_owned(),
            "/missing/legacy/script".to_owned(),
            "reconcile".to_owned(),
            "--unexpected".to_owned(),
        ])
        .unwrap_err();
        assert!(error.starts_with("exit-code-2: unknown reconcile option"));
        assert!(!error.contains("host command script not found"));
    }

    #[test]
    fn mutating_json_output_is_rejected_before_host_inspection() {
        for arguments in [
            vec![
                "reconcile".to_owned(),
                "--format".to_owned(),
                "json".to_owned(),
            ],
            vec![
                "doctor".to_owned(),
                "--fix".to_owned(),
                "--format".to_owned(),
                "json".to_owned(),
            ],
        ] {
            let error = execute_host(
                &[
                    "host".to_owned(),
                    "--script".to_owned(),
                    "/missing/legacy/script".to_owned(),
                ]
                .into_iter()
                .chain(arguments)
                .collect::<Vec<_>>(),
            )
            .unwrap_err();
            assert!(error.contains("--format json is not supported for mutating commands"));
            assert!(!error.contains("host command script not found"));
        }
    }

    #[test]
    fn verifier_requires_positive_available_fact() {
        use mistborn_bootstrap::domain::{InspectionStatus, ObservedState};
        let mut observed = ObservedState::default();
        observed.facts.insert(
            "service.fail2ban".into(),
            InspectionStatus::Available(serde_json::json!({"active": true})),
        );
        assert!(verify_observed_fact(&observed, "service.fail2ban", "active").unwrap());
        observed.facts.insert(
            "service.fail2ban".into(),
            InspectionStatus::Available(serde_json::json!({"active": false})),
        );
        assert!(!verify_observed_fact(&observed, "service.fail2ban", "active").unwrap());
        observed.facts.insert(
            "service.fail2ban".into(),
            InspectionStatus::Unavailable {
                reason: "systemd unavailable".into(),
            },
        );
        assert!(verify_observed_fact(&observed, "service.fail2ban", "active").is_err());
        observed.facts.insert(
            "service.fail2ban".into(),
            InspectionStatus::Error {
                message: "permission denied".into(),
            },
        );
        assert!(verify_observed_fact(&observed, "service.fail2ban", "active").is_err());
        observed.facts.insert(
            "service.fail2ban".into(),
            InspectionStatus::Available(serde_json::json!({})),
        );
        assert!(verify_observed_fact(&observed, "service.fail2ban", "active").is_err());
    }

    #[test]
    fn missing_docker_plans_and_safe_reconciles_despite_unrelated_unavailability() {
        use mistborn_bootstrap::domain::{
            ActionKind, CommandAdapter, CommandOutput, InspectionStatus,
        };
        use mistborn_bootstrap::reconciliation::{
            ApplyOptions, InspectionSnapshot, ReconcileAdapter,
        };
        struct MissingPackages;
        impl CommandAdapter for MissingPackages {
            fn run(&self, _: &str, _: &[&str]) -> Result<CommandOutput, String> {
                Err("No such file or directory".into())
            }
        }
        struct FakeReconciler {
            snapshot: InspectionSnapshot,
            applied: Vec<ActionKind>,
        }
        impl ReconcileAdapter for FakeReconciler {
            fn inspect(&mut self) -> Result<InspectionSnapshot, String> {
                Ok(self.snapshot.clone())
            }
            fn apply(&mut self, action: &ActionKind) -> Result<(), String> {
                self.applied.push(action.clone());
                self.snapshot.observed["facts"]["package.docker"] =
                    serde_json::json!({"available": {"installed": true}});
                Ok(())
            }
            fn verify(
                &mut self,
                id: mistborn_bootstrap::domain::RemediationId,
            ) -> Result<(bool, Vec<String>), String> {
                if id != mistborn_bootstrap::domain::RemediationId::PackagesDocker {
                    return Err("unexpected target".into());
                }
                let observed: mistborn_bootstrap::domain::ObservedState =
                    serde_json::from_value(self.snapshot.observed.clone())
                        .map_err(|error| error.to_string())?;
                let installed = verify_observed_fact(&observed, "package.docker", "installed")?;
                Ok((installed, vec!["package.docker.installed=true".into()]))
            }
        }

        let report = host_diagnostics::inspect_with_package_lookup(
            host_diagnostics::ReportKind::Doctor,
            &MissingPackages,
            |name| (name != "docker").then(|| std::path::PathBuf::from("/usr/bin").join(name)),
        );
        assert!(!report.inspection_incomplete);
        let mut observed = serde_json::to_value(&report.observed).unwrap();
        observed["facts"]["unrelated.permission"] =
            serde_json::json!({"error": {"message": "permission denied"}});
        let proposed =
            mistborn_bootstrap::reconciliation::plan(&observed, &report.diagnostics, None).unwrap();
        assert_eq!(
            proposed
                .remediations
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            vec!["packages/docker"]
        );
        assert!(proposed.remediations[0].available);
        let mut fake = FakeReconciler {
            snapshot: InspectionSnapshot {
                observed,
                diagnostics: report.diagnostics.clone(),
            },
            applied: Vec::new(),
        };
        let history = temporary_directory().join("reconciliation.json");
        let results = mistborn_bootstrap::reconciliation::reconcile(
            &proposed,
            &report.diagnostics,
            &mut fake,
            &history,
            &ApplyOptions {
                safe: true,
                non_interactive: true,
                ..ApplyOptions::default()
            },
        )
        .unwrap();
        assert_eq!(
            fake.applied,
            vec![ActionKind::InstallPackage {
                package: mistborn_bootstrap::domain::PackageName::Docker
            }]
        );
        assert!(results[0].verified);
        assert!(
            matches!(report.observed.facts.get("package.docker"), Some(InspectionStatus::Available(value)) if value["installed"] == false)
        );
    }

    #[test]
    fn doctor_confirmation_note_and_status_drift_line_are_stable() {
        let diagnostic = mistborn_bootstrap::domain::Diagnostic {
            id: "security/ssh".to_owned(),
            severity: mistborn_bootstrap::domain::DiagnosticSeverity::Fail,
            summary: "SSH drift".to_owned(),
            evidence: vec![],
            remediation_id: Some("security/ssh".to_owned()),
            risk: Some(mistborn_bootstrap::domain::RiskClass::Access),
            confirmation_required: true,
        };
        assert_eq!(
            confirmation_note(&diagnostic),
            Some("confirmation required")
        );
        assert_eq!(
            status_drift_line(&host_diagnostics::ReportSummary {
                drift_count: 3,
                ..Default::default()
            }),
            "Drift count: 3"
        );
        assert_eq!(
            status_config_line(&host_diagnostics::ConfigSummary {
                path: "/etc/mistborn/config.toml",
                schema_version: Some(1),
                status: "loaded",
            }),
            "Config: /etc/mistborn/config.toml (schema v1; loaded)"
        );
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

    #[test]
    fn tailscale_reconcile_uses_one_flag_set_argv_without_enrollment() {
        let config = mistborn_bootstrap::config::TailscaleConfig {
            ssh: true,
            advertise_exit_node: false,
            auto_update: true,
        };
        use mistborn_bootstrap::domain::RemediationId;
        let argv = tailscale_set_args(RemediationId::SecurityTailscaleSsh, &config);
        assert_eq!(argv, ["set", "--ssh=true"]);
        assert_eq!(
            tailscale_set_args(RemediationId::SecurityTailscaleExitNode, &config),
            ["set", "--advertise-exit-node=false"]
        );
        assert_eq!(
            tailscale_set_args(RemediationId::SecurityTailscaleAutoUpdate, &config),
            ["set", "--auto-update=true"]
        );
        assert!(
            !argv
                .iter()
                .any(|arg| arg == "up" || arg.contains("authkey"))
        );
    }

    #[test]
    fn fail2ban_dropin_uses_managed_ssh_port_and_owns_only_its_file() {
        let desired = mistborn_bootstrap::config::DesiredState::from_toml(
            "version=1\nprofile='vps'\n[ssh]\nport=2222\npassword_authentication=false\nroot_login=false\n[fail2ban]\nenabled=true\n[fail2ban.sshd]\nenabled=true\nmaxretry=4\nbantime=7200\n",
        ).unwrap();
        let contents = render_fail2ban_dropin(
            desired.fail2ban.as_ref().unwrap(),
            fail2ban_policy_port(&desired).unwrap(),
        )
        .unwrap();
        assert!(contents.contains("port = 2222"));
        assert!(contents.contains("maxretry = 4"));
        assert!(contents.contains("bantime = 7200"));
        assert_eq!(parse_effective_ssh_port("port 2200\n"), Ok(2200));
        assert!(parse_effective_ssh_port("port 22\nport 2222\n").is_err());

        let directory = temporary_directory();
        fs::create_dir_all(&directory).unwrap();
        let target = directory.join("99-mistborn-bootstrap.local");
        assert_eq!(read_owned_fail2ban_dropin(&target).unwrap(), None);
        assert!(atomic_replace_owned_file(&target, contents.as_bytes()).is_ok());
        assert_eq!(fs::read_to_string(&target).unwrap(), contents);
        assert!(restore_owned_file(&target, None, contents.as_bytes()).is_ok());
        assert!(!target.exists());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn fail2ban_dropin_refuses_disabled_policy_and_unowned_replacement() {
        let disabled = mistborn_bootstrap::config::DesiredState::from_toml(
            "version=1\nprofile='vps'\n[fail2ban]\nenabled=true\n[fail2ban.sshd]\nenabled=false\nmaxretry=3\nbantime=3600\n",
        ).unwrap();
        assert!(render_fail2ban_dropin(disabled.fail2ban.as_ref().unwrap(), 22).is_err());
        let directory = temporary_directory();
        fs::create_dir_all(&directory).unwrap();
        let target = directory.join("99-mistborn-bootstrap.local");
        fs::write(&target, "operator-owned\n").unwrap();
        let before = fs::read(&target).unwrap();
        assert!(read_owned_fail2ban_dropin(&target).is_err());
        assert_eq!(fs::read(&target).unwrap(), before);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn fail2ban_rollback_restores_only_unchanged_owned_contents() {
        let directory = temporary_directory();
        fs::create_dir_all(&directory).unwrap();
        let target = directory.join("99-mistborn-bootstrap.local");
        let previous = format!("{FAIL2BAN_DROPIN_MARKER}\n[sshd]\nenabled=false\n");
        let desired = format!("{FAIL2BAN_DROPIN_MARKER}\n[sshd]\nenabled=true\n");
        fs::write(&target, &previous).unwrap();
        let before = read_owned_fail2ban_dropin(&target).unwrap();
        atomic_replace_owned_file(&target, desired.as_bytes()).unwrap();
        restore_owned_file(&target, before.as_deref(), desired.as_bytes()).unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), previous);

        fs::write(&target, "operator concurrent edit\n").unwrap();
        assert!(restore_owned_file(&target, None, desired.as_bytes()).is_err());
        assert_eq!(
            fs::read_to_string(&target).unwrap(),
            "operator concurrent edit\n"
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn atomic_dropin_directory_sync_failure_restores_previous_file() {
        use std::cell::Cell;
        let directory = temporary_directory();
        let target = directory.join("99-mistborn-bootstrap.local");
        let previous = b"# Managed by Mistborn: security/fail2ban-policy\nold\n";
        let desired = b"# Managed by Mistborn: security/fail2ban-policy\nnew\n";
        fs::write(&target, previous).unwrap();
        let calls = Cell::new(0);
        let result = atomic_replace_with_sync(&target, desired, |_| {
            if calls.get() == 0 {
                calls.set(1);
                Err(std::io::Error::other("injected fsync failure"))
            } else {
                Ok(())
            }
        });
        let error = result.unwrap_err();
        assert!(error.contains("injected fsync failure"));
        assert!(error.contains("prior file state restored"));
        assert_eq!(fs::read(&target).unwrap(), previous);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn atomic_dropin_surfaces_restoration_sync_failure() {
        let directory = temporary_directory();
        let target = directory.join("99-mistborn-bootstrap.local");
        let previous = b"# Managed by Mistborn: security/fail2ban-policy\nold\n";
        let desired = b"# Managed by Mistborn: security/fail2ban-policy\nnew\n";
        fs::write(&target, previous).unwrap();
        let result = atomic_replace_with_sync(&target, desired, |_| {
            Err(std::io::Error::other("injected persistent fsync failure"))
        });
        let error = result.unwrap_err();
        assert!(error.contains("rollback failed"));
        assert!(error.contains("injected persistent fsync failure"));
        // The bytes are restored even though the directory entry could not be durably synced.
        assert_eq!(fs::read(&target).unwrap(), previous);
        fs::remove_dir_all(directory).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn fail2ban_dropin_refuses_symlinked_directory_and_target() {
        use std::os::unix::fs::symlink;
        let base = fs::canonicalize(temporary_directory()).unwrap();
        let real = base.join("real");
        let redirected = base.join("jail.d");
        fs::create_dir_all(&real).unwrap();
        symlink(&real, &redirected).unwrap();
        assert!(ensure_real_directory(&redirected).is_err());

        let etc = base.join("etc");
        fs::create_dir(&etc).unwrap();
        let fail2ban_link = etc.join("fail2ban");
        symlink(&real, &fail2ban_link).unwrap();
        assert!(ensure_real_directory(&fail2ban_link.join("jail.d")).is_err());

        let link = real.join("99-mistborn-bootstrap.local");
        symlink(base.join("outside"), &link).unwrap();
        assert!(read_owned_fail2ban_dropin(&link).is_err());
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn tailscale_verifier_requires_available_matching_preferences() {
        use mistborn_bootstrap::domain::{InspectionStatus, ObservedState, RemediationId};
        let mut observed = ObservedState::default();
        observed.facts.insert(
            "tailscale.ssh".to_owned(),
            InspectionStatus::Available(serde_json::json!({"compliant": true})),
        );
        assert!(verify_observed_fact(&observed, "tailscale.ssh", "compliant").unwrap());
        observed.facts.insert(
            "tailscale.ssh".to_owned(),
            InspectionStatus::Available(serde_json::json!({"compliant": false})),
        );
        assert!(!verify_observed_fact(&observed, "tailscale.ssh", "compliant").unwrap());
        observed.facts.insert(
            "tailscale.ssh".to_owned(),
            InspectionStatus::Error {
                message: "daemon inaccessible".to_owned(),
            },
        );
        assert!(
            verify_observed_fact(&observed, "tailscale.ssh", "compliant")
                .unwrap_err()
                .contains("inaccessible")
        );
        assert_eq!(
            RemediationId::SecurityTailscaleSsh.as_str(),
            "security/tailscale-ssh"
        );
    }
}
