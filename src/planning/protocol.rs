#[path = "protocol/limits.rs"]
mod limits;
#[path = "protocol/params.rs"]
mod params;
#[path = "protocol/projection.rs"]
mod projection;
#[path = "protocol/request.rs"]
mod request;
#[path = "protocol/state.rs"]
pub(crate) mod state;

pub use super::evidence::{EvidenceCitationRequest, EVIDENCE_CITATIONS_SCHEMA};
pub(crate) use limits::validate_wire_value;
pub use limits::{decode_jsonl_frame, encode_jsonl, MAX_JSONL_FRAME_BYTES};
pub use projection::{project_question, QuestionProjectionBlock};
pub use request::{
    supported_operations, LogicalRequest, OperationKind, ProtocolError, LOGICAL_OPERATIONS,
    PROTOCOL_VERSION, RESULT_SCHEMA,
};
