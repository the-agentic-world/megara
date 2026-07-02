#![allow(dead_code)]

#[path = "../../src/cli.rs"]
mod cli;
#[path = "../../src/doctor.rs"]
mod doctor;
#[path = "../../src/hook.rs"]
mod hook;
#[path = "../../src/installer.rs"]
mod installer;
#[path = "../../src/paths.rs"]
mod paths;
#[path = "../../src/targets.rs"]
mod targets;
#[path = "../../src/templates.rs"]
mod templates;
#[path = "../../src/ultragoal.rs"]
mod ultragoal;
#[path = "../../src/writer.rs"]
mod writer;

pub(crate) use hook::*;
pub(crate) use installer::{PlannedFile, MANAGED_MARKER};
pub(crate) use serde_json::{json, Value};
pub(crate) use std::{fs, path::Path};
pub(crate) use ultragoal::*;
pub(crate) use writer::*;

#[path = "hook.rs"]
mod hook_tests;
#[path = "ultragoal.rs"]
mod ultragoal_tests;
#[path = "writer.rs"]
mod writer_tests;
