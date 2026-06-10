# Sisyphus Product Plan

## 1. Summary

Sisyphus is a local daemon-based issue-to-agent message broker and loop controller.

It connects to supported issue management services, receives issue events, normalizes those issues into agent-friendly work packets, dispatches the packets into supported coding agents, and enforces the lifecycle of the work locally.

The product exists to remove task-catching work from coding agents. Agents should spend tokens on execution, not on polling issue trackers, scanning queues, or deciding what work exists.

The first supported issue providers are GitHub and GitLab. The first supported agent is Codex. Additional agents can be added later through agent adapters.

Sisyphus is not a central service, not an agent session manager, and not a replacement UI for the agent's native session system.

## 2. Non-Negotiable Requirements

- Sisyphus runs as a local daemon on the developer's machine.
- There is no central backend plan.
- A TUI dashboard/control surface is required.
- The daemon/backend is started explicitly with `sisyphus serve`.
- Headless background execution is supported with `sisyphus serve --daemon`.
- Reboot-persistent autostart is opt-in through `sisyphus register`.
- Sisyphus must not register a background OS service unless the user explicitly runs `sisyphus register`.
- GitHub and GitLab are the first supported issue management services.
- Codex is the first supported agent.
- Future agents must be supported through an adapter boundary.
- Sisyphus must enforce the work lifecycle.
- Sisyphus must act as the local message broker between issue events and agent work.
- While `sisyphus serve` or `sisyphus serve --daemon` is running, issue creation or issue update must be able to trigger work without asking the agent to discover tasks.
- Sisyphus must not pass raw issue content as an execution-ready task when the issue is ambiguous.
- Agents must be prompted to identify blocking ambiguities before implementation.
- Sisyphus must publish agent-generated clarification questions back to the issue as provider comments.
- Sisyphus must not own or reimplement agent session management.
- Sisyphus must remain independent from repository harness systems.
- Sessions started through Sisyphus must remain visible and resumable through the agent's native tools.
- For Codex, a Sisyphus-started session must be discoverable from both Codex CLI and Codex App where Codex itself supports that session visibility.

## 3. Product Positioning

Sisyphus is a local orchestration daemon for issue-driven engineering loops.

It is also the local inbox and dispatch queue for agent work. Its job is to convert provider events into explicit agent messages so the agent does not need to spend context or tokens figuring out which issue to pick up.

It owns:

- Provider connection state
- Repository registration
- Provider event ingestion
- Local work queue
- Agent dispatch state
- Issue normalization
- Agent task packet generation
- Local lifecycle state
- Local event log
- Provider writes such as issue comments, labels, and PR/MR status updates
- Agent result validation

It does not own:

- Agent conversation transcripts
- Agent session storage
- Agent resume/fork/archive behavior
- Agent context compaction
- Agent UI visibility
- Agent-specific session databases or files
- Repository harness interpretation
- OMA workflow execution

The product should be described as:

> Sisyphus receives issue events, brokers them into agent-native work messages, and enforces local lifecycle transitions. The agent remains the owner of its own session model and repository-local harness behavior.

## 4. Target User Experience

### 4.1 Startup And Dashboard

The user starts the local backend explicitly:

```bash
sisyphus serve
```

For headless background execution:

```bash
sisyphus serve --daemon
```

For reboot-persistent autostart, the user registers the background daemon explicitly:

```bash
sisyphus register
```

Command behavior:

- `sisyphus serve` runs the local backend in the foreground.
- `sisyphus serve --daemon` starts the local backend in the background without the TUI.
- `sisyphus register` registers `sisyphus serve --daemon` with the host OS so it starts again after reboot.
- Registration is opt-in and must never happen as a side effect of opening the TUI.

The user opens the TUI dashboard/control surface separately:

```bash
sisyphus
```

The TUI dashboard guides the user through:

1. Daemon health check
2. GitHub token registration
3. GitLab token registration
4. Repository registration
5. Provider capability check
6. Codex availability check
7. Test issue import
8. Local lifecycle dry run

The TUI remains useful as a local dashboard after setup.

### 4.2 Event-Triggered Workflow

The preferred workflow starts from the issue tracker, not from an agent prompt.

