//! The queue view (010; research R1–R4): the Scenes tab's page as a compact list beside a panel
//! showing the selected scene. The panel is the 008 scene page's own state (`SceneState`), so its
//! details, chips, chip cards, sections, and markers are the scene page's. A keyboard selection
//! shows the card's still and title at once and loads details only once it rests (250 ms), so
//! moving through the list sends nothing for rows only passed over (FR-005); a click is deliberate
//! and loads at once. 011: a click plays, and a keyboard selection plays once it rests; a rest
//! started by a page arriving only loads (restores and refreshes never start playback).

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use iced::widget::{button, column, container, row, text, tooltip};
use iced::{Alignment, Element, Length};
use stash_core::playback::playlist::Repeat;
use stash_core::scenes::SceneCard;

use super::layout::{Layout, QueueFit, PAD, QUEUE_STRIP};
use super::thumbs::{thumb_key, Thumbs};
use super::ScenesMsg;
use crate::effects::Effect;
use crate::library::Library;
use crate::screens::scene::{self, SceneState};
use crate::shell::{ShellMsg, TabId};
use crate::widgets::{icon_in, theme, Icon};

/// How long the keyboard selection rests on a scene before its details load (010 R3).
pub const QUEUE_REST: Duration = Duration::from_millis(250);

static NEXT_REST: AtomicU64 = AtomicU64::new(1);

/// Which area of the queue view has the keyboard (F6 switches, 010 R5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QueueFocus {
    #[default]
    List,
    Panel,
}

/// What the queue does when the page it's loading arrives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PendingPlay {
    #[default]
    None,
    /// Play result index `i` (the playlist machine chose it: Next, Previous, a shuffled pick,
    /// Repeat all's return to page 1).
    Index(u32),
    /// Start the selection (`queue_selected` or the first card) with Autoplay on: new results, or
    /// a switch to a queue tab whose page isn't here yet (011 FR-004).
    Start,
    /// The keyboard crossed a page edge: select its card when the page arrives; it plays once it
    /// rests, as any keyboard move (011 R2).
    Keyboard,
}

/// How a selection was made: the keyboard waits for a rest (and plays then, 011), the pointer
/// loads at once, a page arriving waits for a rest that only loads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Via {
    Keyboard,
    Pointer,
    Arrival,
}

/// The queue view's transient state (data model "QueueState"); rebuilt on entry and on restore.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct QueueState {
    /// Index into the page's cards.
    pub selected: Option<usize>,
    /// Generation of the pending rest timer (250 ms); a timer for an older generation does
    /// nothing.
    pub rest: u64,
    /// The 008 scene state for the selected scene.
    pub panel: Option<Box<SceneState>>,
    pub focus: QueueFocus,
    pub pending_play: PendingPlay,
    /// The scene this tab is playing, marked in the list (the session keeps it current).
    pub playing: Option<String>,
    /// The pending rest's intent: `true` when a keyboard move started it (it plays when due),
    /// `false` when a page arrival started it (it only loads) (011 R2).
    pub rest_plays: bool,
}

impl QueueState {
    /// Select card `index` of `cards` (clamped to the page). The panel shows the card's title and
    /// still at once; its details load now for the pointer, after a rest for the keyboard. A
    /// selection change drops a panel for another scene.
    pub fn select(
        &mut self,
        index: usize,
        via: Via,
        cards: &[SceneCard],
        tab: TabId,
    ) -> Vec<Effect> {
        let Some(last) = cards.len().checked_sub(1) else {
            self.clear();
            return Vec::new();
        };
        let index = index.min(last);
        let card = &cards[index];
        self.selected = Some(index);
        if self.panel.as_ref().is_none_or(|p| p.scene_id != card.id) {
            self.panel = Some(Box::new(SceneState::new(&card.id, &card.title)));
        }
        // Any older timer is now stale.
        self.rest = NEXT_REST.fetch_add(1, Ordering::Relaxed);
        let Some(panel) = self.panel.as_mut() else {
            return Vec::new();
        };
        // A keyboard rest plays the scene unless it's the one playing (011 R2).
        let plays = via == Via::Keyboard && self.playing.as_deref() != Some(card.id.as_str());
        self.rest_plays = plays;
        match via {
            Via::Pointer => panel.enter(tab),
            Via::Keyboard | Via::Arrival if panel.needs_load() || plays => {
                vec![Effect::QueueRestDue {
                    tab,
                    generation: self.rest,
                    after: QUEUE_REST,
                }]
            }
            Via::Keyboard | Via::Arrival => Vec::new(),
        }
    }

    /// Whether this queue waits for rest `generation`.
    pub fn awaits_rest(&self, generation: u64) -> bool {
        self.rest == generation && self.panel.is_some()
    }

    /// Rest `generation` came due: the panel loads, if the selection is still the one it was for,
    /// and whether a keyboard move started the rest (the selection is then chosen, 011 R6).
    pub fn rest_due(&mut self, generation: u64, tab: TabId) -> (Vec<Effect>, bool) {
        if self.rest != generation {
            return (Vec::new(), false);
        }
        let plays = std::mem::take(&mut self.rest_plays);
        let Some(panel) = self.panel.as_mut() else {
            return (Vec::new(), false);
        };
        (panel.enter(tab), plays)
    }

