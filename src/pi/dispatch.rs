use std::path::Path;

use anyhow::{bail, Result};

use crate::{paths::InstallScope, targets::pi, templates::TemplateRegistry};

use super::{
    protocol::{PiAction, PiEventRequest, PiEventResponse},
    receipt::{self, AttemptReceipt, EventReceipt},
};

pub fn dispatch(
    request: PiEventRequest,
    scope: InstallScope,
    project_root: &Path,
    runtime_root: &Path,
    registry: &TemplateRegistry,
) -> Result<PiEventResponse> {
    if request.protocol_version != 1 {
        bail!(
            "unsupported Pi protocol version {}; expected 1",
            request.protocol_version
        );
    }
    if request.event_id.trim().is_empty() || request.workflow.trim().is_empty() {
        bail!("Pi event_id and workflow are required");
    }
    match request.action {
        PiAction::Activate => receipt::with_lock(runtime_root, &request.event_id, || {
            activate(&request, scope, project_root, runtime_root, registry)
        }),
        PiAction::NextAction => next_action(&request),
        PiAction::PrepareAttempt => receipt::with_lock(runtime_root, &request.event_id, || {
            prepare_attempt(&request, runtime_root)
        }),
        PiAction::AttemptFinished => receipt::with_lock(runtime_root, &request.event_id, || {
            attempt_finished(&request, runtime_root)
        }),
        PiAction::Shutdown => receipt::with_lock(runtime_root, &request.event_id, || {
            shutdown(&request, runtime_root)
        }),
    }
}

fn activate(
    request: &PiEventRequest,
    scope: InstallScope,
    project_root: &Path,
    runtime_root: &Path,
    registry: &TemplateRegistry,
) -> Result<PiEventResponse> {
    if scope == InstallScope::Project
        && !pi::is_project_trusted(runtime_root, project_root, registry)
    {
        let mut response = PiEventResponse::new("blocked", &request.event_id);
        response.message = Some(
            "Project Pi role agents are not trusted. Rerun Megara install with --trust-project."
                .to_string(),
        );
        return Ok(response);
    }
    if let Some(receipt) = receipt::load(runtime_root, &request.event_id)? {
        return Ok(PiEventResponse::new(
            if receipt.active { "active" } else { "shutdown" },
            &request.event_id,
        ));
    }
    let receipt = EventReceipt {
        event_id: request.event_id.clone(),
        workflow: request.workflow.clone(),
        active: true,
        ..EventReceipt::default()
    };
    receipt::save(runtime_root, &receipt)?;
    Ok(PiEventResponse::new("active", &request.event_id))
}

fn next_action(request: &PiEventRequest) -> Result<PiEventResponse> {
    let mut response = PiEventResponse::new("ready", &request.event_id);
    response.required_roles = required_roles(&request.workflow)
        .into_iter()
        .map(str::to_string)
        .collect();
    Ok(response)
}

fn prepare_attempt(request: &PiEventRequest, runtime_root: &Path) -> Result<PiEventResponse> {
    let role = request
        .role
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("Pi prepare-attempt requires role"))?;
    let mut receipt = receipt::load(runtime_root, &request.event_id)?.unwrap_or(EventReceipt {
        event_id: request.event_id.clone(),
        workflow: request.workflow.clone(),
        active: true,
        ..EventReceipt::default()
    });
    if let Some(output) = &receipt.completed_output {
        let mut response = PiEventResponse::new("completed", &request.event_id);
        response.output = Some(output.clone());
        return Ok(response);
    }
    if let Some(attempt) = receipt.attempts.iter().find(|attempt| {
        attempt.status == "started" && attempt.role == role && attempt.model == request.model
    }) {
        let mut response = PiEventResponse::new("started", &request.event_id);
        response.attempt_id = Some(attempt.id.clone());
        response.model = attempt.model.clone();
        return Ok(response);
    }
    let attempt_id = format!("{}-{}", request.event_id, receipt.attempts.len() + 1);
    receipt.attempts.push(AttemptReceipt {
        id: attempt_id.clone(),
        role: role.to_string(),
        status: "started".to_string(),
        model: request.model.clone(),
        output: None,
        error: None,
    });
    receipt::save(runtime_root, &receipt)?;
    let mut response = PiEventResponse::new("started", &request.event_id);
    response.attempt_id = Some(attempt_id);
    response.model = request.model.clone();
    Ok(response)
}

fn attempt_finished(request: &PiEventRequest, runtime_root: &Path) -> Result<PiEventResponse> {
    let attempt_id = request
        .attempt_id
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("Pi attempt-finished requires attempt_id"))?;
    let mut receipt = receipt::load(runtime_root, &request.event_id)?
        .ok_or_else(|| anyhow::anyhow!("Pi event {} was not activated", request.event_id))?;
    let (attempt_status, attempt_output, attempt_error) = {
        let attempt = receipt
            .attempts
            .iter_mut()
            .find(|attempt| attempt.id == attempt_id)
            .ok_or_else(|| anyhow::anyhow!("Pi attempt {attempt_id} does not exist"))?;
        attempt.status = request
            .status
            .clone()
            .unwrap_or_else(|| "failed".to_string());
        attempt.output = request.output.clone();
        attempt.error = request.error.clone();
        (
            attempt.status.clone(),
            attempt.output.clone(),
            attempt.error.clone(),
        )
    };
    if attempt_status == "completed" {
        receipt.completed_output = attempt_output.clone();
        receipt::save(runtime_root, &receipt)?;
        let mut response = PiEventResponse::new("completed", &request.event_id);
        response.attempt_id = Some(attempt_id.to_string());
        response.output = attempt_output;
        return Ok(response);
    }
    let retries = receipt
        .attempts
        .iter()
        .filter(|attempt| attempt.status == "failed")
        .count();
    let retryable = request.error.as_deref().is_some_and(is_retryable);
    let status = if retryable && retries <= 2 {
        "retry"
    } else if retryable && !receipt.fallback_attempted {
        receipt.fallback_attempted = true;
        "fallback"
    } else {
        "blocked"
    };
    receipt::save(runtime_root, &receipt)?;
    let mut response = PiEventResponse::new(status, &request.event_id);
    response.attempt_id = Some(attempt_id.to_string());
    response.retry_after_ms =
        (status == "retry").then_some(if retries == 1 { 1_000 } else { 2_000 });
    response.message = attempt_error.or_else(|| Some("Pi role execution failed".to_string()));
    Ok(response)
}

fn shutdown(request: &PiEventRequest, runtime_root: &Path) -> Result<PiEventResponse> {
    if let Some(mut receipt) = receipt::load(runtime_root, &request.event_id)? {
        receipt.active = false;
        receipt::save(runtime_root, &receipt)?;
    }
    Ok(PiEventResponse::new("shutdown", &request.event_id))
}

fn required_roles(workflow: &str) -> Vec<&'static str> {
    match workflow {
        "deep-interview" => vec!["researcher", "contrarian", "simplifier", "architect"],
        "ralplan" => vec!["planner", "architect", "critic"],
        "ultragoal" => vec!["executor", "critic"],
        "team" => vec!["planner", "executor", "architect", "critic"],
        _ => Vec::new(),
    }
}

fn is_retryable(error: &str) -> bool {
    let error = error.to_ascii_lowercase();
    error.contains("capacity")
        || error.contains("rate limit")
        || error.contains("429")
        || error.contains("timeout")
}
