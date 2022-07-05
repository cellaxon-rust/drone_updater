use crate::updater::Updater;

use tui::{
    backend::{Backend},
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{
        Gauge, Paragraph, Wrap,
    },
    Frame,
};


pub fn ui<B: Backend>(f: &mut Frame<B>, updater: &Updater)
{
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints(
            [
                Constraint::Percentage(20),
                Constraint::Percentage(10),
                Constraint::Percentage(10),
                Constraint::Percentage(30),
                Constraint::Percentage(8),
                Constraint::Percentage(12),
                Constraint::Percentage(10),
            ]
            .as_ref(),
        )
        .split(f.size());

    let (time_total, time_progress, time_left, progress) = updater.get_update_information();

    let paragraph = Paragraph::new("Drone Updater")
        .style(Style::default().fg(Color::White))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });
    f.render_widget(paragraph, chunks[1]);
    
    let paragraph = Paragraph::new(updater.get_message_version())
        .style(Style::default().fg(Color::White))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });
    f.render_widget(paragraph, chunks[2]);

    let gauge = Gauge::default()
        .gauge_style(
            Style::default()
                .fg(Color::Green)
                .bg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC | Modifier::BOLD),
        )
        .ratio((progress / 100_f32) as f64)
        .label(format!("{}%", format!("{}", progress)));
    f.render_widget(gauge, chunks[3]);

    let paragraph = Paragraph::new(updater.get_message_status())
        .style(Style::default().fg(Color::White))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });
    f.render_widget(paragraph, chunks[5]);
}

