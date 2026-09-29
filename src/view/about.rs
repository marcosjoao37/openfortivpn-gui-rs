use super::OFGApp;

pub fn show(app: &mut OFGApp, ctx: &egui::Context) {
    if !app.about_open {
        return;
    }
    let mut open = true;
    egui::Window::new("About")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.heading("openfortivpn GUI");
            ui.label(concat!("version ", env!("CARGO_PKG_VERSION")));
            ui.add_space(6.0);
            ui.label("A simple desktop interface for the openfortivpn CLI.");
            ui.hyperlink_to(
                "openfortivpn",
                "https://github.com/adrienverge/openfortivpn",
            );
            ui.add_space(6.0);
            ui.label("Built with Rust + egui + tray-icon.");
            ui.label("Developed with AI assistance.");
            ui.add_space(6.0);
            ui.label(format!(
                "Profiles: {}",
                app.shared.paths.book_path.display()
            ));
            ui.label("Passwords (VPN and sudo) are never saved to disk.");
            ui.add_space(8.0);
            if ui.button("Close").clicked() {
                app.about_open = false;
            }
        });
    if !open {
        app.about_open = false;
    }
}
