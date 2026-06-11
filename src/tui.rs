use crate::config::Paths;
use crate::storage::{AgentSessionRef, QueueItem};
use anyhow::{Context, Result, bail};
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};
use serde::de::DeserializeOwned;
use std::io::{self, IsTerminal, Read, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

const CONTROL_TIMEOUT: Duration = Duration::from_millis(750);

#[derive(Debug, Clone, PartialEq, Eq)]
struct DashboardState {
    daemon_running: bool,
    queue_items: Vec<QueueItem>,
    sessions: Vec<AgentSessionRef>,
    error: Option<String>,
}

pub async fn run_dashboard() -> Result<()> {
    let paths = Paths::resolve()?;
    let state = read_dashboard_state(&paths);

    if !io::stdout().is_terminal() {
        print_dashboard_text(&paths, &state);
        return Ok(());
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let result = draw_dashboard(&mut terminal, &paths);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn print_dashboard_text(paths: &Paths, state: &DashboardState) {
    println!("Sisyphus Dashboard");
    println!(
        "daemon: {}",
        if state.daemon_running {
            "running"
        } else {
            "stopped"
        }
    );
    println!("control_socket: {}", paths.socket_path.display());
    println!("queue_items: {}", state.queue_items.len());
    for item in &state.queue_items {
        println!(
            "queue_item: {}\t{}\t{}\t{}",
            item.id, item.provider, item.state, item.issue_url
        );
    }
    println!("sessions: {}", state.sessions.len());
    for session in &state.sessions {
        println!(
            "session: {}\t{}\t{}\t{}\t{}",
            session.queue_item_id,
            session.agent,
            session.dispatch_path,
            session.resume_hint.as_deref().unwrap_or_default(),
            session.app_deep_link.as_deref().unwrap_or_default()
        );
    }
    if let Some(error) = &state.error {
        println!("error: {error}");
    }
    println!("auth: sisyphus auth github");
    println!("provider_add: sisyphus provider-add github <owner> <repo>");
    println!("repo_add: sisyphus repo-add github <owner> <repo> <path>");
    println!("queue: sisyphus queue");
    if !state.daemon_running {
        println!("start: sisyphus serve");
        println!("background: sisyphus serve --daemon");
        println!("autostart: sisyphus register");
    }
}

fn draw_dashboard(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    paths: &Paths,
) -> Result<()> {
    let mut state = read_dashboard_state(paths);
    let mut selected_queue = 0_usize;
    let mut message = String::from("r refresh | up/down select | d dispatch | q quit");
    let mut last_refresh = Instant::now();

    loop {
        clamp_selected_queue(&mut selected_queue, state.queue_items.len());
        terminal.draw(|frame| {
            let area = frame.area();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(8),
                    Constraint::Length(4),
                ])
                .split(area);
            let main_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(62), Constraint::Percentage(38)])
                .split(chunks[1]);

            let title = Paragraph::new(Line::from(vec![
                Span::styled("Sisyphus", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!(
                    " local dashboard  daemon={}  queue={}  sessions={}",
                    if state.daemon_running {
                        "running"
                    } else {
                        "stopped"
                    },
                    state.queue_items.len(),
                    state.sessions.len()
                )),
            ]))
            .block(Block::default().borders(Borders::ALL));
            frame.render_widget(title, chunks[0]);

            let queue_items = queue_list_items(&state);
            let queue_list = List::new(queue_items)
                .block(Block::default().title("Queue").borders(Borders::ALL))
                .highlight_style(
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::White)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("> ");
            let mut queue_state = ListState::default();
            if !state.queue_items.is_empty() {
                queue_state.select(Some(selected_queue));
            }
            frame.render_stateful_widget(queue_list, main_chunks[0], &mut queue_state);

            let sessions = session_list_items(&state);
            let session_list =
                List::new(sessions).block(Block::default().title("Sessions").borders(Borders::ALL));
            frame.render_widget(session_list, main_chunks[1]);

            let mut footer = vec![
                Line::from(format!("Control socket: {}", paths.socket_path.display())),
                Line::from(message.as_str()),
            ];
            if let Some(error) = &state.error {
                footer.push(Line::from(Span::styled(
                    error.as_str(),
                    Style::default().fg(Color::Red),
                )));
            }
            let footer = Paragraph::new(footer)
                .block(Block::default().borders(Borders::ALL))
                .wrap(Wrap { trim: true });
            frame.render_widget(footer, chunks[2]);
        })?;

        if event::poll(Duration::from_millis(250))? {
            let Event::Key(key) = event::read()? else {
                continue;
            };

            match key.code {
                KeyCode::Char('q') => break,
                KeyCode::Char('r') => {
                    state = read_dashboard_state(paths);
                    message = String::from("refreshed");
                }
                KeyCode::Down | KeyCode::Char('j')
                    if selected_queue + 1 < state.queue_items.len() =>
                {
                    selected_queue += 1;
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    selected_queue = selected_queue.saturating_sub(1);
                }
                KeyCode::Char('d') => {
                    message = dispatch_selected_queue_item(paths, &state, selected_queue)
                        .unwrap_or_else(|error| format!("{error:#}"));
                    state = read_dashboard_state(paths);
                }
                _ => {}
            }
        }

        if last_refresh.elapsed() >= Duration::from_secs(2) {
            state = read_dashboard_state(paths);
            last_refresh = Instant::now();
        }
    }

    Ok(())
}