    /// No selection: an empty page, or the results are changing.
    pub fn clear(&mut self) {
        self.selected = None;
        self.panel = None;
        self.rest = NEXT_REST.fetch_add(1, Ordering::Relaxed);
    }

    /// The panel's scene, if it is `id`.
    pub fn panel_for(&mut self, id: &str) -> Option<&mut SceneState> {
        self.panel.as_deref_mut().filter(|p| p.scene_id == id)
    }
}

/// The panel's picture: the cover once it's here, the card's still from the thumbnail cache
/// until then (no request), with Play over it.
fn media<'a>(
    panel: &'a SceneState,
    card: Option<&SceneCard>,
    thumbs: &'a Thumbs,
) -> Element<'a, ShellMsg> {
    let still = card
        .and_then(thumb_key)
        .and_then(|k| thumbs.get(&k).cloned());
    panel.cover_view_or(still)
}

/// The keyboard's area of the queue view has a thin outline (010 R5).
fn area<'a>(content: Element<'a, ShellMsg>, focused: bool, width: Length) -> Element<'a, ShellMsg> {
    container(content)
        .width(width)
        .height(Length::Fill)
        .style(move |_| container::Style {
            border: iced::Border {
                color: if focused {
                    theme::FROSTED_BLUE
                } else {
                    iced::Color::TRANSPARENT
                },
                width: 1.0,
                radius: 0.0.into(),
            },
            ..container::Style::default()
        })
        .into()
}

/// The playlist strip above the list (011 FR-009): Shuffle, Repeat (Off → All → One), and
/// Autoplay, each an icon button with a tooltip naming it and its state; on in the accent colour.
pub fn strip<'a>(shuffle: bool, repeat: Repeat, autoplay: bool) -> Element<'a, ShellMsg> {
    let control = |which: Icon, on: bool, tip: &'static str, msg: ScenesMsg| {
        let colour = if on {
            theme::FROSTED_BLUE
        } else {
            theme::MUTED
        };
        tooltip(
            button(icon_in(which, 18.0, colour))
                .padding(6)
                .style(button::text)
                .on_press(ShellMsg::Scenes(msg)),
            container(text(tip).size(12))
                .padding([4, 8])
                .style(container::rounded_box),
            tooltip::Position::Bottom,
        )
    };
    let (repeat_icon, repeat_tip) = match repeat {
        Repeat::Off => (Icon::Repeat, "Repeat: off (R)"),
        Repeat::All => (Icon::Repeat, "Repeat: all (R)"),
        Repeat::One => (Icon::RepeatOne, "Repeat: one (R)"),
    };
    let shuffle_tip = if shuffle {
        "Shuffle: on (X)"
    } else {
        "Shuffle: off (X)"
    };
    let autoplay_tip = if autoplay {
        "Autoplay: on (A)"
    } else {
        "Autoplay: off (A)"
    };
    container(
        row![
            control(
                Icon::Shuffle,
                shuffle,
                shuffle_tip,
                ScenesMsg::ToggleShuffle
            ),
            control(
                repeat_icon,
                repeat != Repeat::Off,
                repeat_tip,
                ScenesMsg::CycleRepeat
            ),
            control(
                Icon::Autoplay,
                autoplay,
                autoplay_tip,
                ScenesMsg::ToggleAutoplay
            ),
        ]
        .spacing(4)
        .align_y(Alignment::Center),
    )
    .height(Length::Fixed(QUEUE_STRIP))
    .padding([0.0, PAD - 6.0])
    .align_y(Alignment::Center)
    .into()
}

/// The queue view: `strip` above `list` (the windowed rows and the pager) on the left, the panel on the right
/// with `player` in its picture slot when this tab's scene plays there.
#[allow(clippy::too_many_arguments)]
pub fn view<'a>(
    queue: &'a QueueState,
    cards: &'a [SceneCard],
    strip: Element<'a, ShellMsg>,
    list: Element<'a, ShellMsg>,
    player: Option<Element<'a, ShellMsg>>,
    window: &Layout,
    library: &'a Library,
    thumbs: &'a Thumbs,
) -> Element<'a, ShellMsg> {
    let list: Element<'a, ShellMsg> = column![strip, list].into();
    let fit = window.queue_fit();
    if fit == QueueFit::ListOnly {
        return list;
    }
    let card = queue.selected.and_then(|i| cards.get(i));
    let panel: Element<'a, ShellMsg> = match queue.panel.as_deref() {
        Some(p) => {
            let details = (fit == QueueFit::Both).then(|| p.details_view(library, thumbs));
            let picture = player.unwrap_or_else(|| media(p, card, thumbs));
            scene::panel_layout(picture, details, window)
        }
        None => container(column![])
            .width(Length::Fill)
            .height(Length::Fill)
            .into(),
    };
    row![
        area(
            list,
            queue.focus == QueueFocus::List,
            Length::Fixed(window.queue_list_width())
        ),
        area(panel, queue.focus == QueueFocus::Panel, Length::Fill),
    ]
    .into()
}
