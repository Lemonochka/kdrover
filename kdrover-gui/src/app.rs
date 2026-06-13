use drover_core::{

    install, load_install_settings, uninstall, InstallError, InstallSettings, ProxyMode,

};

use eframe::egui::{self, TextEdit, Vec2};

use std::path::PathBuf;

use std::sync::atomic::{AtomicBool, Ordering};



const GITHUB_URL: &str = "https://github.com/hdrover/discord-drover";

const WINDOW_SIZE: [f32; 2] = [520.0, 560.0];

const TEXT_COLOR: egui::Color32 = egui::Color32::from_rgb(28, 27, 31);

const MUTED_TEXT_COLOR: egui::Color32 = egui::Color32::from_rgb(73, 69, 79);

const ACCENT: egui::Color32 = egui::Color32::from_rgb(103, 80, 164);



static WINDOW_SHOWN: AtomicBool = AtomicBool::new(false);



pub struct InstallerApp {

    exe_dir: PathBuf,

    settings: InstallSettings,

    selected_mode: usize,

    status: Option<StatusMessage>,

    show_error_details: bool,

}



#[derive(Clone)]

struct StatusMessage {

    title: String,

    body: String,

    details: Vec<String>,

    is_error: bool,

}



impl InstallerApp {

    pub fn new(exe_dir: PathBuf) -> Self {

        let settings = load_install_settings(&exe_dir);

        let mut status = None;

        if !drover_core::resolve_dll_path(&exe_dir, None).is_file() {

            status = Some(StatusMessage {

                title: "Payload not found".into(),

                body: format!(

                    "Place {} next to kdrover.exe before installing.",

                    drover_core::BUILD_DLL_FILENAME

                ),

                details: Vec::new(),

                is_error: true,

            });

        }



        Self {

            selected_mode: proxy_mode_to_index(settings.mode),

            exe_dir,

            settings,

            status,

            show_error_details: false,

        }

    }



    fn sync_mode_from_selection(&mut self) {

        self.settings.mode = match self.selected_mode {

            0 => ProxyMode::Http,

            1 => ProxyMode::Socks5,

            _ => ProxyMode::Direct,

        };

    }



    fn run_install(&mut self) {

        self.sync_mode_from_selection();

        match install(&self.exe_dir, &self.settings, None) {

            Ok(dirs) => {

                let count = dirs.len();

                self.status = Some(StatusMessage {

                    title: "Installation complete!".into(),

                    body: format!("Installed into {count} Discord folder(s)."),

                    details: dirs

                        .into_iter()

                        .map(|dir| dir.display().to_string())

                        .collect(),

                    is_error: false,

                });

            }

            Err(InstallError::PartialFailure { message, details }) => {

                self.status = Some(StatusMessage {

                    title: "Installation error".into(),

                    body: message,

                    details,

                    is_error: true,

                });

            }

            Err(error) => {

                self.status = Some(StatusMessage {

                    title: "Installation error".into(),

                    body: error.to_string(),

                    details: Vec::new(),

                    is_error: true,

                });

            }

        }

    }



    fn run_uninstall(&mut self) {

        match uninstall() {

            Ok(dirs) => {

                self.status = Some(StatusMessage {

                    title: "Uninstallation complete".into(),

                    body: "All files have been successfully removed.".into(),

                    details: dirs

                        .into_iter()

                        .map(|dir| dir.display().to_string())

                        .collect(),

                    is_error: false,

                });

            }

            Err(InstallError::PartialFailure { message, details }) => {

                self.status = Some(StatusMessage {

                    title: "Uninstallation error".into(),

                    body: message,

                    details,

                    is_error: true,

                });

            }

            Err(error) => {

                self.status = Some(StatusMessage {

                    title: "Uninstallation error".into(),

                    body: error.to_string(),

                    details: Vec::new(),

                    is_error: true,

                });

            }

        }

    }



    fn draw_proxy_fields(&mut self, ui: &mut egui::Ui) {

        ui.label(label_text("Connection type"));

        ui.horizontal(|ui| {

            ui.radio_value(&mut self.selected_mode, 0, "HTTP");

            ui.radio_value(&mut self.selected_mode, 1, "SOCKS5");

            ui.radio_value(&mut self.selected_mode, 2, "Direct");

        });

        ui.add_space(8.0);



        self.sync_mode_from_selection();

        let proxy_enabled = self.settings.proxy_fields_enabled();



        ui.add_enabled_ui(proxy_enabled, |ui| {

            labeled_field(ui, "Host name", &mut self.settings.host, "127.0.0.1", false);

            labeled_field(ui, "Port number", &mut self.settings.port, "8080", false);

            ui.checkbox(&mut self.settings.auth, "Authentication");

        });



        let auth_enabled = self.settings.auth_fields_enabled();

        ui.add_enabled_ui(auth_enabled, |ui| {

            labeled_field(ui, "Login", &mut self.settings.login, "username", false);

            labeled_field(ui, "Password", &mut self.settings.password, "password", true);

        });



        if self.selected_mode == 2 {

            ui.add_space(4.0);

            ui.label(body_text(

                "Direct mode bypasses voice chat restrictions without using a proxy.",

            ));

        }

    }



