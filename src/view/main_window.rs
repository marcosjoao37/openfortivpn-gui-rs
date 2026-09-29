use super::OFGApp;
use crate::model::distro::{sudoers_rule, SUDOERS_EDIT_CMD};
use crate::model::state::{fmt_kb, fmt_mb, ConnPhase};
use crate::util::username;
use egui::{Color32, Margin, RichText, TextEdit};

const RED_BG: Color32 = Color32::from_rgb(0xB3, 0x26, 0x1E);
const AMBER_BG: Color32 = Color32::from_rgb(0x8A, 0x62, 0x00);
const WHITE: Color32 = Color32::WHITE;
const ERR_FG: Color32 = Color32::from_rgb(0xFF, 0x6B, 0x6B);

pub fn draw(app: &mut OFGApp, ctx: &egui::Context) {
    footer_panel(app, ctx);
    egui::CentralPanel::default()
        .frame(egui::Frame::default().inner_margin(Margin::same(16.0)))
        .show(ctx, |ui| {
            banner(ui, app);
            ui.add_space(10.0);
            profile_bar(ui, app);
            ui.add_space(6.0);
            ui.separator();
            ui.add_space(8.0);
            fields(ui, app);
            ui.add_space(12.0);
            action_row(ui, app);
            ui.add_space(10.0);
            stats_row(ui, app);
            ui.add_space(8.0);
            log_section(ui, app);
        });
    sudo_modal(app, ctx);
    quit_confirm_modal(app, ctx);
    super::profile_editor::show(app, ctx);
    super::about::show(app, ctx);
}

fn banner(ui: &mut egui::Ui, app: &mut OFGApp) {
    if !app.state.installed {
        egui::Frame::default()
            .fill(RED_BG)
            .rounding(6.0)
            .inner_margin(Margin::same(12.0))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new("⚠").size(22.0).color(WHITE));
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new("openfortivpn is not installed")
                                .strong()
                                .color(WHITE),
                        );
                        ui.label(
                            RichText::new(format!("Detected distro: {}", app.distro_pretty))
                                .color(WHITE),
                        );
                        copy_row(ui, "Install command", app.distro_cmd.clone());
                        ui.add_space(4.0);
                        sudoers_tip(ui, app, WHITE);
                    });
                });
            });
    } else if !app.state.passwordless {
        egui::Frame::default()
            .fill(AMBER_BG)
            .rounding(6.0)
            .inner_margin(Margin::same(10.0))
            .show(ui, |ui| {
                ui.label(
                    RichText::new(
                        "openfortivpn is not passwordless in sudoers — the sudo password will be \
                         asked before connecting (kept in memory for this session only).",
                    )
                    .color(WHITE),
                );
                sudoers_tip(ui, app, WHITE);
            });
    }

    if let Some(sha) = app.state.unknown_cert.clone() {
        ui.add_space(8.0);
        egui::Frame::default()
            .fill(AMBER_BG)
            .rounding(6.0)
            .inner_margin(Margin::same(10.0))
            .show(ui, |ui| {
                ui.label(
                    RichText::new("The server presented an unknown certificate.")
                        .strong()
                        .color(WHITE),
                );
                ui.label(RichText::new(format!("SHA-256: {sha}")).color(WHITE));
                ui.horizontal(|ui| {
                    if ui.button("Trust this certificate").clicked() {
                        if let Some(mut prof) = app.book.selected_profile().cloned() {
                            prof.trust_cert = true;
                            prof.trusted_cert = sha.to_lowercase();
                            app.book.upsert(prof);
                            app.book.save(&app.paths.book_path).ok();
                            app.state.unknown_cert = None;
                            app.state.push_log(
                                "Certificate trusted and saved to the profile. Reconnect to apply."
                                    .into(),
                            );
                        }
                    }
                    if ui.button("Dismiss").clicked() {
                        app.state.unknown_cert = None;
                    }
                });
            });
    }
}

fn sudoers_tip(ui: &mut egui::Ui, app: &OFGApp, color: Color32) {
    let bin = app
        .state
        .binary_path
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "/usr/bin/openfortivpn".into());
    let rule = sudoers_rule(&username(), &bin);
    ui.label(RichText::new("To skip the sudo password, add a sudoers rule:").color(color));
    copy_row(
        ui,
        "Step 1 — open sudoers with nano:",
        SUDOERS_EDIT_CMD.to_owned(),
    );
    copy_row(ui, "Step 2 — add this line:", rule);
}

