#[path = "installer/marker.rs"]
mod marker;
#[path = "installer/migration.rs"]
mod migration;
#[path = "installer/model.rs"]
mod model;
#[path = "installer/planner.rs"]
mod planner;
#[path = "installer/print.rs"]
mod print;

pub use marker::{strip_managed_marker, MANAGED_MARKER};
#[allow(unused_imports)]
pub use model::{
    DoctorOptions, InstallAction, InstallOptions, InstallPlan, InstallResult, PlannedFile,
};
#[allow(unused_imports)]
pub(crate) use planner::runtime_support_files;
#[allow(unused_imports)]
pub use planner::Planner;