    fn draw_status(&mut self, ui: &mut egui::Ui) {

        let Some(status) = self.status.clone() else {

            return;

        };



        ui.add_space(12.0);

        let color = if status.is_error {

            egui::Color32::from_rgb(179, 38, 30)

        } else {

            egui::Color32::from_rgb(46, 125, 50)

        };

        ui.colored_label(color, &status.title);

        ui.label(body_text(&status.body));

        if !status.details.is_empty() {

            ui.checkbox(&mut self.show_error_details, "Show details");

            if self.show_error_details {

                egui::ScrollArea::vertical()

                    .max_height(80.0)

                    .show(ui, |ui| {

                        for line in &status.details {

                            ui.monospace(line);

                        }

                    });

            }

        }

    }

}



impl eframe::App for InstallerApp {

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {

        if !WINDOW_SHOWN.swap(true, Ordering::Relaxed) {

            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));

            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);

        }



        egui::TopBottomPanel::top("header").show(ctx, |ui| {

            ui.add_space(8.0);

            ui.horizontal(|ui| {

                ui.heading(

                    egui::RichText::new("KDrover")

                        .size(22.0)

                        .strong()

                        .color(TEXT_COLOR),

                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {

                    if ui.link("GitHub").clicked() {

                        let _ = open::that(GITHUB_URL);

                    }

                });

            });

            ui.add_space(8.0);

            ui.separator();

        });



        egui::CentralPanel::default().show(ctx, |ui| {

            ui.add_space(8.0);



            egui::Frame::group(ui.style())

                .inner_margin(16.0)

                .fill(egui::Color32::WHITE)

                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(210, 210, 210)))

                .show(ui, |ui| {

                    ui.label(

                        egui::RichText::new("Proxy settings")

                            .size(18.0)

                            .strong()

                            .color(TEXT_COLOR),

                    );

                    ui.add_space(8.0);

                    self.draw_proxy_fields(ui);

                });



            self.draw_status(ui);



            ui.add_space(16.0);

            ui.horizontal(|ui| {

                if primary_button(ui, "Install", Vec2::new(220.0, 40.0)).clicked() {

                    self.run_install();

                }

                if secondary_button(ui, "Uninstall", Vec2::new(220.0, 40.0)).clicked() {

                    self.run_uninstall();

                }

            });

        });

    }

}



fn primary_button(ui: &mut egui::Ui, label: &str, size: Vec2) -> egui::Response {

    let button = egui::Button::new(

        egui::RichText::new(label)

            .size(16.0)

            .strong()

            .color(egui::Color32::WHITE),

    )

    .fill(ACCENT)

    .min_size(size);

    ui.add(button)

}



fn secondary_button(ui: &mut egui::Ui, label: &str, size: Vec2) -> egui::Response {

    let button = egui::Button::new(

        egui::RichText::new(label)

            .size(16.0)

            .strong()

            .color(ACCENT),

    )

    .stroke(egui::Stroke::new(1.5, ACCENT))

    .fill(egui::Color32::WHITE)

    .min_size(size);

    ui.add(button)

}



fn labeled_field(

    ui: &mut egui::Ui,

    label: &str,

    value: &mut String,

    hint: &str,

    password: bool,

) {

    ui.label(label_text(label));

    let mut edit = TextEdit::singleline(value)

        .hint_text(hint)

        .text_color(TEXT_COLOR);

    if password {

        edit = edit.password(true);

    }

    ui.add(edit);

    ui.add_space(4.0);

}



fn label_text(text: &str) -> egui::RichText {

    egui::RichText::new(text).color(TEXT_COLOR)

}



fn body_text(text: &str) -> egui::RichText {

    egui::RichText::new(text).color(MUTED_TEXT_COLOR)

}



fn apply_light_theme(ctx: &egui::Context) {

    let mut visuals = egui::Visuals::light();

    visuals.window_fill = egui::Color32::from_rgb(250, 250, 252);

    visuals.panel_fill = egui::Color32::from_rgb(250, 250, 252);

    visuals.extreme_bg_color = egui::Color32::WHITE;

    visuals.override_text_color = Some(TEXT_COLOR);

    visuals.weak_text_color = Some(MUTED_TEXT_COLOR);

    ctx.set_visuals(visuals);

}



fn proxy_mode_to_index(mode: ProxyMode) -> usize {

    match mode {

        ProxyMode::Http => 0,

        ProxyMode::Socks5 => 1,

        ProxyMode::Direct => 2,

    }

}



pub fn run() -> eframe::Result<()> {

    let exe_dir = std::env::current_exe()

        .ok()

        .and_then(|path| path.parent().map(PathBuf::from))

        .unwrap_or_else(|| PathBuf::from("."));



    let options = eframe::NativeOptions {

        viewport: egui::ViewportBuilder::default()

            .with_app_id("kdrover-installer-v2")

            .with_inner_size(WINDOW_SIZE)

            .with_min_inner_size(WINDOW_SIZE)

            .with_title("KDrover"),

        centered: true,

        persist_window: false,

        renderer: eframe::Renderer::Glow,

        ..Default::default()

    };



    eframe::run_native(

        "KDrover",

        options,

        Box::new(|cc| {

            apply_light_theme(&cc.egui_ctx);

            Ok(Box::new(InstallerApp::new(exe_dir)))

        }),

    )

}