fn copy_row(ui: &mut egui::Ui, label: &str, text: String) {
    ui.horizontal(|ui| {
        ui.label(label);
        let mut s = text.clone();
        ui.add_enabled(false, TextEdit::singleline(&mut s).desired_width(330.0));
        if ui.button("Copy").clicked() {
            ui.ctx().copy_text(text.clone());
        }
    });
}

fn profile_bar(ui: &mut egui::Ui, app: &mut OFGApp) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("Profile:").strong());
        let enabled = !app.state.phase.busy();
        let current = app
            .book
            .selected_profile()
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "—".into());
        if enabled {
            let mut sel = app.book.selected.clone();
            egui::ComboBox::from_id_salt("profile-select")
                .selected_text(current.clone())
                .width(220.0)
                .show_ui(ui, |ui| {
                    for p in &app.book.profiles {
                        ui.selectable_value(&mut sel, Some(p.id.clone()), p.name.clone());
                    }
                });
            if sel != app.book.selected {
                app.book.selected = sel;
                app.book.save(&app.paths.book_path).ok();
            }
        } else {
            let mut cur = current;
            ui.add_enabled(false, TextEdit::singleline(&mut cur).desired_width(220.0));
        }
        if ui
            .add_enabled(enabled, egui::Button::new("+"))
            .on_hover_text("New profile")
            .clicked()
        {
            let n = app.book.profiles.len() + 1;
            app.editor = Some(super::profile_editor::EditorState::new(
                None,
                format!("Profile {n}"),
            ));
        }
        if ui
            .add_enabled(
                enabled && app.book.selected.is_some(),
                egui::Button::new("Edit"),
            )
            .on_hover_text("Edit selected profile")
            .clicked()
        {
            if let Some(p) = app.book.selected_profile().cloned() {
                let name = p.name.clone();
                app.editor = Some(super::profile_editor::EditorState::new(Some(p), name));
            }
        }
        if ui
            .button("↻")
            .on_hover_text("Re-check installation and sudoers")
            .clicked()
        {
            app.reprobe();
        }
    });
}

fn fields(ui: &mut egui::Ui, app: &mut OFGApp) {
    let prof = app.book.selected_profile().cloned();
    let can_edit_password = app.state.installed
        && matches!(app.state.phase, ConnPhase::Idle | ConnPhase::Failed { .. });
    egui::Grid::new("fields-grid")
        .num_columns(2)
        .spacing([14.0, 10.0])
        .show(ui, |ui| {
            ui.label("Server:");
            let mut s = prof.as_ref().map(|p| p.server.clone()).unwrap_or_default();
            ui.add_enabled(false, TextEdit::singleline(&mut s).desired_width(340.0));
            ui.end_row();

            ui.label("User:");
            let mut u = prof
                .as_ref()
                .map(|p| p.username.clone())
                .unwrap_or_default();
            ui.add_enabled(false, TextEdit::singleline(&mut u).desired_width(340.0));
            ui.end_row();

            ui.label("VPN password:");
            ui.add_enabled(
                can_edit_password,
                TextEdit::singleline(&mut app.vpn_pass)
                    .password(true)
                    .desired_width(340.0),
            );
            ui.end_row();

            ui.label("Trust certificate:");
            let mut t = prof.as_ref().map(|p| p.trust_cert).unwrap_or(false);
            ui.add_enabled(false, egui::Checkbox::without_text(&mut t));
            ui.end_row();

            if prof.as_ref().map(|p| p.trust_cert).unwrap_or(false) {
                ui.label("Certificate SHA-256:");
                let mut c = prof
                    .as_ref()
                    .map(|p| p.trusted_cert.clone())
                    .unwrap_or_default();
                ui.add_enabled(
                    false,
                    TextEdit::singleline(&mut c)
                        .desired_width(340.0)
                        .font(egui::TextStyle::Monospace),
                );
                ui.end_row();
            }
        });
}

fn action_row(ui: &mut egui::Ui, app: &mut OFGApp) {
    ui.horizontal(|ui| {
        let can_connect = app.state.installed
            && matches!(app.state.phase, ConnPhase::Idle | ConnPhase::Failed { .. })
            && !app.vpn_pass.is_empty()
            && app.book.selected_profile().is_some();
        let can_disconnect = app.state.phase.busy();
        if ui
            .add_enabled(
                can_connect,
                egui::Button::new(RichText::new("Connect").strong()),
            )
            .clicked()
        {
            app.on_connect();
        }
        if ui
            .add_enabled(can_disconnect, egui::Button::new("Disconnect"))
            .clicked()
        {
            app.on_disconnect();
        }
        let (dot, txt, color) = phase_indicator(&app.state.phase);
        ui.label(RichText::new(dot).size(14.0).color(color));
        ui.label(txt);
    });
    if let ConnPhase::Failed { reason } = &app.state.phase {
        ui.label(RichText::new(reason).color(ERR_FG));
    }
}

