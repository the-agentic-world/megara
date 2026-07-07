use std::{
    env, fs,
    io::{self, Read},
    path::Path,
};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[path = "ultragoal/brief.rs"]
mod brief;
#[path = "ultragoal/checkpoint.rs"]
mod checkpoint;
#[path = "ultragoal/checkpoint_ledger.rs"]
mod checkpoint_ledger;
#[path = "ultragoal/complete.rs"]
mod complete;
#[path = "ultragoal/create.rs"]
mod create;
#[path = "ultragoal/flow.rs"]
mod flow;
#[path = "ultragoal/goals.rs"]
mod goals;
#[path = "ultragoal/model.rs"]
mod model;
#[path = "ultragoal/quality.rs"]
mod quality;
#[path = "ultragoal/quality_refs.rs"]
mod quality_refs;
#[path = "ultragoal/receipt.rs"]
mod receipt;
#[path = "ultragoal/status.rs"]
mod status;
#[path = "ultragoal/steer.rs"]
mod steer;
#[path = "ultragoal/store.rs"]
mod store;
#[path = "ultragoal/util.rs"]
mod util;

pub(crate) use goals::parse_goals;
pub(crate) use quality::{read_quality_gate, validate_quality_gate};

use crate::cli::{UltragoalArgs, UltragoalCommands};
use store::UltragoalPaths;

const WORKFLOW: &str = "ultragoal";
const RALPLAN: &str = "ralplan";

pub fn run(args: UltragoalArgs) -> Result<()> {
    let paths = UltragoalPaths::resolve(args.scope, &args.session_id)?;
    match args.command {
        UltragoalCommands::Status(command) => status::run(&paths, &args.session_id, command),
        UltragoalCommands::CreateGoals(command) => {
            create::run(&paths, args.scope, &args.session_id, command)
        }
        UltragoalCommands::StartGoal(command) => complete::run(&paths, command),
        UltragoalCommands::Checkpoint(command) => checkpoint::run(&paths, command),
        UltragoalCommands::Steer(command) => steer::run(&paths, command),
    }
}
