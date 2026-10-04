//! The Home screen (007 T031; data model "Screen"): the server's summary and the connection's
//! details. The summary comes from the cache first (the last one read), so Home shows something
//! while connecting or offline, and fills in from the live connection when it arrives.

use iced::widget::{center, column, container, text};
use iced::{Alignment, Element, Length};
use serde::{Deserialize, Serialize};
use stash_core::connection::ServerInfo;

use super::Context;
use crate::widgets::theme;

/// The summary's state (transient: never saved).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Summary {
    #[default]
    Loading,
    Ready(ServerInfo),
    /// Nothing cached and the server can't be reached.
    Unreachable,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Home {
    #[serde(skip)]
    pub summary: Summary,
}

impl Home {
    pub fn needs_load(&self) -> bool {
        self.summary == Summary::Loading
    }

    /// A summary read from the cache (`None`: nothing cached). `unreachable` says the connection
    /// has failed, so there's nothing more to wait for.
    pub fn loaded(&mut self, info: Option<ServerInfo>, unreachable: bool) {
        match info {
            Some(info) => self.summary = Summary::Ready(info),
            None if unreachable && self.summary == Summary::Loading => {
                self.summary = Summary::Unreachable;
            }
            None => {}
        }
    }

    pub fn view<'a, M: 'a>(&'a self, ctx: &Context<'a>) -> Element<'a, M> {
        let mut body = column![
            text(ctx.profile.display_name.clone()).size(28),
            text(ctx.profile.base_url.to_string())
                .size(13)
                .color(theme::MUTED),
            text(format!(
                "{} · {}",
                ctx.connection.label(),
                ctx.connection.security_label()
            ))
            .size(14),
        ]
        .spacing(6)
        .align_x(Alignment::Center);
        body = body.push(match &self.summary {
            Summary::Loading => Element::from(text("Reading the server's summary…").size(14)),
            Summary::Unreachable => {
                container(text("The server can't be reached, and nothing is cached yet.").size(14))
                    .padding(10)
                    .style(theme::problem)
                    .into()
            }
            Summary::Ready(info) => column![
                text(format!("Stash {}", info.version)).size(16),
                text(format!(
                    "{} scenes · {} images · {} galleries · {} performers",
                    info.counts.scenes,
                    info.counts.images,
                    info.counts.galleries,
                    info.counts.performers
                ))
                .size(14),
            ]
            .spacing(4)
            .align_x(Alignment::Center)
            .into(),
        });
        center(container(body).padding(24).style(theme::panel))
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}
