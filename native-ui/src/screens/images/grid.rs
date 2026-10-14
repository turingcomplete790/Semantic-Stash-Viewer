//! The Images grid and wall (012 US1, contracts/images-ui.md): the browse layer's grid with image
//! cards: a square thumbnail at the card's size, title and date below (none in the wall), and a
//! small badge for animated and clip images. The card size slider sits in the top bar.

use iced::widget::{column, container, row, slider, stack, text};
use iced::{Alignment, Color, Element, Length};
use stash_core::images::{ImageCard, Media};

use super::state::{shell_msg, ImageKind, ImagesMsg, ImagesState, CARD_SIZES};
use crate::screens::browse::controls::Map;
use crate::screens::browse::grid::{self, clickable, lines, thumb, Pictures};
use crate::screens::browse::layout::{Layout, CARD_TEXT};
use crate::screens::scenes::grid::duration;
use crate::screens::{Context, Mode};
use crate::shell::{ShellMsg, TabId};
use crate::widgets::{icon, theme, Icon};

/// The badge's words: animated files loop, clips show their length.
pub fn badge(card: &ImageCard) -> Option<String> {
    match card.media {
        Media::Still => None,
        Media::MayAnimate => Some("loop".to_owned()),
        Media::Clip { duration: d } => Some(duration(d)),
    }
}

/// The picture with its badge in the top right corner.
fn picture(
    pictures: Pictures<'_>,
    card: &ImageCard,
    width: f32,
    height: f32,
) -> Element<'static, ShellMsg> {
    let still = thumb::<ImageKind>(pictures, card, width, height);
    let Some(words) = badge(card) else {
        return still;
    };
    let mark = container(
        row![icon(Icon::Play, 10.0), text(words).size(11)]
            .spacing(3)
            .align_y(Alignment::Center),
    )
    .padding([1, 5])
    .style(|_| container::Style {
        background: Some(Color::from_rgba(0.0, 0.0, 0.0, 0.7).into()),
        text_color: Some(theme::TEXT),
        border: iced::Border {
            radius: 4.0.into(),
            ..Default::default()
        },
        ..Default::default()
    });
    stack![
        still,
        container(mark)
            .padding(6)
            .width(Length::Fixed(width))
            .height(Length::Fixed(height))
            .align_x(Alignment::End)
            .align_y(Alignment::Start),
    ]
    .into()
}

/// An image card in `mode` (the browse grid's [`crate::screens::browse::state::Kind::card_view`]):
/// images have the grid and the wall only.
pub fn card_view(
    c: &ImageCard,
    index: usize,
    focused: bool,
    mode: Mode,
    layout: &Layout,
    pictures: Pictures<'_>,
    msg: Map<ImageKind>,
) -> Element<'static, ShellMsg> {
    match mode {
        Mode::Wall => {
            let (w, h) = (layout.wall_width(), layout.wall_height());
            clickable::<ImageKind>(
                picture(pictures, c, w, h),
                index,
                focused,
                Length::Fixed(w),
                h,
                msg,
            )
        }
        Mode::Grid | Mode::List | Mode::Queue => {
            let w = layout.card_width();
            let body = column![
                picture(pictures, c, w, layout.thumb_height()),
                lines(
                    &c.title,
                    c.date.as_deref().unwrap_or(""),
                    w,
                    14.0,
                    12.0,
                    2.0
                )
                .padding([4, 8]),
            ];
            clickable::<ImageKind>(
                body.into(),
                index,
                focused,
                Length::Fixed(w),
                layout.thumb_height() + CARD_TEXT,
                msg,
            )
        }
    }
}

/// The card size control: the web UI's zoom slider.
fn card_size(state: &ImagesState) -> Element<'static, ShellMsg> {
    row![
        text("Size").size(13).color(theme::MUTED),
        slider(0..=(CARD_SIZES.len() as u8 - 1), state.card_size, |v| {
            ShellMsg::Images(ImagesMsg::CardSize(v))
        })
        .step(1u8)
        .width(Length::Fixed(80.0)),
    ]
    .spacing(6)
    .align_y(Alignment::Center)
    .into()
}

pub fn view<'a>(state: &'a ImagesState, ctx: &Context<'a>, tab: TabId) -> Element<'a, ShellMsg> {
    let ctx = Context {
        layout: state.layout(&ctx.layout),
        ..*ctx
    };
    grid::view(
        &state.list,
        &ctx,
        tab,
        None,
        shell_msg,
        Some(card_size(state)),
        |list| list,
    )
}
