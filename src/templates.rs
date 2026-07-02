#[path = "templates/model.rs"]
mod model;
#[path = "templates/registry.rs"]
mod registry;
#[path = "templates/specs.rs"]
mod specs;

pub use model::HarnessTemplate;
pub use registry::TemplateRegistry;