fn dispatch_selected_queue_item(
    paths: &Paths,
    state: &DashboardState,
    selected_queue: usize,
) -> Result<String> {
    if !state.daemon_running {
        bail!("daemon is not running");
    }

    let item = state
        .queue_items
        .get(selected_queue)
        .context("no queue item selected")?;

    let body = control_post(paths, &format!("/queue/{}/dispatch", item.id))?;
    Ok(format!(
        "dispatch requested for queue item {}: {body}",
        item.id
    ))
}

fn read_dashboard_state(paths: &Paths) -> DashboardState {
    if UnixStream::connect(&paths.socket_path).is_err() {
        let (queue_items, sessions, error) = local_dashboard_snapshot(paths);
        return DashboardState {
            daemon_running: false,
            queue_items,
            sessions,
            error,
        };
    }

    let queue_items = match control_json::<Vec<QueueItem>>(paths, "/queue") {
        Ok(items) => items,
        Err(error) => {
            let (queue_items, sessions, snapshot_error) = local_dashboard_snapshot(paths);
            return DashboardState {
                daemon_running: true,
                queue_items,
                sessions,
                error: Some(snapshot_error.map_or_else(
                    || format!("control API unavailable; showing local snapshot: {error:#}"),
                    |snapshot_error| {
                        format!(
                            "control API unavailable: {error:#}; local snapshot failed: {snapshot_error}"
                        )
                    },
                )),
            };
        }
    };

    let sessions = match control_json::<Vec<AgentSessionRef>>(paths, "/sessions") {
        Ok(sessions) => sessions,
        Err(error) => {
            let fallback_sessions =
                crate::storage::list_agent_session_refs(paths).unwrap_or_default();
            return DashboardState {
                daemon_running: true,
                queue_items,
                sessions: fallback_sessions,
                error: Some(format!(
                    "control API sessions unavailable; showing local snapshot: {error:#}"
                )),
            };
        }
    };

    DashboardState {
        daemon_running: true,
        queue_items,
        sessions,
        error: None,
    }
}

fn local_dashboard_snapshot(
    paths: &Paths,
) -> (Vec<QueueItem>, Vec<AgentSessionRef>, Option<String>) {
    let mut errors = Vec::new();
    let queue_items = match crate::storage::list_queue_items(paths) {
        Ok(items) => items,
        Err(error) => {
            errors.push(format!("{error:#}"));
            Vec::new()
        }
    };
    let sessions = match crate::storage::list_agent_session_refs(paths) {
        Ok(items) => items,
        Err(error) => {
            errors.push(format!("{error:#}"));
            Vec::new()
        }
    };
    let error = if errors.is_empty() {
        None
    } else {
        Some(errors.join("; "))
    };

    (queue_items, sessions, error)
}

fn queue_list_items(state: &DashboardState) -> Vec<ListItem<'static>> {
    queue_list_labels(state)
        .into_iter()
        .map(ListItem::new)
        .collect()
}

fn queue_list_labels(state: &DashboardState) -> Vec<String> {
    if !state.daemon_running && state.queue_items.is_empty() {
        return vec![
            "daemon stopped".to_string(),
            "start with: sisyphus serve".to_string(),
        ];
    }

    if state.queue_items.is_empty() {
        return vec!["no queue items".to_string()];
    }

    state.queue_items.iter().map(queue_item_label).collect()
}

fn session_list_items(state: &DashboardState) -> Vec<ListItem<'static>> {
    if !state.daemon_running && state.sessions.is_empty() {
        return vec![ListItem::new("daemon stopped")];
    }

    if state.sessions.is_empty() {
        return vec![ListItem::new("no session refs")];
    }

    state
        .sessions
        .iter()
        .map(|session| ListItem::new(session_label(session)))
        .collect()
}

fn queue_item_label(item: &QueueItem) -> String {
    format!(
        "#{:<4} {:<28} {:<7} {}",
        item.id,
        truncate(&item.state, 28),
        item.provider,
        truncate(&item.issue_url, 96)
    )
}