1. A GitHub or GitLab issue is created or updated.
2. Sisyphus detects the issue change through polling or manual import.
3. Sisyphus places the work in a local queue.
4. Sisyphus normalizes the issue into a `WorkItem`.
5. Sisyphus packages the `WorkItem` as an agent-specific `AgentTask`.
6. Sisyphus dispatches the task into the selected agent with a clarification gate.
7. The agent first decides whether the issue is actionable or needs clarification.
8. If clarification is needed, the agent returns structured questions and Sisyphus posts them as an issue comment.
9. If the task is actionable, the agent works inside its native session and follows repository-local instructions.
10. Sisyphus records local lifecycle transitions and stores only agent-native session references.

The agent should not be asked to scan GitHub, scan GitLab, inspect labels, or decide which issue to start. That discovery cost belongs to Sisyphus.

### 4.3 TUI-Initiated Workflow

The user opens the Sisyphus TUI and selects an issue.

Sisyphus then:

1. Fetches the issue from GitHub or GitLab
2. Converts provider-specific issue data into a common `WorkItem`
3. Converts the `WorkItem` into an agent-specific `AgentTask`
4. Creates or opens an agent-native session through the selected agent adapter
5. Asks the agent to produce clarification questions before execution if the issue is ambiguous
6. Stores only an `AgentSessionRef`, not the session itself
7. Tracks lifecycle state locally
8. Receives agent clarification requests or execution results through daemon APIs or tools
9. Applies lifecycle transitions only when rules are satisfied
10. Writes status updates back to GitHub or GitLab

For Codex, the user should be able to continue from either surface:

```bash
codex resume <session-id>
```

or:

```text
codex://threads/<session-id>
```

The exact mechanism must use Codex-supported APIs, commands, and deep links. Sisyphus must not write Codex session files directly.

## 5. Core Architecture

```text
TUI Dashboard / Control TUI
  -> Local Sisyphus Daemon
      -> Provider Registry
      -> Provider Event Ingestor
      -> GitHub Provider Adapter
      -> GitLab Provider Adapter
      -> Local Work Queue
      -> WorkItem Normalizer
      -> Lifecycle Engine
      -> Agent Task Packager
      -> Agent Dispatch Loop
      -> Agent Adapter Registry
      -> Codex Agent Adapter
      -> Local Event Store
      -> Local Artifact Store

Codex CLI / Codex App
  -> Owns Codex sessions
  -> Owns transcripts
  -> Owns resume/open/archive behavior
  -> Owns repository harness execution such as OMA
  -> Calls back into Sisyphus for lifecycle actions
```

## 6. Component Responsibilities

### 6.1 TUI Dashboard / Control TUI

The TUI is a local dashboard and control plane for a daemon that is already running through `sisyphus serve`.

Responsibilities:

- Display daemon health
- Register GitHub and GitLab credentials
- Register repositories
- Show provider capability status
- Show active runs
- Show failed lifecycle transitions
- Open an agent-native session
- Print or copy native resume commands
- Trigger retry, cancel, or block actions

The TUI must not contain lifecycle business logic. It sends commands to the daemon through the Unix socket control API. If the daemon is not running, the TUI should show the user to run `sisyphus serve`.

The TUI may show helper commands for `sisyphus serve --daemon` and `sisyphus register`, but it should not silently register autostart behavior on the user's behalf.

### 6.2 Local Daemon

The daemon is the source of truth for Sisyphus-owned state.

Responsibilities:

- Store local configuration
- Store provider auth references
- Poll provider updates
- Enqueue provider events as local work messages
- Normalize issues
- Generate agent tasks
- Request ambiguity analysis from the selected agent before execution
- Dispatch queued work to agent adapters
- Enforce lifecycle transitions
- Publish clarification questions as provider comments
- Store run events
- Store task artifacts
- Coordinate provider writes
- Expose a local Unix socket control API for TUI, CLI helpers, and diagnostics

The daemon must run without a central service.

The daemon must not require an agent to poll issue trackers or inspect issue queues. It should wake the agent with a concrete task packet when work is ready.

### 6.3 Provider Adapters

Provider adapters translate provider-specific concepts into Sisyphus domain models.

Initial adapters:

- GitHub
- GitLab

Responsibilities:

- Parse issue URLs
- Fetch issue details
- Fetch comments or discussions
- Fetch labels, assignees, milestones, and status
- Resolve repository identity
- Write issue comments
- Write clarification question comments
- Update labels when configured
- Detect linked PRs or MRs when available
- Read CI/check/pipeline status when available

