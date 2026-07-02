#[path = "parser/block.rs"]
mod block;
#[path = "parser/gate.rs"]
mod gate;
#[path = "parser/question.rs"]
mod question;
#[path = "parser/review.rs"]
mod review;
#[path = "parser/terminal.rs"]
mod terminal;
#[path = "parser/text.rs"]
mod text;

pub(crate) use block::{block_list, parse_block, parse_blocks, Block};
pub(crate) use gate::{approval_gate_from_text, plan_gate_from_text, ApprovalGate, PlanGate};
pub(crate) use question::question_from_text;
pub(crate) use review::{review_passes_from_text, ReviewPass};
pub(crate) use terminal::{workflow_state_from_text, TerminalState};
pub(crate) use text::{text_before_block, text_before_first_workflow_block};