fn session_label(session: &AgentSessionRef) -> String {
    let resume = session
        .resume_hint
        .as_deref()
        .or(session.app_deep_link.as_deref())
        .or(session.session_id.as_deref())
        .unwrap_or("-");
    format!(
        "#{:<4} {:<7} {:<11} {}",
        session.queue_item_id,
        session.agent,
        session.dispatch_path,
        truncate(resume, 96)
    )
}

fn truncate(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }

    let keep = max_chars.saturating_sub(3);
    let mut truncated = value.chars().take(keep).collect::<String>();
    truncated.push_str("...");
    truncated
}

fn clamp_selected_queue(selected_queue: &mut usize, item_count: usize) {
    if item_count == 0 {
        *selected_queue = 0;
    } else if *selected_queue >= item_count {
        *selected_queue = item_count - 1;
    }
}

fn control_json<T>(paths: &Paths, route: &str) -> Result<T>
where
    T: DeserializeOwned,
{
    let response = control_get(paths, route)?;
    let body = successful_response_body(&response)?;
    serde_json::from_str(body.trim()).with_context(|| format!("failed to parse {route} response"))
}

fn control_post(paths: &Paths, route: &str) -> Result<String> {
    let response = control_request(paths, "POST", route)?;
    Ok(successful_response_body(&response)?.trim().to_string())
}

fn control_get(paths: &Paths, route: &str) -> Result<String> {
    control_request(paths, "GET", route)
}

fn control_request(paths: &Paths, method: &str, route: &str) -> Result<String> {
    let mut stream = UnixStream::connect(&paths.socket_path)?;
    stream.set_read_timeout(Some(CONTROL_TIMEOUT))?;
    stream.set_write_timeout(Some(CONTROL_TIMEOUT))?;
    let request = match method {
        "POST" => {
            format!("{method} {route} HTTP/1.1\r\nhost: sisyphus\r\ncontent-length: 0\r\n\r\n")
        }
        _ => format!("{method} {route} HTTP/1.1\r\nhost: sisyphus\r\n\r\n"),
    };
    stream.write_all(request.as_bytes())?;
    let _ = stream.shutdown(Shutdown::Write);

    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    Ok(response)
}

fn successful_response_body(response: &str) -> Result<&str> {
    let status_line = response
        .lines()
        .next()
        .context("missing HTTP status line")?;
    let status = status_line
        .split_whitespace()
        .nth(1)
        .context("missing HTTP status code")?
        .parse::<u16>()
        .context("invalid HTTP status code")?;
    let body = response
        .split_once("\r\n\r\n")
        .map(|(_, body)| body)
        .context("missing HTTP response body")?;

    if !(200..300).contains(&status) {
        bail!("{}", body.trim());
    }

    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn queue_item(id: i64, state: &str) -> QueueItem {
        QueueItem {
            id,
            provider: "github".to_string(),
            issue_url: "https://github.com/acme/widgets/issues/42".to_string(),
            state: state.to_string(),
            payload: "{}".to_string(),
        }
    }

    #[test]
    fn parses_successful_http_body() {
        let response = "HTTP/1.1 200 OK\r\ncontent-length: 3\r\n\r\n{}\n";
        assert_eq!(successful_response_body(response).unwrap(), "{}\n");
    }

    #[test]
    fn rejects_non_success_http_body() {
        let response = "HTTP/1.1 409 Conflict\r\ncontent-length: 11\r\n\r\nnot queued\n";
        let error = successful_response_body(response).unwrap_err().to_string();
        assert_eq!(error, "not queued");
    }

    #[test]
    fn clamps_selected_queue_to_available_items() {
        let mut selected = 4;
        clamp_selected_queue(&mut selected, 2);
        assert_eq!(selected, 1);

        clamp_selected_queue(&mut selected, 0);
        assert_eq!(selected, 0);
    }

    #[test]
    fn formats_queue_item_label_with_state_and_source() {
        let label = queue_item_label(&queue_item(7, "queued"));
        assert!(label.contains("#7"));
        assert!(label.contains("queued"));
        assert!(label.contains("github"));
        assert!(label.contains("https://github.com/acme/widgets/issues/42"));
    }

    #[test]
    fn offline_dashboard_still_lists_local_queue_items() {
        let state = DashboardState {
            daemon_running: false,
            queue_items: vec![queue_item(7, "queued")],
            sessions: Vec::new(),
            error: None,
        };

        let labels = queue_list_labels(&state);

        assert_eq!(labels.len(), 1);
        assert!(labels[0].contains("#7"));
        assert!(labels[0].contains("queued"));
    }
}