Provider differences must be expressed through capabilities instead of leaking provider-specific branches throughout the core.

Example capabilities:

```text
issue_comments
issue_labels
assignees
merge_proposals
review_threads
ci_status
polling
self_managed_instance
```

### 6.4 WorkItem Normalizer

The normalizer converts provider-specific issues into a common model.

`WorkItem` should contain:

- Provider
- Provider issue id
- Repository reference
- Title
- Description
- Author
- Labels
- Assignees
- Current status
- Relevant comments
- Linked merge proposal references
- Source URL
- Provider capabilities used during import

The normalizer should preserve enough provider metadata for traceability, but core lifecycle logic should operate on provider-neutral fields.

### 6.5 Agent Task Packager

The task packager converts a `WorkItem` into an agent-specific task packet.

For Codex, `AgentTask` should include:

- Sisyphus run id
- Issue URL
- Repository path
- Goal
- Acceptance criteria
- Relevant issue context
- Constraints
- Branch policy
- Verification expectations
- Clarification gate instructions
- Callback instructions
- Lifecycle rules summary
- Required final response format

This is one of the product's most important layers. The agent should receive a clear task, not a raw issue dump.

Every `AgentTask` must begin with an ambiguity check. The agent should decide whether the task is actionable from the provided issue context. If not, it must return a structured clarification request instead of starting implementation.

Clarification prompt contract:

```text
First, inspect the task for blocking ambiguity.
If the task is not actionable, do not implement.
Return clarification questions in the required structured format.
Ask only questions that materially affect implementation or verification.
Prefer concrete multiple-choice questions when possible.
If the task is actionable, state that no blocking clarification is needed and continue.
```

The required clarification output should be machine-readable so Sisyphus can publish it as a provider comment without interpretation.

Example:

```json
{
  "type": "clarification_request",
  "blocking": true,
  "summary": "The issue does not specify the target authentication flow.",
  "questions": [
    {
      "id": "target_flow",
      "question": "Which login flow should be changed?",
      "options": ["email/password", "OAuth", "both"]
    }
  ]
}
```

Clarification comment template:

```md
Sisyphus needs clarification before starting this task.

**Summary**
{summary}

**Questions**
{numbered_questions}

Reply to this comment or update the issue description. Sisyphus will retry after new context is detected.

<!-- sisyphus:run={run_id}; clarification={clarification_id} -->
```

Comment formatting rules:

- Publish one compact clarification comment per run.
- Number each question.
- Render options as bullets under the question.
- Include only implementation-relevant questions.
- Do not publish agent reasoning or internal analysis.
- Keep Sisyphus metadata in an HTML comment footer.

The task packager must not interpret repository harness frameworks such as OMA. If a repository contains OMA, `AGENTS.md`, Codex rules, or other agent-native guidance, the task should tell the agent to follow the repository's normal instructions. The agent tool is responsible for consuming that harness through its native mechanism.

### 6.6 Agent Adapters

Agent adapters integrate with agent-native session systems without owning those sessions.

Initial adapter:

- Codex

Future possible adapters:

- Claude Code
- Cursor
- Aider
- Other local coding agents

Adapter interface:

```text
AgentAdapter
  capabilities() -> AgentCapabilities
  create_or_open_task(task) -> AgentSessionRef
  open_uri(ref) -> string
  resume_hint(ref) -> string
  observe(ref) -> AgentObservation
```

Rules:

- The adapter may ask the agent tool to create a session.
- The adapter may store the session reference returned by the agent tool.
- The adapter may deliver a prepared task packet to the agent.
- The adapter must not write or mutate agent-owned session storage directly.
- The adapter must not duplicate the agent transcript into Sisyphus state.
- The adapter must not implement its own resume/fork/archive behavior.
- The adapter must not implement repository harness behavior that belongs to the agent tool.

### 6.7 Lifecycle Engine

The lifecycle engine validates all Sisyphus-owned work transitions.

The agent can request a transition, but the daemon decides whether the transition is allowed.

Example:

```text
Codex: Work is complete.
Sisyphus: Reject Completed transition because verification artifact is missing.
```

## 7. Domain Model

### 7.1 ProviderRef

Identifies the issue management service.

Fields:

- `provider`: `github` or `gitlab`
- `instance_url`
- `capabilities`

### 7.2 RepositoryRef

Identifies a repository across providers.

Fields:

