use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct Plan {
    pub version: u8,
    pub collection: String,
    pub stages: Vec<Stage>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Stage {
    pub id: String,
    pub title: String,
    pub help: String,
    #[serde(default)]
    pub tasks: Vec<Task>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub action: String,
    #[serde(default = "default_weight")]
    pub weight: u32,
    #[serde(default = "default_revision")]
    pub revision: u32,
    #[serde(default)]
    pub inputs: Vec<String>,
    #[serde(default)]
    pub requires_confirmation: bool,
}

fn default_weight() -> u32 {
    1
}

fn default_revision() -> u32 {
    1
}

pub fn load(path: &Path, expected_collection: &str) -> Result<Plan, String> {
    let source = fs::read_to_string(path)
        .map_err(|error| format!("cannot open {}: {error}", path.display()))?;
    let plan: Plan = toml::from_str(&source).map_err(|error| format!("invalid plan: {error}"))?;
    if !(1..=2).contains(&plan.version) || plan.collection != expected_collection {
        return Err(format!(
            "plan {} has incompatible version or collection",
            path.display()
        ));
    }
    if plan.stages.is_empty() {
        return Err(format!("plan {} has no stages", path.display()));
    }
    let mut stage_ids = std::collections::BTreeSet::new();
    for stage in &plan.stages {
        if stage.id.is_empty() || !stage_ids.insert(stage.id.clone()) {
            return Err(format!(
                "plan {} has an empty or duplicate stage id",
                path.display()
            ));
        }
        let mut task_ids = std::collections::BTreeSet::new();
        for task in &stage.tasks {
            if task.id.is_empty()
                || task.action.is_empty()
                || task.weight == 0
                || task.revision == 0
                || !task_ids.insert(task.id.clone())
            {
                return Err(format!(
                    "stage {} has an invalid or duplicate task",
                    stage.id
                ));
            }
        }
    }
    Ok(plan)
}