fn phase_indicator(phase: &ConnPhase) -> (&'static str, String, Color32) {
    match phase {
        ConnPhase::Idle => ("●", "Idle".into(), Color32::GRAY),
        ConnPhase::Connecting => (
            "●",
            "Connecting…".into(),
            Color32::from_rgb(0xF5, 0x9E, 0x0B),
        ),
        ConnPhase::Connected { iface } => (
            "●",
            format!("Connected ({iface})"),
            Color32::from_rgb(0x22, 0xC5, 0x5E),
        ),
        ConnPhase::External { pid, iface } => (
            "●",
            format!("Connected ({iface}, external pid {pid})"),
            Color32::from_rgb(0x22, 0xC5, 0x5E),
        ),
        ConnPhase::Failed { .. } => ("●", "Failed".into(), Color32::from_rgb(0xDC, 0x26, 0x26)),
    }
}

fn stats_row(ui: &mut egui::Ui, app: &OFGApp) {
    let (connected, stats) = match &app.state.phase {
        ConnPhase::Connected { .. } | ConnPhase::External { .. } => (true, app.state.stats),
        _ => (false, None),
    };
    if connected {
        match stats {
            Some(s) => {
                let iface = app.state.stats_iface.clone().unwrap_or_default();
                ui.label(
                    RichText::new(format!(
                        "↓ {} MB   ↑ {} MB   ↓ {} KB/s   ↑ {} KB/s   ({iface})",
                        fmt_mb(s.rx_bytes),
                        fmt_mb(s.tx_bytes),
                        fmt_kb(s.rx_kbps),
                        fmt_kb(s.tx_kbps)
                    ))
                    .strong(),
                );
            }
            None => {
                ui.label("Connected — collecting statistics…");
            }
        }
    } else if matches!(app.state.phase, ConnPhase::Connecting) {
        ui.label("Waiting for the VPN interface…");
    } else {
        ui.label("Not connected — statistics unavailable.");
    }
}

fn log_section(ui: &mut egui::Ui, app: &mut OFGApp) {
    egui::CollapsingHeader::new("Connection log")
        .default_open(true)
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .max_height(160.0)
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    for line in &app.state.log {
                        ui.monospace(line);
                    }
                });
        });
}

fn footer_panel(app: &mut OFGApp, ctx: &egui::Context) {
    egui::TopBottomPanel::bottom("footer")
        .frame(egui::Frame::default().inner_margin(Margin::symmetric(16.0, 8.0)))
        .show(ctx, |ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("About").clicked() {
                    app.about_open = !app.about_open;
                }
                let status = match &app.state.phase {
                    ConnPhase::Connected { iface } | ConnPhase::External { iface, .. } => {
                        format!("connected ({iface})")
                    }
                    ConnPhase::Connecting => "connecting…".to_owned(),
                    ConnPhase::Failed { .. } => "failed".to_owned(),
                    ConnPhase::Idle => "idle".to_owned(),
                };
                ui.label(RichText::new(status).weak());
            });
        });
}

fn sudo_modal(app: &mut OFGApp, ctx: &egui::Context) {
    if app.pending_connect.is_none() {
        return;
    }
    let mut open = true;
    egui::Window::new("sudo password required")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.label("openfortivpn is not passwordless in sudoers.");
            ui.label(
                "Enter your sudo password to run the VPN (kept in memory for this session only, \
                 cleared on exit).",
            );
            let resp = ui.add(
                TextEdit::singleline(&mut app.sudo_buf)
                    .password(true)
                    .desired_width(280.0),
            );
            let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if enter || ui.button("Connect").clicked() {
                app.confirm_sudo();
            }
            if ui.button("Cancel").clicked() {
                app.cancel_sudo();
            }
        });
    if !open {
        app.cancel_sudo();
    }
}

fn quit_confirm_modal(app: &mut OFGApp, ctx: &egui::Context) {
    if !app.quit_confirm {
        return;
    }
    let mut open = true;
    egui::Window::new("VPN active")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.label("A VPN session is active. Disconnect and exit?");
            ui.horizontal(|ui| {
                if ui.button("Disconnect and exit").clicked() {
                    app.quit_confirm = false;
                    app.do_quit();
                }
                if ui.button("Cancel").clicked() {
                    app.quit_confirm = false;
                }
            });
        });
    if !open {
        app.quit_confirm = false;
    }
}
