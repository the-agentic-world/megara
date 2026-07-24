use super::*;

pub(super) fn handle_post_tool_use(
    timestamp: &str,
    state_dir: &Path,
    payload: &Value,
    payload_file: &Path,
) -> Result<i32> {
    ultragoal_continuation::record_completed_checkpoint(
        timestamp,
        state_dir,
        payload,
        payload_file,
    )?;
    Ok(0)
}
