use std::fmt;

use super::super::domain::*;
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CoreError {
    InvalidRequest(String),
    SessionExists(SessionId),
    SessionNotFound(SessionId),
    RevisionConflict { expected: u64, actual: u64 },
    InvalidPhase(String),
    QuestionMismatch,
    ModelActionMismatch,
    ProposalSchemaInvalid(String),
    ProposalBaseMismatch,
    InvalidSourceReference,
    BlockersPresent,
    CandidateNotFound(CandidateId),
    CandidateStale,
    ApprovalBindingMismatch,
    Invariant(String),
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRequest(message) => write!(f, "invalid request: {message}"),
            Self::SessionExists(id) => write!(f, "session already exists: {id}"),
            Self::SessionNotFound(id) => write!(f, "session not found: {id}"),
            Self::RevisionConflict { expected, actual } => {
                write!(
                    f,
                    "expected revision {expected}, current revision is {actual}"
                )
            }
            Self::InvalidPhase(message) => write!(f, "invalid phase: {message}"),
            Self::QuestionMismatch => write!(f, "question does not match pending question"),
            Self::ModelActionMismatch => write!(f, "model work item does not match current action"),
            Self::ProposalSchemaInvalid(message) => write!(f, "proposal schema invalid: {message}"),
            Self::ProposalBaseMismatch => write!(f, "proposal base does not match current state"),
            Self::InvalidSourceReference => write!(f, "invalid source reference"),
            Self::BlockersPresent => write!(f, "blocking blockers are present"),
            Self::CandidateNotFound(id) => write!(f, "candidate not found: {id}"),
            Self::CandidateStale => write!(f, "candidate is stale"),
            Self::ApprovalBindingMismatch => write!(f, "approval binding does not match candidate"),
            Self::Invariant(message) => write!(f, "state invariant failed: {message}"),
        }
    }
}

impl std::error::Error for CoreError {}