- `provider`
- `instance_url`
- `owner_or_namespace`
- `name`
- `default_branch`
- `remote_url`
- `local_path`

### 7.3 IssueEvent

Provider event captured by Sisyphus before normalization.

Fields:

- `id`
- `provider_ref`
- `repository_ref`
- `event_type`
- `source_url`
- `provider_issue_id`
- `provider_payload_ref`
- `received_at`
- `dedupe_key`

Sisyphus should store enough event data to deduplicate and replay work, but it does not need to preserve every provider payload field in the core model.

### 7.4 WorkQueueItem

Local broker record for pending agent work.

Fields:

- `id`
- `issue_event_id`
- `work_item_id`
- `state`
- `priority`
- `attempt_count`
- `next_attempt_at`
- `created_at`
- `updated_at`

The queue exists so Sisyphus can trigger work without requiring agents to discover tasks.

### 7.5 WorkItem

Provider-neutral issue representation.

Fields:

- `id`
- `provider_ref`
- `repository_ref`
- `source_url`
- `title`
- `body`
- `author`
- `labels`
- `assignees`
- `state`
- `comments`
- `linked_merge_proposals`
- `imported_at`

### 7.6 AgentTask

Agent-ready task packet.

Fields:

- `id`
- `run_id`
- `queue_item_id`
- `work_item_id`
- `agent`
- `workspace_path`
- `prompt`
- `acceptance_criteria`
- `constraints`
- `clarification_contract`
- `created_at`

### 7.7 ClarificationRequest

Agent-generated request for missing information.

Fields:

- `id`
- `run_id`
- `agent_task_id`
- `blocking`
- `summary`
- `questions`
- `provider_comment_id`
- `created_at`
- `resolved_at`

The agent creates the questions. Sisyphus stores and publishes them. Sisyphus should not rewrite the questions beyond formatting them into a provider comment.

### 7.8 AgentSessionRef

Reference to an agent-owned session.

Fields:

- `agent`
- `session_id`
- `workspace_path`
- `open_uri`
- `resume_hint`
- `created_at`
- `last_seen_at`

This is a reference only. It is not a Sisyphus session.

### 7.9 LoopRun

Sisyphus-owned lifecycle record for a unit of work.

Fields:

- `id`
- `work_item_id`
- `agent_task_id`
- `agent_session_ref`
- `state`
- `lease_owner`
- `started_at`
- `updated_at`
- `completed_at`

### 7.10 LoopEvent

Append-only record of significant daemon-owned events.

Examples:

- Issue event received
- Work queued
- Work item imported
- Agent task generated
- Agent task dispatched
- Agent session reference registered
- Clarification requested
- Clarification comment published
- Clarification resolved
- Transition requested
- Transition accepted
- Transition rejected
- Provider comment written
- Verification artifact received
- Run blocked
- Run completed

## 8. Lifecycle

Initial lifecycle:

```text
Discovered
-> Prepared
-> AssignedToAgent
-> InProgress
-> AwaitingVerification
-> AwaitingPublish
-> ObservingExternalChecks
-> NeedsIteration
-> Completed
```

Clarification branch:

```text
AssignedToAgent
-> AwaitingClarification
-> Prepared
```

Terminal or exceptional states:

```text
Blocked
Cancelled
Failed
```

### 8.1 Transition Rules

`Discovered -> Prepared`

- Requires a valid `WorkItem`
- Requires registered repository mapping

`Prepared -> AssignedToAgent`

- Requires generated `AgentTask`
- Requires selected supported agent

`AssignedToAgent -> InProgress`

- Requires valid `AgentSessionRef`
- Requires agent response that no blocking clarification is needed
- Requires no active conflicting run for the same repository/worktree

`AssignedToAgent -> AwaitingClarification`

- Requires valid `AgentSessionRef`
- Requires agent-generated `ClarificationRequest`
- Requires provider comment publication or a recorded provider-write failure

`AwaitingClarification -> Prepared`

- Requires new provider comment, issue body update, or explicit user action that may resolve the ambiguity
- Sisyphus should rebuild the `WorkItem` from current provider state before redispatch

`InProgress -> AwaitingVerification`

- Requires agent result submission
- Requires changed files, patch summary, or explicit no-change explanation

`AwaitingVerification -> AwaitingPublish`

- Requires verification result
- Verification may be local tests, lint, build, manual approval, or configured command

