use anyhow::Result;
use ratatui::{
    backend::TestBackend,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Terminal,
};

const WIDTH: u16 = 92;
const INNER_WIDTH: usize = 90;
const MAX_HEIGHT: usize = 80;

pub struct Section {
    title: String,
    lines: Vec<String>,
}

impl Section {
    pub fn new(title: impl Into<String>, lines: Vec<String>) -> Self {
        Self {
            title: title.into(),
            lines,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }
}

pub fn print_dashboard(
    title: &str,
    status: &str,
    rows: &[(&str, String)],
    sections: &[Section],
) -> Result<()> {
    let lines = dashboard_lines(status, rows, sections);
    print_panel(title, lines)
}

pub fn print_list(title: &str, subtitle: &str, items: &[String]) -> Result<()> {
    let sections = [Section::new("Items", items.to_vec())];
    print_dashboard(title, subtitle, &[], &sections)
}

fn dashboard_lines(
    status: &str,
    rows: &[(&str, String)],
    sections: &[Section],
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    lines.push(Line::from(vec![
        Span::styled("status", label_style()),
        Span::raw("  "),
        Span::styled(status.to_string(), value_style()),
    ]));

    if !rows.is_empty() {
        lines.push(Line::raw(""));
        let label_width = rows
            .iter()
            .map(|(label, _)| label.len())
            .max()
            .unwrap_or(0)
            .max(8);
        for (label, value) in rows {
            push_wrapped_line(
                &mut lines,
                &format!("{label:<label_width$}  "),
                value,
                &format!("{:width$}  ", "", width = label_width),
            );
        }
    }

    for section in sections.iter().filter(|section| !section.is_empty()) {
        lines.push(Line::raw(""));
        lines.push(Line::from(Span::styled(
            section.title.clone(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )));
        for line in &section.lines {
            push_wrapped_line(&mut lines, "  ", line, "    ");
        }
    }

    lines
}

fn print_panel(title: &str, lines: Vec<Line<'static>>) -> Result<()> {
    let height = (lines.len() + 4).clamp(7, MAX_HEIGHT) as u16;
    let backend = TestBackend::new(WIDTH, height);
    let mut terminal = Terminal::new(backend)?;
    terminal.draw(|frame| {
        let area = Rect::new(0, 0, WIDTH, height);
        let block = Block::default()
            .title(format!(" Megara / {title} "))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));
        let paragraph = Paragraph::new(lines).block(block);
        frame.render_widget(paragraph, area);
    })?;

    println!("{}", buffer_to_string(terminal.backend().buffer()));
    Ok(())
}

fn buffer_to_string(buffer: &ratatui::buffer::Buffer) -> String {
    let width = buffer.area.width as usize;
    buffer
        .content
        .chunks(width)
        .map(|cells| {
            cells
                .iter()
                .map(|cell| cell.symbol())
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn label_style() -> Style {
    Style::default()
        .fg(Color::Gray)
        .add_modifier(Modifier::BOLD)
}

fn value_style() -> Style {
    Style::default()
        .fg(Color::Green)
        .add_modifier(Modifier::BOLD)
}

fn push_wrapped_line(
    lines: &mut Vec<Line<'static>>,
    prefix: &str,
    text: &str,
    continuation_prefix: &str,
) {
    let mut rest = text.trim_end();
    let mut current_prefix = prefix;
    loop {
        let available = INNER_WIDTH
            .saturating_sub(current_prefix.chars().count())
            .max(1);
        if rest.chars().count() <= available {
            lines.push(Line::raw(format!("{current_prefix}{rest}")));
            return;
        }

        let (chunk, next) = split_for_width(rest, available);
        lines.push(Line::raw(format!("{current_prefix}{chunk}")));
        rest = next.trim_start();
        current_prefix = continuation_prefix;
    }
}

fn split_for_width(text: &str, limit: usize) -> (&str, &str) {
    let hard_cut = byte_index_after_chars(text, limit);
    let search = &text[..hard_cut];
    let preferred_cut = search
        .rfind(|character| ['/', ' ', ',', ';'].contains(&character))
        .filter(|position| *position > hard_cut / 2)
        .map(|position| position + 1)
        .unwrap_or(hard_cut);
    (&text[..preferred_cut], &text[preferred_cut..])
}

fn byte_index_after_chars(text: &str, count: usize) -> usize {
    text.char_indices()
        .nth(count)
        .map(|(index, _)| index)
        .unwrap_or(text.len())
}
