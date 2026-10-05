//! The notification centre in the shell (007 T053; spec US5; capability C12, behaviour that stays
//! B7): the panel behind the navigation bar's bell, the bell's badge, and the toast layer.
//!
//! Everything comes from the core's `NotificationCenter` (fed by its `watch`): connection alerts
//! (one entry per server that updates as the connection changes), Stash jobs with live progress
//! (the `JobsWatcher`), and playback failures (posted by the playback region). The UI only keeps
//! which toasts are showing; changes go back to the centre as effects.
//!
//! Toasts never take focus: they're a layer of plain containers over the page, and keys keep
//! going where they were going.

use std::collections::HashSet;
use std::time::Duration;

use iced::widget::{button, column, container, progress_bar, row, scrollable, text, Space};
use iced::{Alignment, Background, Element, Length};
use stash_core::shell::notifications::{JobStatus, Notification, Severity};
use uuid::Uuid;

use crate::effects::{Effect, NotificationAction};
use crate::widgets::{icon, icon_button, theme, Icon};

/// How many toasts show at once (the newest).
const MAX_TOASTS: usize = 3;

/// What the bell shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Badge {
    pub unread: usize,
    /// A Stash job is running.
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq)]
struct Toast {
    id: String,
    /// The entry's `updated_at` when it toasted: an update toasts again.
    stamp: String,
}

#[derive(Debug, Default)]
pub struct Notifications {
    /// This server's and general entries, newest first.
    pub list: Vec<Notification>,
    /// The session's server (entries for other servers are hidden).
    pub profile: Option<Uuid>,
    toasts: Vec<Toast>,
    /// `(id, updated_at)` already toasted.
    seen: HashSet<(String, String)>,
}

#[derive(Debug, Clone)]
pub enum Msg {
    /// The centre's list changed.
    Changed(Vec<Notification>),
    /// The panel opened: everything in it has been seen.
    Opened,
    ToastExpired {
        id: String,
        stamp: String,
    },
    /// A toast's close button (the entry stays in the centre).
    CloseToast(String),
    Dismiss(String),
    DismissAll,
    /// Close the panel (handled by the session, which owns overlays).
    Close,
}

fn toast_time(severity: Severity) -> Duration {
    match severity {
        Severity::Error => Duration::from_secs(10),
        Severity::Warning => Duration::from_secs(8),
        Severity::Info => Duration::from_secs(5),
    }
}

impl Notifications {
    /// The centre as a session for `profile` sees it.
    pub fn for_profile(profile: Uuid) -> Self {
        Self {
            profile: Some(profile),
            ..Self::default()
        }
    }

    pub fn badge(&self) -> Badge {
        Badge {
            unread: self.list.iter().filter(|n| !n.read).count(),
            active: self
                .list
                .iter()
                .any(|n| n.job.as_ref().is_some_and(|j| j.status.is_active())),
        }
    }

    /// The entries showing as toasts, newest first.
    pub fn toasts(&self) -> Vec<&Notification> {
        self.toasts
            .iter()
            .filter_map(|t| self.list.iter().find(|n| n.id == t.id))
            .collect()
    }

    pub fn update(&mut self, msg: Msg) -> Vec<Effect> {
        match msg {
            Msg::Changed(all) => self.changed(all),
            Msg::Opened => {
                let unread: Vec<String> = self
                    .list
                    .iter()
                    .filter(|n| !n.read)
                    .map(|n| n.id.clone())
                    .collect();
                self.toasts.clear();
                if unread.is_empty() {
                    Vec::new()
                } else {
                    vec![Effect::Notifications(NotificationAction::MarkRead(unread))]
                }
            }
            Msg::ToastExpired { id, stamp } => {
                self.toasts.retain(|t| !(t.id == id && t.stamp == stamp));
                Vec::new()
            }
            Msg::CloseToast(id) => {
                self.toasts.retain(|t| t.id != id);
                Vec::new()
            }
            Msg::Dismiss(id) => {
                self.toasts.retain(|t| t.id != id);
                vec![Effect::Notifications(NotificationAction::Dismiss(id))]
            }
            Msg::DismissAll => {
                self.toasts.clear();
                vec![Effect::Notifications(NotificationAction::DismissAll)]
            }
            Msg::Close => Vec::new(),
        }
    }

    fn changed(&mut self, all: Vec<Notification>) -> Vec<Effect> {
        let profile = self.profile;
        self.list = all
            .into_iter()
            .filter(|n| n.profile_id.is_none() || n.profile_id == profile)
            .collect();
        // Toasts for entries that went away go too.
        let list = &self.list;
        self.toasts.retain(|t| list.iter().any(|n| n.id == t.id));
        let mut effects = Vec::new();
        for n in self.list.iter().filter(|n| n.toast) {
            let key = (n.id.clone(), n.updated_at.clone());
            if !self.seen.insert(key) {
                continue;
            }
            self.toasts.retain(|t| t.id != n.id);
            self.toasts.insert(
                0,
                Toast {
                    id: n.id.clone(),
                    stamp: n.updated_at.clone(),
                },
            );
            effects.push(Effect::ExpireToast {
                id: n.id.clone(),
                stamp: n.updated_at.clone(),
                after: toast_time(n.severity),
            });
        }
        self.toasts.truncate(MAX_TOASTS);
        effects
    }