`AwaitingPublish -> ObservingExternalChecks`

- Requires PR/MR creation or update when the work requires code changes
- Requires provider update event recorded

`ObservingExternalChecks -> NeedsIteration`

- Requires failed CI, review feedback, or user-requested changes

`ObservingExternalChecks -> Completed`

- Requires configured completion condition
- Examples: PR merged, issue closed, maintainer approval, or explicit local override

Any non-terminal state -> `Blocked`

- Requires blocker reason
- Should write provider-visible status when configured

Any non-terminal state -> `Cancelled`

- Requires user action
- Should not erase event history

## 9. Codex Integration

Codex is the first supported agent.

### 9.1 Requirements

- Sisyphus must create or route work into a Codex-native thread/session.
- Sisyphus must store only Codex's session reference.
- The Codex session must remain manageable by Codex itself.
- Codex must consume repository-local instructions and harness systems through its normal mechanisms.
- The user must be able to resume from Codex CLI when Codex supports the session.
- The user must be able to open from Codex App when Codex supports the session deep link.
- Sisyphus must not write to `~/.codex/sessions` or other Codex-owned session storage.
- Sisyphus must not parse or execute OMA workflows on Codex's behalf.
- Sisyphus does not define a global minimum Codex CLI version.
- Codex support is determined through runtime capability probing.

### 9.2 OMA And Repository Harness Boundary

If a repository uses OMA, Codex should use OMA because Codex is the agent executing the work.

Sisyphus should not become OMA-aware beyond passing the task into the correct workspace. In an OMA-enabled repository, Codex will see the normal repository instructions such as `AGENTS.md`, `.agents/`, `.codex/`, and related runtime configuration.

Allowed Sisyphus behavior:

- Include the issue URL and Sisyphus run id in the Codex task prompt.
- Set the Codex working directory to the registered repository path.
- Tell Codex to follow repository instructions and available project workflows.
- Store Codex-native session references returned by supported Codex integration paths.

Forbidden Sisyphus behavior:

- Read OMA workflows to decide how Codex should execute them.
- Run OMA workflows directly as a substitute for Codex.
- Convert OMA skills into Sisyphus lifecycle rules.
- Store OMA execution state as Sisyphus-owned session state.

This keeps the boundary strict:

```text
Sisyphus owns issue events, dispatch, queueing, and lifecycle.
Codex owns execution, OMA usage, and agent-native session behavior.
```

### 9.3 Codex Integration Order

Sisyphus should not rely on a global minimum Codex CLI version. The `CodexAdapter` should probe for the capabilities required by each integration path and select the best available path at runtime.

Capability probes:

```text
app_server_available
exec_json_available
exec_json_thread_started_event
resume_available
app_deep_link_available
session_ref_observable
```

If a required capability is missing, Sisyphus should fall back to another path or report an actionable setup error.

Preferred order:

```text
1. Codex app-server
2. codex exec --json
3. codex:// deep-link-first manual open
```

Sisyphus must select the first available path through runtime capability probing.

Preferred integration path:

```text
Codex app-server
```

Reason:

- It exposes thread and turn primitives.
- It is intended for rich local integrations.
- It can return thread identifiers through an API boundary.

Risk:

- It is currently marked experimental in Codex CLI.
- Sisyphus must isolate it behind capability probing and fallback paths.

Fallback integration path:

```text
codex exec --json
```

Reason:

- It is scriptable.
- It emits `thread.started` events.
- It can be used for non-interactive agent execution.

Risk:

- It may be less suitable for a rich interactive UX.
- App visibility and resume behavior must be verified in real environments.

Manual-open path:

```text
codex://threads/new?prompt=<encoded-task>&path=<workspace>
```

Reason:

- It gives a clean Codex App entry point.
- It lets Codex own the session from the beginning.

Risk:

- The daemon may not know the final session id unless Codex exposes it through the selected integration path.

### 9.4 Codex Session Visibility Acceptance Criteria

- A Sisyphus-started Codex task has an `AgentSessionRef`.
- The TUI can show the Codex session id when available.
- The TUI can show a Codex CLI resume hint when available.
- The TUI can open a Codex App deep link when available.
- The daemon lifecycle remains authoritative even if the user continues the Codex session outside the Sisyphus TUI.
- Codex can use repository-local OMA or equivalent harness instructions without Sisyphus parsing them.
- The daemon can recover after restart because the lifecycle state is local and the Codex session reference is stored.

