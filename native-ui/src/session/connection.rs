//! The session's connection region (007 data model "Session"; capabilities C2, C3; behaviour that
//! stays B6), driven by the core's connection snapshots. Connecting, offline, and failure states
//! don't tear down the shell: cached screens keep working (003).

use iced::widget::{button, container, row, text};
use iced::{Alignment, Background, Color, Element, Theme};
use stash_core::connection::snapshot::{ConnectionSnapshot, SessionState};
use stash_core::connection::SecurityState;

use crate::messages::for_connect;
use crate::widgets::theme;

#[derive(Debug, Clone, Default)]
pub struct Connection {
    pub snapshot: ConnectionSnapshot,
}

/// Events the session cares about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Up {
    /// The session reached `Connected` (from any other state).
    BecameConnected,
    /// The server needs a (new) API key: ask for it.
    KeyNeeded,
}

impl Connection {
    pub fn connected(&self) -> bool {
        matches!(self.snapshot.state, SessionState::Connected)
    }

    /// A new snapshot from the core.
    pub fn observe(&mut self, snapshot: ConnectionSnapshot) -> Option<Up> {
        let was_connected = self.connected();
        let was_auth_failed = matches!(self.snapshot.state, SessionState::AuthFailed { .. });
        self.snapshot = snapshot;
        match &self.snapshot.state {
            SessionState::Connected if !was_connected => Some(Up::BecameConnected),
            SessionState::AuthFailed { .. } if !was_auth_failed => Some(Up::KeyNeeded),
            _ => None,
        }
    }

    /// The state in a few words.
    pub fn label(&self) -> String {
        match &self.snapshot.state {
            SessionState::Idle => "Not connected".into(),
            SessionState::Connecting { .. } => "Connecting…".into(),
            SessionState::Connected => "Connected".into(),
            SessionState::Offline { .. } => "Offline, retrying".into(),
            SessionState::AuthFailed { failure } | SessionState::Failed { failure } => {
                for_connect(failure).title
            }
        }
    }

    /// Whether the connection is unencrypted, unverified, or verified (Principle VII: always
    /// visible).
    pub fn security_label(&self) -> &'static str {
        match self.snapshot.security {
            Some(SecurityState::Unencrypted) => "Unencrypted (http)",
            Some(SecurityState::EncryptedUnverified) => "Encrypted, certificate not verified",
            Some(SecurityState::EncryptedVerified) => "Encrypted and verified",
            None => "Security not known yet",
        }
    }

    fn dot(&self) -> Color {
        match &self.snapshot.state {
            SessionState::Connected => theme::ICY_AQUA,
            SessionState::Connecting { .. } | SessionState::Idle => theme::FROSTED_BLUE,
            SessionState::Offline { .. } => Color {
                a: 0.6,
                ..theme::ROSEWOOD
            },
            SessionState::AuthFailed { .. } | SessionState::Failed { .. } => theme::ROSEWOOD,
        }
    }

    /// The indicator: a coloured dot, the server's name, the state, and the security state.
    /// Pressing it opens the server menu.
    pub fn indicator<'a, M: Clone + 'a>(&self, server: &str, on_press: M) -> Element<'a, M> {
        let dot = self.dot();
        button(
            row![
                container(text(""))
                    .width(10)
                    .height(10)
                    .style(move |_: &Theme| container::Style::default()
                        .background(Background::Color(dot))
                        .border(iced::border::rounded(5))),
                text(server.to_owned()).size(14),
                text(format!("· {} · {}", self.label(), self.security_label())).size(12),
                text("▾").size(12),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .padding([4, 12])
        .style(theme::chip)
        .on_press(on_press)
        .into()
    }
}
