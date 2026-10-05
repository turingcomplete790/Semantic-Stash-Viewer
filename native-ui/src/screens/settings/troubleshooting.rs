//! Settings → Troubleshooting (007 T056; capability C13): the cache's size and a way to clear it
//! (the library on the server is never touched), and the log folder.

use iced::widget::{button, column, container, row, text};
use iced::{Alignment, Element};
use stash_core::AppError;

use super::SettingsMsg;
use crate::effects::Effect;
use crate::messages::for_error;
use crate::shell::ShellMsg;
use crate::widgets::theme;

#[derive(Debug, Clone, PartialEq, Default)]
pub enum Clear {
    #[default]
    Idle,
    Clearing,
    /// Bytes freed.
    Cleared(u64),
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Troubleshooting {
    pub cache_size: Option<u64>,
    pub clear: Clear,
}

#[derive(Debug, Clone)]
pub enum TroubleMsg {
    Clear,
    OpenLogs,
}

/// `12.3 MB`.
pub fn bytes(n: u64) -> String {
    let n = n as f64;
    match n {
        n if n >= 1e9 => format!("{:.1} GB", n / 1e9),
        n if n >= 1e6 => format!("{:.1} MB", n / 1e6),
        n if n >= 1e3 => format!("{:.0} kB", n / 1e3),
        n => format!("{n:.0} bytes"),
    }
}

impl Troubleshooting {
    pub fn enter(&mut self) -> Vec<Effect> {
        self.clear = Clear::Idle;
        vec![Effect::ReadCacheSize]
    }

    pub fn cleared(&mut self, result: Result<u64, AppError>) {
        match result {
            Ok(freed) => {
                self.clear = Clear::Cleared(freed);
                self.cache_size = Some(0);
            }
            Err(e) => self.clear = Clear::Failed(for_error(&e).title),
        }
    }

    pub fn update(&mut self, msg: TroubleMsg) -> Vec<Effect> {
        match msg {
            TroubleMsg::Clear => {
                if self.clear == Clear::Clearing {
                    return Vec::new();
                }
                self.clear = Clear::Clearing;
                vec![Effect::ClearCache]
            }
            TroubleMsg::OpenLogs => vec![Effect::OpenLogFolder],
        }
    }

    pub fn view<'a>(&'a self, logs: String) -> Element<'a, ShellMsg> {
        let msg = |m: TroubleMsg| ShellMsg::Settings(SettingsMsg::Trouble(m));
        let size = self.cache_size.map_or_else(|| "Reading…".to_owned(), bytes);
        let status: Element<'a, ShellMsg> = match &self.clear {
            Clear::Idle => text("").into(),
            Clear::Clearing => text("Clearing…").size(13).into(),
            Clear::Cleared(freed) => text(format!("Cleared {}.", bytes(*freed)))
                .size(13)
                .color(theme::ICY_AQUA)
                .into(),
            Clear::Failed(m) => container(text(m.clone()).size(13))
                .padding(8)
                .style(theme::problem)
                .into(),
        };
        column![
            text("Troubleshooting").size(22),
            text("Cache").size(16),
            text(format!(
                "This server's cache holds {size}. Clearing it only removes copies on this computer: \
                 screens fetch again from Stash, and nothing in your library changes."
            ))
            .size(14),
            row![
                button(text("Clear cache").size(13))
                    .style(button::secondary)
                    .on_press_maybe((self.clear != Clear::Clearing).then(|| msg(TroubleMsg::Clear))),
                status,
            ]
            .spacing(12)
            .align_y(Alignment::Center),
            text("Logs").size(16),
            text(format!("The app writes its logs to {logs}.")).size(14),
            button(text("Open the log folder").size(13))
                .style(button::secondary)
                .on_press(msg(TroubleMsg::OpenLogs)),
        ]
        .spacing(10)
        .into()
    }
}