## 10. Local Storage

MVP storage should be local and simple.

Recommended layout:

```text
~/.sisyphus/
  config.toml
  sisyphus.db
  artifacts/
    <run-id>/
      agent-task.md
      agent-result.json
      verification.log
```

Default polling configuration:

```toml
[polling]
interval_seconds = 5
max_backoff_seconds = 60

[dispatch]
require_open = true
trigger_labels = ["sisyphus"]
ignore_labels = ["wontfix", "blocked"]
```

`interval_seconds` defaults to `5` when omitted. Provider polling behavior must be configurable through `~/.sisyphus/config.toml`.

Automatic dispatch defaults to an AND rule: the issue must be open and must have at least one configured trigger label. With the default configuration, only open issues labeled `sisyphus` are dispatched automatically.

Recommended database:

```text
SQLite
```

Rationale:

- Portable
- Local-first
- No central backend dependency
- Good enough for event log and lifecycle state
- Easy to inspect during early development

Sensitive provider tokens should use the host keychain where practical. If keychain integration is not available in the first build, local encrypted storage should be treated as a security-critical follow-up rather than hidden inside general config handling.

## 11. Local Daemon API

Sisyphus uses a local Unix socket control API.

- Transport: Unix domain socket
- Default path: `~/.sisyphus/sisyphus.sock`
- Used by: TUI, CLI helpers, local diagnostics
- Purpose: daemon control, provider setup, repository registration, queue inspection, lifecycle operations

Initial endpoints:

```text
GET  /health
GET  /queue
GET  /sessions
GET  /events
POST /queue/:id/dispatch
```

Sisyphus does not expose provider webhook receiver endpoints or loopback HTTP callback endpoints in the MVP.

Provider changes are detected through polling or manual import. Polling defaults to every 5 seconds and can be changed in `~/.sisyphus/config.toml`. Polling results should feed the same local event ingestion path used by manual imports. The agent dispatch loop should consume from the local queue, not from provider APIs directly.

By default, automatic dispatch applies only when both conditions are true:

- The issue is open.
- The issue has at least one configured trigger label.

With the default configuration, this means an open issue labeled `sisyphus`. Closed issues, unlabeled issues, ignored-label issues, bot-only updates, Sisyphus-authored comments, and issues with an active run should not dispatch automatically.

## 12. MVP Scope

### 12.1 In Scope

- TUI dashboard/control surface
- Local daemon
- Foreground daemon startup with `sisyphus serve`
- Headless background daemon startup with `sisyphus serve --daemon`
- Explicit autostart registration with `sisyphus register`
- SQLite-backed state and event log
- Provider event ingestion
- Local work queue
- Agent dispatch loop
- GitHub issue import
- GitLab issue import
- Common `WorkItem` model
- Codex `AgentTask` generation
- Agent ambiguity analysis prompt contract
- Clarification request storage
- Provider issue comments for clarification questions
- Codex adapter proof of concept
- Agent session reference storage
- Lifecycle state machine
- Transition validation
- Provider issue comments for lifecycle updates
- Basic retry and block flows
- Daemon restart recovery

### 12.2 Out Of Scope

- Central backend
- Multi-user coordination
- Web dashboard
- Hosted runners
- Jira, Linear, Trello, or other issue providers
- Full project board synchronization
- Cross-machine session transfer
- Reimplementation of Codex session management
- Direct mutation of Codex session storage
- Sisyphus-level OMA execution
- Repository harness interpretation
- Agent-side issue discovery or queue scanning
- Full automation of every provider-specific workflow nuance

## 13. Implementation Phases

### Phase 1: Foundation

Deliverables:

- Project skeleton
- Domain model definitions
- Lifecycle state machine
- SQLite event store
- Local work queue schema
- Local daemon health endpoint
- `sisyphus serve` foreground startup
- `sisyphus serve --daemon` background startup
- `sisyphus register` autostart registration
- Basic TUI dashboard shell

Success criteria:

- Daemon starts locally through `sisyphus serve`.
- Daemon can start headlessly through `sisyphus serve --daemon`.
- User can register reboot-persistent startup through `sisyphus register`.
- TUI can detect daemon health.
- A synthetic issue event can be enqueued and inspected.
- Lifecycle transitions can be accepted or rejected in isolated tests.
- Event log survives daemon restart.