    /// The bell, with the unread count and a marker while a job runs.
    pub fn bell<'a, M: Clone + 'a>(&self, on_press: M) -> Element<'a, M> {
        let badge = self.badge();
        let mut content = row![icon(Icon::Bell, 18.0)]
            .spacing(4)
            .align_y(Alignment::Center);
        if badge.unread > 0 {
            content = content.push(
                container(text(badge.unread.min(99).to_string()).size(11))
                    .padding([1, 6])
                    .style(|_| {
                        container::Style::default()
                            .background(Background::Color(theme::ROSEWOOD))
                            .border(iced::border::rounded(8))
                            .color(theme::TEXT)
                    }),
            );
        }
        if badge.active {
            content = content.push(text("●").size(10).color(theme::FROSTED_BLUE));
        }
        button(content)
            .padding(6)
            .style(button::text)
            .on_press(on_press)
            .into()
    }

    /// The panel (an overlay).
    pub fn panel(&self) -> Element<'_, Msg> {
        let header = row![
            text("Notifications").size(20),
            Space::new().width(Length::Fill),
            button(text("Clear all").size(13))
                .style(button::text)
                .on_press_maybe((!self.list.is_empty()).then_some(Msg::DismissAll)),
            icon_button(Icon::Close, 14.0, Some(Msg::Close)),
        ]
        .align_y(Alignment::Center);
        let body: Element<'_, Msg> = if self.list.is_empty() {
            container(text("Nothing here.").color(theme::MUTED))
                .padding(12)
                .into()
        } else {
            scrollable(column(self.list.iter().map(entry)).spacing(8))
                .height(Length::Shrink)
                .into()
        };
        container(column![header, body].spacing(12))
            .padding(18)
            .width(Length::Fixed(440.0))
            .max_height(560.0)
            .style(theme::panel)
            .into()
    }

    /// The toast layer: bottom right, over the page, never focused.
    pub fn toast_layer(&self) -> Option<Element<'_, Msg>> {
        let toasts = self.toasts();
        if toasts.is_empty() {
            return None;
        }
        let stack = column(toasts.into_iter().map(toast))
            .spacing(8)
            .width(Length::Fixed(360.0));
        Some(
            container(stack)
                .padding(16)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_right(Length::Fill)
                .align_bottom(Length::Fill)
                .into(),
        )
    }
}

fn severity_color(severity: Severity) -> iced::Color {
    match severity {
        Severity::Error => theme::ROSEWOOD,
        Severity::Warning => theme::FROSTED_BLUE,
        Severity::Info => theme::CERULEAN,
    }
}

/// "just now", "5 min ago", "3 h ago", or the date.
pub fn ago(timestamp: &str) -> String {
    let Ok(at) = chrono::DateTime::parse_from_rfc3339(timestamp) else {
        return String::new();
    };
    let secs = (chrono::Utc::now() - at.with_timezone(&chrono::Utc)).num_seconds();
    match secs {
        s if s < 60 => "just now".into(),
        s if s < 3600 => format!("{} min ago", s / 60),
        s if s < 86_400 => format!("{} h ago", s / 3600),
        _ => at.format("%Y-%m-%d").to_string(),
    }
}

fn job_line(n: &Notification) -> Option<Element<'_, Msg>> {
    let job = n.job.as_ref()?;
    let status = match job.status {
        JobStatus::Queued => "Queued",
        JobStatus::Running => "Running",
        JobStatus::Stopping => "Stopping",
        JobStatus::Finished => "Finished",
        JobStatus::Failed => "Failed",
        JobStatus::Cancelled => "Cancelled",
        JobStatus::Unknown => "Status unknown",
    };
    let mut line = column![].spacing(4);
    if job.status.is_active() {
        if let Some(p) = job.progress {
            line = line.push(progress_bar(0.0..=1.0, p as f32).girth(6));
        }
    }
    let label = match (job.status.is_active(), job.progress) {
        (true, Some(p)) => format!("{status} · {:.0}%", p * 100.0),
        _ => status.to_owned(),
    };
    line = line.push(text(label).size(12).color(theme::MUTED));
    Some(line.into())
}

fn entry(n: &Notification) -> Element<'_, Msg> {
    let dot = container(Space::new().width(8).height(8)).style(move |_| {
        container::Style::default()
            .background(Background::Color(severity_color(n.severity)))
            .border(iced::border::rounded(4))
    });
    let mut body = column![row![
        text(n.title.clone()).size(15).width(Length::Fill),
        text(ago(&n.updated_at)).size(11).color(theme::MUTED),
    ]
    .spacing(8)]
    .spacing(4)
    .width(Length::Fill);
    if let Some(detail) = &n.detail {
        body = body.push(text(detail.clone()).size(13));
    }
    if let Some(job) = job_line(n) {
        body = body.push(job);
    }
    let read = n.read;
    container(
        row![
            dot,
            body,
            icon_button(Icon::Close, 12.0, Some(Msg::Dismiss(n.id.clone()))),
        ]
        .spacing(10)
        .align_y(Alignment::Start),
    )
    .padding(10)
    .style(move |_| {
        container::Style::default()
            .background(Background::Color(if read {
                theme::BACKGROUND
            } else {
                theme::YALE_BLUE
            }))
            .border(iced::border::rounded(8))
    })
    .into()
}

fn toast(n: &Notification) -> Element<'_, Msg> {
    let color = severity_color(n.severity);
    let mut body = column![text(n.title.clone()).size(15)]
        .spacing(2)
        .width(Length::Fill);
    if let Some(detail) = &n.detail {
        body = body.push(text(detail.clone()).size(13));
    }
    container(
        row![
            body,
            icon_button(Icon::Close, 12.0, Some(Msg::CloseToast(n.id.clone()))),
        ]
        .spacing(8)
        .align_y(Alignment::Start),
    )
    .padding(12)
    .style(move |_| {
        container::Style::default()
            .background(Background::Color(theme::SURFACE))
            .border(iced::border::rounded(10).color(color).width(1.5))
            .color(theme::TEXT)
    })
    .into()
}
