#[path = "domain/entity.rs"]
mod entity;
#[path = "domain/event.rs"]
mod event;
#[path = "domain/proposal.rs"]
mod proposal;
#[path = "domain/state.rs"]
mod state;

pub use entity::*;
pub use event::*;
pub use proposal::*;
pub use state::*;
