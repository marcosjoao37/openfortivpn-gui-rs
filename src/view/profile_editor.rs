use super::OFGApp;
use crate::model::profile::Profile;
use egui::{Color32, RichText, TextEdit};
use uuid::Uuid;

const ERR_FG: Color32 = Color32::from_rgb(0xFF, 0x6B, 0x6B);

#[derive(Debug)]
pub struct EditorState {
    pub id: Option<String>,
    pub name: String,
    pub server: String,
    pub username: String,
    pub trust: bool,
    pub cert: String,
    pub iface: String,
    pub confirm_delete: bool,
    pub error: Option<String>,
}

impl EditorState {
    pub fn new(profile: Option<Profile>, fallback_name: String) -> Self {
        match profile {
            Some(p) => Self {
                id: Some(p.id),
                name: p.name,
                server: p.server,
                username: p.username,
                trust: p.trust_cert,
                cert: p.trusted_cert,
                iface: p.interface.unwrap_or_default(),
                confirm_delete: false,
                error: None,
            },
            None => Self {
                id: None,
                name: fallback_name,
                server: String::new(),
                username: String::new(),
                trust: false,
                cert: String::new(),
                iface: String::new(),
                confirm_delete: false,
                error: None,
            },
        }
    }

    fn to_profile(&self) -> Profile {
        Profile {
            id: self
                .id
                .clone()
                .unwrap_or_else(|| Uuid::new_v4().to_string()),
            name: self.name.trim().to_owned(),
            server: self.server.trim().to_owned(),
            username: self.username.trim().to_owned(),
            trust_cert: self.trust,
            trusted_cert: self.cert.trim().to_lowercase(),
            interface: {
                let s = self.iface.trim();
                (!s.is_empty()).then(|| s.to_owned())
            },
        }
    }
}

pub fn show(app: &mut OFGApp, ctx: &egui::Context) {
    if app.editor.is_none() {
        return;
    }
    let mut ed = app.editor.take().unwrap();
    let mut open = true;
    let mut done = false;
    let title = if ed.id.is_some() {
        "Edit profile"
    } else {
        "New profile"
    };
    egui::Window::new(title)
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            egui::Grid::new("editor-grid")
                .num_columns(2)
                .spacing([12.0, 10.0])
                .show(ui, |ui| {
                    ui.label("Name:");
                    ui.add(TextEdit::singleline(&mut ed.name).desired_width(320.0));
                    ui.end_row();

                    ui.label("Server:");
                    ui.add(
                        TextEdit::singleline(&mut ed.server)
                            .desired_width(320.0)
                            .hint_text("host:port"),
                    );
                    ui.end_row();

                    ui.label("User:");
                    ui.add(TextEdit::singleline(&mut ed.username).desired_width(320.0));
                    ui.end_row();

                    ui.label("Trust certificate:");
                    ui.checkbox(&mut ed.trust, "send --trusted-cert");
                    ui.end_row();

                    if ed.trust {
                        ui.label("Certificate SHA-256:");
                        ui.add(
                            TextEdit::singleline(&mut ed.cert)
                                .desired_width(320.0)
                                .font(egui::TextStyle::Monospace),
                        );
                        ui.end_row();
                    }

                    ui.label("Interface override:");
                    ui.add(
                        TextEdit::singleline(&mut ed.iface)
                            .desired_width(320.0)
                            .hint_text("empty = auto (ppp*/tun*)"),
                    );
                    ui.end_row();
                });

            if let Some(e) = &ed.error {
                ui.label(RichText::new(e).color(ERR_FG));
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.button(RichText::new("Save").strong()).clicked() {
                    let prof = ed.to_profile();
                    match prof.valid() {
                        Err(e) => ed.error = Some(e),
                        Ok(()) => {
                            app.apply_editor(prof);
                            done = true;
                        }
                    }
                }
                if ed.id.is_some() {
                    let label = if ed.confirm_delete {
                        "Confirm delete"
                    } else {
                        "Delete"
                    };
                    if ui.button(label).clicked() {
                        if ed.confirm_delete {
                            app.delete_editor();
                            done = true;
                        } else {
                            ed.confirm_delete = true;
                        }
                    }
                }
                if ui.button("Cancel").clicked() {
                    done = true;
                }
            });
        });

    if !open {
        done = true;
    }
    if done {
        app.editor = None;
    } else {
        app.editor = Some(ed);
    }
}
