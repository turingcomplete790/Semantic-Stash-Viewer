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

/// A card that floats over a panel (a chip's details): framed in cerulean, lifted by a shadow.
pub fn hover_card(_: &Theme) -> container::Style {
    container::Style::default()
        .background(Background::Color(SURFACE))
        .border(Border {
            color: CERULEAN,
            width: 1.5,
            radius: 10.0.into(),
        })
        .shadow(iced::Shadow {
            color: Color {
                a: 0.5,
                ..Color::BLACK
            },
            offset: iced::Vector::new(0.0, 4.0),
            blur_radius: 16.0,
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
/// A list row the keyboard is on (a frosted outline), or a plain one.
pub fn row_highlight(on: bool) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        border: Border {
            radius: 6.0.into(),
            width: if on { 1.0 } else { 0.0 },
            color: FROSTED_BLUE,
        },
        ..container::Style::default()
    }
}

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

/// The tab strip under the navigation bar.
pub fn strip(_: &Theme) -> container::Style {
    container::Style::default().background(Background::Color(BACKGROUND))
}

/// A scene card or list row; `focused` is the keyboard's card.
pub fn card(focused: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
        let edge = if focused {
            FROSTED_BLUE
        } else if hovered {
            CERULEAN
        } else {
            Color::TRANSPARENT
        };
        button::Style {
            background: Some(Background::Color(if hovered { YALE_BLUE } else { SURFACE })),
            text_color: TEXT,
            border: border::rounded(8).color(edge).width(2.0),
            ..button::Style::default()
        }
    }
}

// ---- Tag chips (008 FR-003d; constitution Principle XI) ---------------------------------------

/// Dark chip text (the RPG palette's darkest).
pub const CHIP_DARK: stash_core::semantic::Rgb = stash_core::semantic::Rgb(0x0d, 0x0a, 0x05);
/// Light chip text (the RPG palette's light text).
pub const CHIP_LIGHT: stash_core::semantic::Rgb = stash_core::semantic::Rgb(0xe8, 0xe0, 0xc8);

fn luminance(c: stash_core::semantic::Rgb) -> f64 {
    let channel = |v: u8| {
        let v = f64::from(v) / 255.0;
        if v <= 0.039_28 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(c.0) + 0.7152 * channel(c.1) + 0.0722 * channel(c.2)
}

/// The WCAG contrast ratio between two colours (1 to 21).
pub fn contrast(a: stash_core::semantic::Rgb, b: stash_core::semantic::Rgb) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// The text colour on a chip of `background` (constitution Principle XI: at least 4.5:1): the RPG
/// palette's dark or light text, whichever contrasts more; when neither reaches 4.5:1 (the
/// palette's red and purple are mid-tones), pure black or white, whichever contrasts more.
pub fn chip_text(background: stash_core::semantic::Rgb) -> stash_core::semantic::Rgb {
    use stash_core::semantic::Rgb;
    let best = |a: Rgb, b: Rgb| {
        if contrast(background, a) >= contrast(background, b) {
            a
        } else {
            b
        }
    };
    let rpg = best(CHIP_DARK, CHIP_LIGHT);
    if contrast(background, rpg) >= 4.5 {
        rpg
    } else {
        best(Rgb(0, 0, 0), Rgb(0xff, 0xff, 0xff))
    }
}

/// A tag chip's style: its rule colour (text picked by [`chip_text`]), or the app's usual chip
/// when it has none.
pub fn tag_chip(
    colour: Option<stash_core::semantic::Rgb>,
) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let (bg, fg) = match colour {
            Some(c) => {
                let t = chip_text(c);
                (rgb(c.0, c.1, c.2), rgb(t.0, t.1, t.2))
            }
            None => (YALE_BLUE, TEXT),
        };
        button::Style {
            background: Some(Background::Color(bg)),
            text_color: fg,
            border: Border {
                radius: 12.0.into(),
                width: if matches!(status, button::Status::Hovered) {
                    1.0
                } else {
                    0.0
                },
                color: FROSTED_BLUE,
            },
            ..button::Style::default()
        }
    }
}

#[cfg(test)]
mod chip_tests {
    use super::*;
    use stash_core::semantic::{default_colour_rules, Rgb};

    #[test]
    fn chip_text_reaches_4_5_to_1_on_every_default_and_some_picked_colours() {
        let mut colours: Vec<Rgb> = default_colour_rules()
            .into_iter()
            .map(|r| r.colour)
            .collect();
        colours.extend([
            Rgb(0, 0, 0),
            Rgb(0xff, 0xff, 0xff),
            Rgb(0x80, 0x80, 0x80),
            Rgb(0xc3, 0x3c, 0x54),
            Rgb(0x25, 0x4e, 0x70),
            Rgb(0x8e, 0xe3, 0xef),
        ]);
        for c in colours {
            let ratio = contrast(c, chip_text(c));
            assert!(ratio >= 4.5, "{c}: {ratio:.2}");
        }
    }

    #[test]
    fn the_rpg_text_colours_are_used_when_they_are_enough() {
        assert_eq!(chip_text(Rgb(0x7d, 0xd8, 0x7a)), CHIP_DARK);
        assert_eq!(chip_text(Rgb(0x2a, 0x50, 0x90)), CHIP_LIGHT);
    }
}
