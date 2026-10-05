//! Small widgets shared by every screen (007 T013): icons, one-line text, and button styles.

pub mod scroll_watch;
pub mod theme;

use iced::widget::{button, container, svg, text, Svg};
use iced::{Element, Length, Theme};

/// The app's icons, drawn as SVG so they scale and take the text colour.
///
/// Glyphs follow Lucide (<https://lucide.dev>), ISC licence: Copyright (c) for portions of Lucide
/// are held by Cole Bemis 2013-2022 as part of Feather (MIT); all other copyright (c) for Lucide
/// is held by Lucide Contributors 2022. Permission to use, copy, modify, and/or distribute this
/// software for any purpose with or without fee is hereby granted, provided that the above
/// copyright notice and this permission notice appear in all copies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Home,
    Scenes,
    Bell,
    Settings,
    Server,
    Close,
    Plus,
    Back,
    Forward,
    Play,
    Pause,
    More,
}

impl Icon {
    fn body(self) -> &'static str {
        match self {
            Icon::Home => {
                r#"<path d="m3 9 9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/><path d="M9 22V12h6v10"/>"#
            }
            Icon::Scenes => {
                r#"<circle cx="12" cy="12" r="10"/><polygon points="10 8 16 12 10 16 10 8"/>"#
            }
            Icon::Bell => {
                r#"<path d="M6 8a6 6 0 0 1 12 0c0 7 3 9 3 9H3s3-2 3-9"/><path d="M10.3 21a1.94 1.94 0 0 0 3.4 0"/>"#
            }
            Icon::Settings => {
                r#"<path d="M20 7h-9"/><path d="M14 17H5"/><circle cx="17" cy="17" r="3"/><circle cx="7" cy="7" r="3"/>"#
            }
            Icon::Server => {
                r#"<rect x="2" y="2" width="20" height="8" rx="2"/><rect x="2" y="14" width="20" height="8" rx="2"/><path d="M6 6h.01"/><path d="M6 18h.01"/>"#
            }
            Icon::Close => r#"<path d="M18 6 6 18"/><path d="m6 6 12 12"/>"#,
            Icon::Plus => r#"<path d="M5 12h14"/><path d="M12 5v14"/>"#,
            Icon::Back => r#"<path d="m15 18-6-6 6-6"/>"#,
            Icon::Forward => r#"<path d="m9 18 6-6-6-6"/>"#,
            Icon::Play => r#"<polygon points="6 3 20 12 6 21 6 3"/>"#,
            Icon::Pause => {
                r#"<rect x="14" y="4" width="4" height="16" rx="1"/><rect x="6" y="4" width="4" height="16" rx="1"/>"#
            }
            Icon::More => {
                r#"<circle cx="5" cy="12" r="1"/><circle cx="12" cy="12" r="1"/><circle cx="19" cy="12" r="1"/>"#
            }
        }
    }

    fn svg_source(self) -> String {
        format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">{}</svg>"#,
            self.body()
        )
    }
}

/// An icon `size` pixels square, tinted with the theme's text colour.
pub fn icon<'a>(which: Icon, size: f32) -> Svg<'a, Theme> {
    svg(svg::Handle::from_memory(which.svg_source().into_bytes()))
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
        .style(|theme: &Theme, _status| svg::Style {
            color: Some(theme.palette().text),
        })
}

/// Text kept to one line: iced 0.14 can't ellipsize, so the line is clipped at its container's
/// edge (007 T013, recorded in research R10).
pub fn one_line<'a, Message: 'a>(
    content: impl text::IntoFragment<'a>,
    size: f32,
) -> Element<'a, Message> {
    container(text(content).size(size).wrapping(text::Wrapping::None))
        .clip(true)
        .width(Length::Fill)
        .into()
}

/// A borderless icon button (navigation bar, tab strip, overlays).
pub fn icon_button<'a, Message: Clone + 'a>(
    which: Icon,
    size: f32,
    on_press: Option<Message>,
) -> button::Button<'a, Message> {
    button(icon(which, size))
        .padding(6)
        .style(button::text)
        .on_press_maybe(on_press)
}
