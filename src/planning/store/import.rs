use rusqlite::{params, OptionalExtension, TransactionBehavior};

use super::super::engine::{CoreError, InMemoryPlanningCore, LegacyImportCommand};
use super::command_cache::{check_idempotency, persist_command_result};
use super::persistence::{insert_event, upsert_session};
use super::transaction::EventContext;
use super::{PlanningStore, StoreError, StoredOutcome};

impl PlanningStore {
    pub(crate) fn import_legacy_with_context(
        &mut self,
        command_id: &str,
        request_hash: &str,
        command: LegacyImportCommand,
        context: EventContext,
    ) -> Result<StoredOutcome, StoreError> {
        super::transaction::validate_command_identity(command_id, request_hash)?;
        if command.project_id != self.identity.project_id {
            return Err(StoreError::ProjectIdMismatch {
                expected: self.identity.project_id.clone(),
                actual: command.project_id,
            });
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(outcome) = check_idempotency(&tx, command_id, request_hash)? {
            tx.commit()?;
            return Ok(outcome);
        }
        if tx
            .query_row(
                "SELECT 1 FROM sessions WHERE session_id=?1",
                params![&command.session_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .is_some()
        {
            return Err(StoreError::Core(CoreError::SessionExists(
                command.session_id,
            )));
        }
        let mut core = InMemoryPlanningCore::default();
        let result = core.import_legacy(command)?;
        let state = result.state.clone();
        upsert_session(&tx, &state)?;
        let event_id = insert_event(&tx, command_id, &result.event, &state, &context)?;
        persist_command_result(
            &tx,
            command_id,
            &state,
            Some(&result.event),
            Some(&event_id),
            request_hash,
        )?;
        tx.commit()?;
        Ok(StoredOutcome {
            state,
            event: Some(result.event),
            replayed: false,
        })
    }
}
