use super::*;

pub(super) fn run_workflow_event(
    state_dir: &Path,
    timestamp: &str,
    options: &HookOptions,
    payload: &Value,
    payload_file: &Path,
) -> Result<i32> {
    match options.event.as_str() {
        "Stop" => stop::handle_stop(timestamp, state_dir, payload, payload_file),
        "UserPromptSubmit" => {
            user_prompt::handle_user_prompt(timestamp, state_dir, payload, payload_file)
        }
        "PreToolUse" => pre_tool::handle_pre_tool_use(timestamp, state_dir, payload, payload_file),
        _ => Ok(0),
    }
}
