//! Settings → Playback (008 FR-012, FR-017): "Resume where you left off" and "Save play history".
//! The switches are the app's own (the same for every server) and save at once.

use iced::widget::{column, text, toggler};
use iced::Element;

use crate::services::settings::PlaybackPreferences;
use crate::shell::ShellMsg;
use crate::widgets::theme;

/// A switch the user flipped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackChange {
    Resume(bool),
    SaveHistory(bool),
}

impl PlaybackChange {
    pub fn apply(self, mut prefs: PlaybackPreferences) -> PlaybackPreferences {
        match self {
            PlaybackChange::Resume(on) => prefs.resume = on,
            PlaybackChange::SaveHistory(on) => prefs.save_history = on,
        }
        prefs
    }
}

pub fn view<'a>(prefs: PlaybackPreferences) -> Element<'a, ShellMsg> {
    let note = |s: &'static str| text(s).size(13).color(theme::MUTED);
    column![
        text("Playback").size(22),
        toggler(prefs.resume)
            .label("Resume where you left off")
            .on_toggle(|on| ShellMsg::PlaybackChange(PlaybackChange::Resume(on))),
        note("Play starts where you stopped last time, with a button to start from the beginning."),
        toggler(prefs.save_history)
            .label("Save play history")
            .on_toggle(|on| ShellMsg::PlaybackChange(PlaybackChange::SaveHistory(on))),
        note(
            "Sends your resume point, time watched, and play count to the server, as the \
             Stash web player does. Off: nothing about your viewing is sent or kept."
        ),
    ]
    .spacing(12)
    .into()
}