### Phase 2: Provider Event And Read Path

Deliverables:

- Provider event ingestion path
- Polling-based issue event detection
- GitHub issue adapter
- GitLab issue adapter
- Repository registration
- WorkItem normalization

Success criteria:

- A GitHub or GitLab issue event creates a queue item.
- A GitHub issue URL imports into a `WorkItem`.
- A GitLab issue URL imports into the same `WorkItem` shape.
- Provider capability differences are represented explicitly.

### Phase 3: Codex Task Routing

Deliverables:

- Codex `AgentTask` prompt format
- Structured clarification request format
- Codex adapter integration spike
- Queue-to-agent dispatch loop
- Agent session reference capture
- TUI actions for opening/resuming Codex sessions

Success criteria:

- Sisyphus can route one queued issue into Codex.
- Codex can return a structured clarification request before implementation.
- Sisyphus can publish that clarification request as an issue comment.
- Sisyphus stores only `AgentSessionRef`.
- User can continue through Codex-native surfaces where supported.
- Codex consumes repo-local guidance such as OMA through Codex's native workspace behavior.
- Sisyphus does not write Codex-owned session files.

### Phase 4: Lifecycle Enforcement

Deliverables:

- Agent result submission endpoint
- Clarification request submission endpoint
- Transition validation rules
- Verification artifact support
- Provider-visible status comments

Success criteria:

- Codex cannot mark work complete without daemon-accepted transition.
- Ambiguous work enters `AwaitingClarification` instead of `InProgress`.
- Invalid transitions are rejected with actionable reasons.
- Valid transitions are recorded as events.
- Provider issue receives lifecycle status updates.

### Phase 5: Operational Hardening

Deliverables:

- Retry handling
- Block/cancel flows
- Polling scheduler
- Token validation
- Error reporting in TUI
- Daemon diagnostics

Success criteria:

- Common provider auth failures are visible and recoverable.
- Daemon restart does not lose active run state.
- Failed provider writes can be retried.
- TUI can show the current reason a run is blocked or failed.

## 14. Technology Direction

Recommended stack:

```text
Language: Rust
TUI: ratatui + crossterm
Daemon API: Unix socket control API
Storage: SQLite
Config: TOML
Provider APIs: GitHub REST/GraphQL, GitLab REST
```

Rationale:

- Rust produces portable single binaries with strong runtime predictability.
- Rust's type system fits provider normalization, lifecycle state machines, and adapter boundaries well.
- `ratatui` with `crossterm` is mature enough for a dashboard/control-plane TUI.
- SQLite fits local daemon state.
- The architecture remains compatible with future agent adapters.

TypeScript may still be useful for Codex app-server protocol experiments or generated client prototypes, but it should not be the primary daemon/TUI implementation language.

## 15. Key Risks

### 15.1 Codex App/CLI Session Visibility

Risk:

Codex session visibility across CLI and App depends on Codex-native behavior and supported APIs.

Mitigation:

- Use Codex-supported APIs and commands only.
- Validate `app-server`, `codex exec --json`, `codex resume`, and `codex://threads` behavior in a spike.
- Treat direct session file mutation as forbidden.

### 15.2 Experimental Codex App-Server

Risk:

Codex app-server is currently experimental and may change.

Mitigation:

- Isolate it behind `CodexAdapter`.
- Store adapter capability data.
- Probe for required capabilities at runtime.
- Provide fallback paths where feasible.

### 15.3 Provider Model Drift

Risk:

GitHub and GitLab differ in issue comments, discussions, linked merge requests, CI status, and permissions.

Mitigation:

- Use explicit provider capabilities.
- Keep provider metadata traceable.
- Avoid encoding provider assumptions in lifecycle core.

### 15.4 Lifecycle Bypass

Risk:

Users may continue an agent session outside Sisyphus and assume work is complete without daemon validation.

Mitigation:

- Make lifecycle state visible in TUI and provider comments.
- Include the Sisyphus queue item id in every `AgentTask`.
- Require Sisyphus-owned state changes to pass through daemon lifecycle transitions.
- Treat external agent completion as agent-native context, not authoritative Sisyphus state.

### 15.5 Harness Boundary Drift

Risk:

Sisyphus may slowly accumulate repository harness behavior, such as parsing OMA workflows or deciding which agent workflow should run.

Mitigation:

