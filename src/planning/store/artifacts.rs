use super::super::engine::{
    ApprovalCommand, CoreError, PlanCandidateCommand, RevisionRequestCommand, SpecCandidateCommand,
};
use super::transaction::{CoreOutcome, EventContext};
use super::{PlanningStore, StoreError, StoredOutcome};

impl PlanningStore {
    pub(crate) fn record_noop(
        &mut self,
        command_id: &str,
        request_hash: &str,
        session_id: &str,
    ) -> Result<StoredOutcome, StoreError> {
        self.execute_with_context(
            command_id,
            request_hash,
            session_id,
            move |core| {
                let state = core
                    .state(session_id)
                    .cloned()
                    .ok_or_else(|| CoreError::SessionNotFound(session_id.to_string()))?;
                Ok(CoreOutcome::Unchanged { state })
            },
            EventContext::default(),
        )
    }

    pub(crate) fn generate_spec_with_context(
        &mut self,
        command_id: &str,
        request_hash: &str,
        command: SpecCandidateCommand,
        context: EventContext,
    ) -> Result<StoredOutcome, StoreError> {
        let session_id = command.session_id.clone();
        self.execute_with_context(
            command_id,
            request_hash,
            &session_id,
            move |core| core.generate_spec(command).map(CoreOutcome::Changed),
            context,
        )
    }

    pub(crate) fn approve_spec_with_context(
        &mut self,
        command_id: &str,
        request_hash: &str,
        command: ApprovalCommand,
        context: EventContext,
    ) -> Result<StoredOutcome, StoreError> {
        let session_id = command.session_id.clone();
        self.execute_with_context(
            command_id,
            request_hash,
            &session_id,
            move |core| core.approve_spec(command).map(CoreOutcome::Changed),
            context,
        )
    }

    pub(crate) fn revise_spec_with_context(
        &mut self,
        command_id: &str,
        request_hash: &str,
        command: RevisionRequestCommand,
        context: EventContext,
    ) -> Result<StoredOutcome, StoreError> {
        let session_id = command.session_id.clone();
        self.execute_with_context(
            command_id,
            request_hash,
            &session_id,
            move |core| core.revise_spec(command).map(CoreOutcome::Changed),
            context,
        )
    }

    pub(crate) fn generate_plan_with_context(
        &mut self,
        command_id: &str,
        request_hash: &str,
        command: PlanCandidateCommand,
        context: EventContext,
    ) -> Result<StoredOutcome, StoreError> {
        let session_id = command.session_id.clone();
        self.execute_with_context(
            command_id,
            request_hash,
            &session_id,
            move |core| core.generate_plan(command).map(CoreOutcome::Changed),
            context,
        )
    }

    pub(crate) fn approve_plan_with_context(
        &mut self,
        command_id: &str,
        request_hash: &str,
        command: ApprovalCommand,
        context: EventContext,
    ) -> Result<StoredOutcome, StoreError> {
        let session_id = command.session_id.clone();
        self.execute_with_context(
            command_id,
            request_hash,
            &session_id,
            move |core| core.approve_plan(command).map(CoreOutcome::Changed),
            context,
        )
    }

    pub(crate) fn revise_plan_with_context(
        &mut self,
        command_id: &str,
        request_hash: &str,
        command: RevisionRequestCommand,
        context: EventContext,
    ) -> Result<StoredOutcome, StoreError> {
        let session_id = command.session_id.clone();
        self.execute_with_context(
            command_id,
            request_hash,
            &session_id,
            move |core| core.revise_plan(command).map(CoreOutcome::Changed),
            context,
        )
    }
}
