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
    /// Lucide `image` (012).
    Images,
    /// The image viewer (012): Lucide `chevron-left`, `chevron-right`, `rotate-cw`, `maximize`,
    /// `minimize`, `info`.
    ChevronLeft,
    ChevronRight,
    RotateCw,
    Maximize,
    Minimize,
    Info,
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
    /// The playlist controls (011): Lucide `shuffle`, `repeat`, `repeat-1`, and Autoplay (a
    /// switch with a play glyph, in the same stroke).
    Shuffle,
    Repeat,
    RepeatOne,
    Autoplay,
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
            Icon::Images => {
                r#"<rect width="18" height="18" x="3" y="3" rx="2" ry="2"/><circle cx="9" cy="9" r="2"/><path d="m21 15-3.086-3.086a2 2 0 0 0-2.828 0L6 21"/>"#
            }
            Icon::ChevronLeft => r#"<path d="m15 18-6-6 6-6"/>"#,
            Icon::ChevronRight => r#"<path d="m9 18 6-6-6-6"/>"#,
            Icon::RotateCw => {
                r#"<path d="M21 12a9 9 0 1 1-9-9c2.52 0 4.93 1 6.74 2.74L21 8"/><path d="M21 3v5h-5"/>"#
            }
            Icon::Maximize => {
                r#"<path d="M8 3H5a2 2 0 0 0-2 2v3"/><path d="M21 8V5a2 2 0 0 0-2-2h-3"/><path d="M3 16v3a2 2 0 0 0 2 2h3"/><path d="M16 21h3a2 2 0 0 0 2-2v-3"/>"#
            }
            Icon::Minimize => {
                r#"<path d="M8 3v3a2 2 0 0 1-2 2H3"/><path d="M21 8h-3a2 2 0 0 1-2-2V3"/><path d="M3 16h3a2 2 0 0 1 2 2v3"/><path d="M16 21v-3a2 2 0 0 1 2-2h3"/>"#
            }
            Icon::Info => {
                r#"<circle cx="12" cy="12" r="10"/><path d="M12 16v-4"/><path d="M12 8h.01"/>"#
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
            Icon::Shuffle => {
                r#"<path d="m18 14 4 4-4 4"/><path d="m18 2 4 4-4 4"/><path d="M2 18h1.973a4 4 0 0 0 3.3-1.7l5.454-8.6a4 4 0 0 1 3.3-1.7H22"/><path d="M2 6h1.972a4 4 0 0 1 3.6 2.2"/><path d="M22 18h-6.041a4 4 0 0 1-3.3-1.8l-.359-.45"/>"#
            }
            Icon::Repeat => {
                r#"<path d="m17 2 4 4-4 4"/><path d="M3 11v-1a4 4 0 0 1 4-4h14"/><path d="m7 22-4-4 4-4"/><path d="M21 13v1a4 4 0 0 1-4 4H3"/>"#
            }
            Icon::RepeatOne => {
                r#"<path d="m17 2 4 4-4 4"/><path d="M3 11v-1a4 4 0 0 1 4-4h14"/><path d="m7 22-4-4 4-4"/><path d="M21 13v1a4 4 0 0 1-4 4H3"/><path d="M11 10h1v4"/>"#
            }
            Icon::Autoplay => {
                r#"<rect x="2" y="6" width="20" height="12" rx="6"/><polygon points="13 9.5 18 12 13 14.5 13 9.5"/>"#
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

/// An icon `size` pixels square in `color` (a control's state: the accent when on).
pub fn icon_in<'a>(which: Icon, size: f32, color: iced::Color) -> Svg<'a, Theme> {
    svg(svg::Handle::from_memory(which.svg_source().into_bytes()))
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
        .style(move |_: &Theme, _status| svg::Style { color: Some(color) })
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
