//! Settings → About (007 T056; capability C13): what this is, which version, which server, where
//! its files live, and whose work it builds on.

use iced::widget::{column, text};
use iced::Element;

use crate::screens::Context;
use crate::services::paths::Paths;
use crate::shell::ShellMsg;
use crate::widgets::theme;

pub fn view<'a>(ctx: &Context<'a>) -> Element<'a, ShellMsg> {
    let build = if cfg!(debug_assertions) {
        "development build"
    } else {
        "release build"
    };
    let server = match &ctx.connection.snapshot.server {
        Some(info) => format!(
            "Connected to {} running Stash {}.",
            ctx.profile.display_name, info.version
        ),
        None => format!("{}: {}.", ctx.profile.display_name, ctx.connection.label()),
    };
    let mut col = column![
        text("About").size(22),
        text("Semantic Stash Viewer").size(18),
        text(format!("Version {} ({build}).", env!("CARGO_PKG_VERSION"))).size(14),
        text(server).size(14),
    ]
    .spacing(8);
    if let Some(paths) = Paths::resolve() {
        col = col.push(text("Files").size(16)).push(
            text(format!(
                "Settings: {}\nData (session, notifications, logs): {}\nCache: {}",
                paths.config.display(),
                paths.data.display(),
                paths.cache.display()
            ))
            .size(13)
            .color(theme::MUTED),
        );
    }
    col.push(text("Licences").size(16))
        .push(
            text(
                "MIT licence. Built with iced (MIT), mpv/libmpv (LGPL 2.1+), wgpu (MIT/Apache 2.0), \
                 and Lucide icons (ISC). Talks to your Stash server; nothing is sent anywhere else.",
            )
            .size(13)
            .color(theme::MUTED),
        )
        .into()
}