- Keep Sisyphus focused on issue events, queueing, dispatch, and lifecycle.
- Let Codex consume OMA and repository instructions natively.
- Do not add OMA-specific lifecycle rules to Sisyphus core.
- Treat harness behavior as part of agent execution, not broker behavior.

### 15.6 Agent Token Waste Through Task Discovery

Risk:

If agents are asked to scan issue trackers or decide which work to pick up, Sisyphus fails its core purpose and wastes context on task discovery.

Mitigation:

- Provider events must be ingested by the daemon.
- Queue items must be explicit.
- Agent prompts must contain the selected task directly.
- Agents must not be asked to poll GitHub/GitLab or choose from issue lists in the normal path.

### 15.7 Local Control Socket Exposure

Risk:

Another local process could attempt to submit fake control operations through the Unix socket.

Mitigation:

- Use a Unix domain socket under `~/.sisyphus/`.
- Reject invalid lifecycle transitions in the daemon.
- Keep provider credentials as environment-variable references, not stored token values.
- Never expose provider webhook receiver endpoints or HTTP callback endpoints in the MVP.

### 15.8 Background Daemon Registration Drift

Risk:

Autostart registration can become hard to reason about if it happens implicitly, registers duplicate daemon instances, or survives after the user expects Sisyphus to stop.

Mitigation:

- Require explicit `sisyphus register` for reboot-persistent autostart.
- Never register autostart as a side effect of opening the TUI.
- Register `sisyphus serve --daemon` as the autostart target.
- Ensure daemon startup handles an already-running instance safely.
- Surface registered/running status in the TUI dashboard.

### 15.9 Raw Issue Ambiguity Leakage

Risk:

If Sisyphus passes an ambiguous issue directly into implementation, the agent may guess, implement the wrong behavior, or waste a full session on avoidable clarification.

Mitigation:

- Every `AgentTask` must include a clarification gate.
- Agents must return structured questions instead of implementing when ambiguity blocks execution.
- Sisyphus must publish clarification questions to the provider issue.
- `AwaitingClarification` must be a normal lifecycle state, not a failure.
- The run must be redispatched only after new issue context or explicit user action.

## 16. Open Design Decisions

No open product-shaping decisions remain in this planning pass. The remaining work is implementation planning and validation.

## 17. MVP Acceptance Criteria

- A user can start the local daemon/backend with `sisyphus serve`.
- A user can start the local daemon/backend headlessly with `sisyphus serve --daemon`.
- A user can register `sisyphus serve --daemon` for reboot-persistent startup with `sisyphus register`.
- A user can open the TUI dashboard with `sisyphus`.
- The daemon runs locally without central backend dependency.
- The user can connect GitHub and GitLab credentials.
- The user can register a local repository.
- A provider issue event can be ingested without involving an agent.
- A local queue item can be created from a provider issue event.
- The user can import a GitHub issue.
- The user can import a GitLab issue.
- Imported issues normalize into the same `WorkItem` model.
- Sisyphus can generate a Codex-ready `AgentTask`.
- Sisyphus can dispatch a queued task into a Codex-native session or provide a Codex-native open path.
- A Codex task prompt requires ambiguity analysis before implementation.
- If Codex returns blocking questions, Sisyphus records a `ClarificationRequest`.
- Sisyphus publishes clarification questions back to the source GitHub/GitLab issue as a comment.
- Ambiguous work remains in `AwaitingClarification` until new provider context or explicit user action arrives.
- Sisyphus stores only a Codex session reference, not Codex transcript or session state.
- The user can open or resume the Codex session through Codex-native surfaces where supported.
- Codex can use OMA or other repository-local harnesses without Sisyphus interpreting them.
- The normal path does not require Codex to scan issue trackers or choose an issue.
- The daemon enforces lifecycle transitions.
- Invalid lifecycle transitions are rejected.
- Valid transitions are recorded in the local event log.
- Provider-visible lifecycle updates can be written back as issue comments.
- Daemon restart preserves run state.

## 18. Planning Conclusion

The core product bet is not broad provider coverage or a sophisticated UI. The core bet is a strict boundary:

```text
Sisyphus owns issue events, message brokering, dispatch, and lifecycle.
The agent owns execution, repository harness usage, and the agent session.
```

This boundary keeps Sisyphus portable across issue providers and future coding agents while avoiding brittle coupling to any agent's private session storage.
