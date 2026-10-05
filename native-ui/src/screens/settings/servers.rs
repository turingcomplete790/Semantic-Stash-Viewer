//! Settings → Servers (007 T056; capability C4): every saved server, to edit (re-tested before
//! saving, or saved anyway), delete (after a confirmation), reorder, or add.

use iced::widget::{button, checkbox, column, container, row, text, text_input, Space};
use iced::{Alignment, Element, Length};
use stash_core::profiles::{ProfileDraft, ServerProfile};
use stash_core::AppError;
use uuid::Uuid;

use super::SettingsMsg;
use crate::effects::Effect;
use crate::messages::for_error;
use crate::shell::ShellMsg;
use crate::widgets::{icon_button, theme, Icon};

/// What the editor holds while a server is being changed.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EditForm {
    pub name: String,
    pub address: String,
    pub api_key: String,
    pub show_key: bool,
    pub strict_tls: bool,
}

impl EditForm {
    fn from(p: &ServerProfile) -> Self {
        Self {
            name: p.display_name.clone(),
            address: p.base_url.to_string(),
            api_key: p.api_key.clone().unwrap_or_default(),
            show_key: false,
            strict_tls: p.strict_tls,
        }
    }

    fn draft(&self) -> ProfileDraft {
        ProfileDraft {
            display_name: (!self.name.trim().is_empty()).then(|| self.name.trim().to_owned()),
            address: self.address.trim().to_owned(),
            api_key: (!self.api_key.trim().is_empty()).then(|| self.api_key.trim().to_owned()),
            strict_tls: self.strict_tls,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum Editor {
    #[default]
    Closed,
    Editing {
        id: Uuid,
        form: EditForm,
    },
    /// Testing the server, then saving.
    Testing {
        id: Uuid,
        form: EditForm,
    },
    /// The test failed; the form keeps what was typed.
    Error {
        id: Uuid,
        form: EditForm,
        message: String,
    },
}

impl Editor {
    fn parts(&mut self) -> Option<(Uuid, &mut EditForm)> {
        match self {
            Editor::Editing { id, form } | Editor::Error { id, form, .. } => Some((*id, form)),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ServersState {
    pub list: Vec<ServerProfile>,
    pub editor: Editor,
    pub confirm_delete: Option<Uuid>,
    /// The last delete or reorder failure.
    pub problem: Option<String>,
}

#[derive(Debug, Clone)]
pub enum ServersMsg {
    Edit(Uuid),
    Name(String),
    Address(String),
    ApiKey(String),
    ToggleShowKey,
    StrictTls(bool),
    Save,
    SaveAnyway,
    Cancel,
    Delete(Uuid),
    ConfirmDelete,
    CancelDelete,
    MoveUp(Uuid),
    MoveDown(Uuid),
    Add,
}

impl ServersState {
    pub fn loaded(&mut self, list: Vec<ServerProfile>) {
        self.list = list;
    }

    /// A save finished for server `id`.
    pub fn saved(&mut self, id: Uuid, result: Result<ServerProfile, AppError>) {
        let form = match std::mem::take(&mut self.editor) {
            Editor::Testing { id: editing, form } if editing == id => form,
            other => {
                self.editor = other;
                if let Ok(p) = result {
                    self.replace(p);
                }
                return;
            }
        };
        match result {
            Ok(p) => self.replace(p),
            Err(e) => {
                let m = for_error(&e);
                self.editor = Editor::Error {
                    id,
                    form,
                    message: format!("{} {}", m.title, m.detail),
                };
            }
        }
    }

    pub fn deleted(&mut self, result: Result<ServerProfile, AppError>) {
        match result {
            Ok(p) => self.list.retain(|s| s.id != p.id),
            Err(e) => self.problem = Some(for_error(&e).title),
        }
    }

    fn replace(&mut self, p: ServerProfile) {
        match self.list.iter_mut().find(|s| s.id == p.id) {
            Some(slot) => *slot = p,
            None => self.list.push(p),
        }
    }

    fn save(&mut self, force: bool) -> Vec<Effect> {
        let Some((id, form)) = self.editor.parts() else {
            return Vec::new();
        };
        if form.address.trim().is_empty() {
            return Vec::new();
        }
        let form = form.clone();
        let draft = form.draft();
        self.editor = Editor::Testing { id, form };
        vec![Effect::SaveProfile { id, draft, force }]
    }

    fn reorder(&mut self, id: Uuid, up: bool) -> Vec<Effect> {
        let Some(i) = self.list.iter().position(|s| s.id == id) else {
            return Vec::new();
        };
        let j = if up {
            match i.checked_sub(1) {
                Some(j) => j,
                None => return Vec::new(),
            }
        } else if i + 1 < self.list.len() {
            i + 1
        } else {
            return Vec::new();
        };
        self.list.swap(i, j);
        vec![Effect::ReorderProfiles(
            self.list.iter().map(|s| s.id).collect(),
        )]
    }

    /// `true` in the second slot: open the "Add a server" form (the session's).
    pub fn update(&mut self, msg: ServersMsg) -> (Vec<Effect>, bool) {
        let edit = |s: &mut Self, f: &dyn Fn(&mut EditForm)| {
            if let Some((_, form)) = s.editor.parts() {
                f(form);
            }
        };
        let effects = match msg {
            ServersMsg::Edit(id) => {
                if let Some(p) = self.list.iter().find(|s| s.id == id) {
                    self.editor = Editor::Editing {
                        id,
                        form: EditForm::from(p),
                    };
                }
                Vec::new()
            }
            ServersMsg::Name(v) => {
                edit(self, &|f| f.name.clone_from(&v));
                Vec::new()
            }
            ServersMsg::Address(v) => {
                edit(self, &|f| f.address.clone_from(&v));
                Vec::new()
            }
            ServersMsg::ApiKey(v) => {
                edit(self, &|f| f.api_key.clone_from(&v));
                Vec::new()
            }
            ServersMsg::ToggleShowKey => {
                edit(self, &|f| f.show_key = !f.show_key);
                Vec::new()
            }
            ServersMsg::StrictTls(on) => {
                edit(self, &|f| f.strict_tls = on);
                Vec::new()
            }
            ServersMsg::Save => self.save(false),
            ServersMsg::SaveAnyway => self.save(true),
            ServersMsg::Cancel => {
                self.editor = Editor::Closed;
                Vec::new()
            }
            ServersMsg::Delete(id) => {
                self.confirm_delete = Some(id);
                Vec::new()
            }
            ServersMsg::ConfirmDelete => match self.confirm_delete.take() {
                Some(id) => vec![Effect::DeleteProfile(id)],
                None => Vec::new(),
            },
            ServersMsg::CancelDelete => {
                self.confirm_delete = None;
                Vec::new()
            }
            ServersMsg::MoveUp(id) => self.reorder(id, true),
            ServersMsg::MoveDown(id) => self.reorder(id, false),
            ServersMsg::Add => return (Vec::new(), true),
        };
        (effects, false)
    }

    pub fn view<'a>(&'a self, active: Uuid) -> Element<'a, ShellMsg> {
        let msg = |m: ServersMsg| ShellMsg::Settings(SettingsMsg::Servers(m));
        let mut list = column![row![
            text("Servers").size(22),
            Space::new().width(Length::Fill),
            button(text("Add a server…").size(13))
                .style(button::secondary)
                .on_press(msg(ServersMsg::Add)),
        ]
        .align_y(Alignment::Center)]
        .spacing(10);
        let last = self.list.len().saturating_sub(1);
        for (i, s) in self.list.iter().enumerate() {
            let is_active = s.id == active;
            let mut info = column![
                row![
                    text(s.display_name.clone()).size(16),
                    text(if is_active { "Active" } else { "" })
                        .size(12)
                        .color(theme::ICY_AQUA),
                ]
                .spacing(8),
                text(s.base_url.to_string()).size(12).color(theme::MUTED),
                text(format!(
                    "{} · {}",
                    if s.api_key.is_some() {
                        "API key saved"
                    } else {
                        "No API key"
                    },
                    if s.strict_tls {
                        "strict certificate checking"
                    } else {
                        "certificates not checked"
                    }
                ))
                .size(12)
                .color(theme::MUTED),
            ]
            .spacing(2)
            .width(Length::Fill);
            if self.confirm_delete == Some(s.id) {
                info = info.push(
                    row![
                        text("Delete this server and its cache?").size(13),
                        button(text("Delete").size(13))
                            .style(button::danger)
                            .on_press(msg(ServersMsg::ConfirmDelete)),
                        button(text("Keep").size(13))
                            .style(button::text)
                            .on_press(msg(ServersMsg::CancelDelete)),
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center),
                );
            }
            let controls = row![
                icon_button(
                    Icon::Back,
                    14.0,
                    (i > 0).then(|| msg(ServersMsg::MoveUp(s.id)))
                )
                .style(button::text),
                icon_button(
                    Icon::Forward,
                    14.0,
                    (i < last).then(|| msg(ServersMsg::MoveDown(s.id)))
                ),
                button(text("Edit").size(13))
                    .style(button::secondary)
                    .on_press(msg(ServersMsg::Edit(s.id))),
                button(text("Delete").size(13))
                    .style(button::text)
                    .on_press_maybe((!is_active).then(|| msg(ServersMsg::Delete(s.id)))),
            ]
            .spacing(6)
            .align_y(Alignment::Center);
            list = list.push(
                container(row![info, controls].spacing(12).align_y(Alignment::Center))
                    .padding(12)
                    .style(|_| {
                        container::Style::default()
                            .background(iced::Background::Color(theme::SURFACE))
                            .border(iced::border::rounded(8))
                    }),
            );
            if let Editor::Editing { id, form }
            | Editor::Testing { id, form }
            | Editor::Error { id, form, .. } = &self.editor
            {
                if *id == s.id {
                    list = list.push(self.editor_view(form));
                }
            }
        }
        if let Some(problem) = &self.problem {
            list = list.push(text(problem.clone()).size(13).color(theme::ROSEWOOD));
        }
        list = list.push(
            text("The active server can't be deleted: switch to another one first.")
                .size(12)
                .color(theme::MUTED),
        );
        list.into()
    }

    fn editor_view<'a>(&'a self, form: &'a EditForm) -> Element<'a, ShellMsg> {
        let msg = |m: ServersMsg| ShellMsg::Settings(SettingsMsg::Servers(m));
        let testing = matches!(self.editor, Editor::Testing { .. });
        let field = |label: &'a str, value: &'a str, on: fn(String) -> ServersMsg| {
            column![
                text(label).size(12),
                text_input(label, value)
                    .on_input_maybe((!testing).then_some(move |v| msg(on(v))))
                    .padding(8)
                    .size(14),
            ]
            .spacing(4)
        };
        let key = row![
            text_input("API key (leave empty for none)", &form.api_key)
                .secure(!form.show_key)
                .on_input_maybe((!testing).then_some(move |v| msg(ServersMsg::ApiKey(v))))
                .padding(8)
                .size(14),
            button(text(if form.show_key { "Hide" } else { "Show" }).size(13))
                .style(button::text)
                .on_press(msg(ServersMsg::ToggleShowKey)),
        ]
        .spacing(8)
        .align_y(Alignment::Center);
        let mut body = column![
            field("Name", &form.name, ServersMsg::Name),
            field("Address", &form.address, ServersMsg::Address),
            column![text("API key").size(12), key].spacing(4),
            checkbox(form.strict_tls)
                .label("Verify the server's certificate (strict)")
                .on_toggle_maybe((!testing).then_some(move |v| msg(ServersMsg::StrictTls(v)))),
        ]
        .spacing(10);
        if let Editor::Error { message, .. } = &self.editor {
            body = body.push(
                container(text(message.clone()).size(13))
                    .padding(10)
                    .style(theme::problem),
            );
        }
        let mut actions = row![
            button(
                text(if testing {
                    "Testing…"
                } else {
                    "Test and save"
                })
                .size(13)
            )
            .on_press_maybe((!testing).then(|| msg(ServersMsg::Save))),
            button(text("Cancel").size(13))
                .style(button::text)
                .on_press_maybe((!testing).then(|| msg(ServersMsg::Cancel))),
        ]
        .spacing(8);
        if matches!(self.editor, Editor::Error { .. }) {
            actions = actions.push(
                button(text("Save anyway").size(13))
                    .style(button::secondary)
                    .on_press(msg(ServersMsg::SaveAnyway)),
            );
        }
        container(body.push(actions))
            .padding(14)
            .style(theme::panel)
            .into()
    }
}
