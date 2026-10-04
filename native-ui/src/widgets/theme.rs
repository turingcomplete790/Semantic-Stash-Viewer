//! The app's colours and the styles built from them.
//!
//! Palette (the user's, 2026-10-04): rosewood `#c33c54`, yale blue `#254e70`, cerulean `#37718e`,
//! frosted blue `#8ee3ef`, icy aqua `#aef3e7`. The base is a dark blue drawn from yale blue so the
//! accents stand out; cerulean marks things to press, frosted blue and icy aqua mark what's active
//! or healthy, rosewood marks problems.

use iced::theme::Palette;
use iced::widget::{button, container};
use iced::{border, Background, Border, Color, Theme};

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::from_rgb8(r, g, b)
}

pub const ROSEWOOD: Color = rgb(0xc3, 0x3c, 0x54);
pub const YALE_BLUE: Color = rgb(0x25, 0x4e, 0x70);
pub const CERULEAN: Color = rgb(0x37, 0x71, 0x8e);
pub const FROSTED_BLUE: Color = rgb(0x8e, 0xe3, 0xef);
pub const ICY_AQUA: Color = rgb(0xae, 0xf3, 0xe7);

/// The window background: yale blue, much darker.
pub const BACKGROUND: Color = rgb(0x0e, 0x1d, 0x2b);
/// Panels and bars: between the background and yale blue.
pub const SURFACE: Color = rgb(0x16, 0x2f, 0x46);
/// Body text: near white, leaning to icy aqua.
pub const TEXT: Color = rgb(0xe6, 0xf7, 0xf5);
/// Secondary text.
pub const MUTED: Color = rgb(0x9f, 0xbd, 0xc9);

/// The app's theme.
pub fn theme() -> Theme {
    Theme::custom(
        "Semantic Stash",
        Palette {
            background: BACKGROUND,
            text: TEXT,
            primary: CERULEAN,
            success: ICY_AQUA,
            warning: FROSTED_BLUE,
            danger: ROSEWOOD,
        },
    )
}

/// The top bar.
pub fn bar(_: &Theme) -> container::Style {
    container::Style::default()
        .background(Background::Color(SURFACE))
        .border(Border {
            color: YALE_BLUE,
            width: 1.0,
            radius: 0.0.into(),
        })
}

/// A floating panel (menus, prompts).
pub fn panel(_: &Theme) -> container::Style {
    container::Style::default()
        .background(Background::Color(SURFACE))
        .border(Border {
            color: CERULEAN,
            width: 1.0,
            radius: 10.0.into(),
        })
        .color(TEXT)
}

/// A notice that something went wrong.
pub fn problem(_: &Theme) -> container::Style {
    container::Style::default()
        .background(Background::Color(Color {
            a: 0.18,
            ..ROSEWOOD
        }))
        .border(Border {
            color: ROSEWOOD,
            width: 1.0,
            radius: 8.0.into(),
        })
        .color(TEXT)
}

/// A clickable chip (the server indicator): visibly a control, brighter on hover.
pub fn chip(_: &Theme, status: button::Status) -> button::Style {
    let (bg, edge) = match status {
        button::Status::Hovered | button::Status::Pressed => (CERULEAN, FROSTED_BLUE),
        _ => (YALE_BLUE, CERULEAN),
    };
    button::Style {
        background: Some(Background::Color(bg)),
        text_color: TEXT,
        border: border::rounded(16).color(edge).width(1.0),
        ..button::Style::default()
    }
}

/// A row in a menu; `active` marks the current choice.
pub fn menu_row(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let bg = match (active, status) {
            (true, _) => Some(Background::Color(Color {
                a: 0.35,
                ..CERULEAN
            })),
            (false, button::Status::Hovered | button::Status::Pressed) => {
                Some(Background::Color(YALE_BLUE))
            }
            (false, button::Status::Disabled) => None,
            (false, _) => None,
        };
        button::Style {
            background: bg,
            text_color: if active { ICY_AQUA } else { TEXT },
            border: border::rounded(6),
            ..button::Style::default()
        }
    }
}
