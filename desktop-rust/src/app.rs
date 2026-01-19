use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};

use anyhow::Result;
use eframe::egui::{self, Color32, RichText};
use eframe::egui::{FontData, FontDefinitions, FontFamily};
use image::ImageReader;

use crate::io::{default_sorted_path, load_result_items, save_sorted_items};
use crate::model::{AppState, ResultItem};
use photo_ai_common::layout::LAYOUT_FIELDS;

const DETAIL_FIELDS: &[(&str, &str)] = &[
    ("file_name", "File Name"),
    ("file_path", "File Path"),
    ("date", "Date"),
    ("photo_category", "Photo Category"),
    ("work_type", "Work Type"),
    ("variety", "Variety"),
    ("subphase", "Subphase"),
    ("remarks", "Remarks"),
    ("station", "Station"),
    ("description", "Description"),
    ("measurements", "Measurements"),
    ("detected_text", "Detected Text"),
    ("has_board", "Has Board"),
    ("reasoning", "Reasoning"),
];

pub struct DesktopApp {
    state: AppState,
    drag_index: Option<usize>,
    status: String,
    export_status: String,
    export_format: ExportFormat,
    analyze_status: String,
    analyze_batch_size: usize,
    analyze_provider: AnalyzeProvider,
    analyze_rx: Option<Receiver<UiMessage>>,
    export_rx: Option<Receiver<UiMessage>>,
    analyzing: bool,
    exporting: bool,
    thumbs: HashMap<String, egui::TextureHandle>,
    thumb_rx: Receiver<ThumbData>,
    thumb_tx: mpsc::Sender<ThumbData>,
    thumb_inflight: HashSet<String>,
    pending_thumbs: Vec<ThumbData>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExportFormat {
    Pdf,
    Excel,
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AnalyzeProvider {
    Claude,
    Codex,
}

enum UiMessage {
    AnalyzeDone { ok: bool, message: String, output: Option<PathBuf> },
    ExportDone { message: String },
}

struct ThumbData {
    path: String,
    size: [usize; 2],
    pixels: Vec<u8>,
}

impl Default for ExportFormat {
    fn default() -> Self {
        ExportFormat::Pdf
    }
}

impl Default for AnalyzeProvider {
    fn default() -> Self {
        AnalyzeProvider::Claude
    }
}

impl DesktopApp {
    fn open_json(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("JSON", &["json"])
            .pick_file()
        {
            match self.load_from_path(&path) {
                Ok(_) => self.status = format!("Loaded {}", path.display()),
                Err(err) => self.status = format!("Load failed: {err}"),
            }
        }
    }

    fn reload_json(&mut self) {
        let Some(path) = self.state.source_path.clone() else {
            self.status = "No source file loaded".to_string();
            return;
        };
        match self.load_from_path(&path) {
            Ok(_) => self.status = format!("Reloaded {}", path.display()),
            Err(err) => self.status = format!("Reload failed: {err}"),
        }
    }

    fn load_from_path(&mut self, path: &Path) -> Result<()> {
        let items = load_result_items(path)?;
        self.state.items = items.clone();
        self.state.original_items = items;
        self.state.selected_index = if self.state.items.is_empty() { None } else { Some(0) };
        self.state.source_path = Some(path.to_path_buf());
        self.state.dirty = false;
        self.thumbs.clear();
        self.thumb_inflight.clear();
        self.pending_thumbs.clear();
        Ok(())
    }

    fn save_sorted(&mut self) {
        let Some(source) = &self.state.source_path else {
            self.status = "No source file loaded".to_string();
            return;
        };
        let default_path = default_sorted_path(source);
        if let Some(path) = rfd::FileDialog::new()
            .set_file_name(
                default_path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("result.sorted.json"),
            )
            .save_file()
        {
            match save_sorted_items(&path, &self.state.items) {
                Ok(_) => {
                    self.status = format!("Saved {}", path.display());
                    self.state.dirty = false;
                }
                Err(err) => self.status = format!("Save failed: {err}"),
            }
        }
    }

    fn reset_order(&mut self) {
        self.state.items = self.state.original_items.clone();
        self.state.dirty = false;
    }

    fn move_item(&mut self, from: usize, to: usize) {
        if from >= self.state.items.len() || to >= self.state.items.len() {
            return;
        }
        let item = self.state.items.remove(from);
        self.state.items.insert(to, item);
        self.state.dirty = true;
        self.state.selected_index = Some(to);
    }

    fn request_thumbnail(&mut self, path: &str) {
        if path.is_empty() || self.thumbs.contains_key(path) || self.thumb_inflight.contains(path) {
            return;
        }
        self.thumb_inflight.insert(path.to_string());
        let sender = self.thumb_tx.clone();
        let path_owned = path.to_string();

        std::thread::spawn(move || {
            let image = ImageReader::open(&path_owned).ok().and_then(|r| r.decode().ok());
            if let Some(image) = image {
                let thumb = image.thumbnail(220, 160);
                let size = [thumb.width() as usize, thumb.height() as usize];
                let pixels = thumb.to_rgba8().into_raw();
                let _ = sender.send(ThumbData {
                    path: path_owned,
                    size,
                    pixels,
                });
            } else {
                let _ = sender.send(ThumbData {
                    path: path_owned,
                    size: [0, 0],
                    pixels: Vec::new(),
                });
            }
        });
    }

    fn process_pending_thumbs(&mut self, ctx: &egui::Context) {
        let pending = std::mem::take(&mut self.pending_thumbs);
        for msg in pending {
            if msg.size[0] == 0 || msg.size[1] == 0 {
                continue;
            }
            let color_image = egui::ColorImage::from_rgba_unmultiplied(msg.size, &msg.pixels);
            let texture = ctx.load_texture(&msg.path, color_image, egui::TextureOptions::default());
            self.thumbs.insert(msg.path, texture);
        }
    }

    fn render_card(
        &mut self,
        ui: &mut egui::Ui,
        _ctx: &egui::Context,
        index: usize,
        item: &ResultItem,
    ) {
        let is_selected = self.state.selected_index == Some(index);
        let frame = egui::Frame::none()
            .fill(if is_selected { Color32::from_rgb(31, 35, 48) } else { Color32::from_rgb(24, 28, 40) })
            .stroke(egui::Stroke::new(1.0, if is_selected { Color32::from_rgb(246, 196, 69) } else { Color32::from_gray(40) }))
            .rounding(egui::Rounding::same(10.0))
            .inner_margin(egui::Margin::same(10.0));

        let inner = frame.show(ui, |ui| {
            let card_width = ui.available_width();
            ui.set_min_width(card_width);
            ui.horizontal(|ui| {
                let thumb_size = egui::vec2(160.0, 120.0);
                if !item.file_path.is_empty() {
                    if let Some(texture) = self.thumbs.get(&item.file_path) {
                        ui.add(egui::Image::new(texture).fit_to_exact_size(thumb_size));
                    } else {
                        self.request_thumbnail(&item.file_path);
                        ui.allocate_ui_with_layout(thumb_size, egui::Layout::centered_and_justified(egui::Direction::LeftToRight), |ui| {
                            ui.label("Loading...");
                        });
                    }
                } else {
                    ui.allocate_ui_with_layout(thumb_size, egui::Layout::centered_and_justified(egui::Direction::LeftToRight), |ui| {
                        ui.label("No image");
                    });
                }

                ui.add_space(8.0);
                ui.vertical(|ui| {
                    ui.set_min_width(ui.available_width());
                    let grid = egui::Grid::new(format!("card_grid_{}", index))
                        .striped(true)
                        .min_col_width(60.0);
                    grid.show(ui, |ui| {
                        for field in LAYOUT_FIELDS {
                            ui.label(RichText::new(field.label).color(Color32::from_gray(200)).size(12.0));
                            let value = value_by_key(item, field.key);
                            ui.label(RichText::new(if value.is_empty() { "-" } else { value }).size(12.0));
                            ui.end_row();
                        }
                    });
                });
            });
        });

        let response = inner.response.interact(egui::Sense::click_and_drag());
        if response.clicked() {
            self.state.selected_index = Some(index);
        }

        response.context_menu(|ui| {
            if ui.button("Analyze Folder (from file path)").clicked() {
                if !item.file_path.is_empty() {
                    let folder = PathBuf::from(&item.file_path)
                        .parent()
                        .map(|p| p.to_path_buf());
                    if let Some(folder) = folder {
                        self.run_analyze_for_folder(folder);
                    }
                }
                ui.close_menu();
            }
        });

        if response.drag_started() {
            self.drag_index = Some(index);
        }

        if response.drag_stopped() {
            if let Some(from) = self.drag_index.take() {
                let to = index;
                if from != to {
                    self.move_item(from, to);
                }
            }
        }
    }

    fn render_details(&self, ui: &mut egui::Ui) {
        let Some(index) = self.state.selected_index else {
            ui.label("Select a card to see details.");
            return;
        };
        let Some(item) = self.state.items.get(index) else {
            ui.label("Select a card to see details.");
            return;
        };

        ui.group(|ui| {
            ui.label(RichText::new("File Name").strong());
            let text = if item.file_name.is_empty() { "-" } else { item.file_name.as_str() };
            let response = ui.selectable_label(false, text);
            response.context_menu(|ui| {
                if ui.button("Copy Path").clicked() {
                    ui.output_mut(|o| o.copied_text = item.file_path.clone());
                    ui.close_menu();
                }
            });
        });

        for (field, label) in DETAIL_FIELDS {
            if *field == "file_name" || *field == "file_path" {
                continue;
            }
            ui.group(|ui| {
                ui.label(RichText::new(*label).strong());
                let value = match *field {
                    "file_name" => &item.file_name,
                    "file_path" => &item.file_path,
                    "date" => &item.date,
                    "photo_category" => &item.photo_category,
                    "work_type" => &item.work_type,
                    "variety" => &item.variety,
                    "subphase" => &item.subphase,
                    "remarks" => &item.remarks,
                    "station" => &item.station,
                    "description" => &item.description,
                    "measurements" => &item.measurements,
                    "detected_text" => &item.detected_text,
                    "reasoning" => &item.reasoning,
                    _ => "",
                };
                if *field == "has_board" {
                    let text = if item.has_board { "true" } else { "false" };
                    ui.label(text);
                } else {
                    ui.label(if value.is_empty() { "-" } else { value });
                }
            });
        }
    }

    fn run_export(&mut self, format: ExportFormat) {
        let Some(source) = &self.state.source_path else {
            self.export_status = "No source file loaded".to_string();
            return;
        };
        let output_path = default_sorted_path(source);
        if let Err(err) = save_sorted_items(&output_path, &self.state.items) {
            self.export_status = format!("Save failed: {err}");
            return;
        }

        let format_arg = match format {
            ExportFormat::Pdf => "pdf",
            ExportFormat::Excel => "excel",
            ExportFormat::Both => "both",
        };

        let cli = resolve_cli_binary();
        let (tx, rx) = mpsc::channel();
        self.export_rx = Some(rx);
        self.exporting = true;
        self.export_status = "Export running...".to_string();

        std::thread::spawn(move || {
            let result = std::process::Command::new(cli)
                .args(["export", output_path.to_string_lossy().as_ref(), "--format", format_arg])
                .output();

            let message = match result {
                Ok(out) if out.status.success() => UiMessage::ExportDone {
                    message: "Export complete".to_string(),
                },
                Ok(out) => {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    UiMessage::ExportDone {
                        message: format!("Export failed: {}", stderr.trim()),
                    }
                }
                Err(err) => UiMessage::ExportDone {
                    message: format!("Export failed: {err}"),
                },
            };
            let _ = tx.send(message);
        });
    }

    fn run_analyze(&mut self) {
        let Some(folder) = rfd::FileDialog::new().pick_folder() else {
            return;
        };
        self.run_analyze_for_folder(folder);
    }

    fn run_analyze_for_folder(&mut self, folder: PathBuf) {
        let default_output = folder.join("result.json");
        let Some(output) = rfd::FileDialog::new()
            .set_file_name(
                default_output
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("result.json"),
            )
            .save_file()
        else {
            return;
        };

        let provider = match self.analyze_provider {
            AnalyzeProvider::Claude => "claude",
            AnalyzeProvider::Codex => "codex",
        };

        let cli = resolve_cli_binary();
        let batch_arg = self.analyze_batch_size.to_string();
        let (tx, rx) = mpsc::channel();
        self.analyze_rx = Some(rx);
        self.analyzing = true;
        self.analyze_status = "Analyze running...".to_string();

        std::thread::spawn(move || {
            let result = std::process::Command::new(cli)
                .args([
                    "analyze",
                    folder.to_string_lossy().as_ref(),
                    "--output",
                    output.to_string_lossy().as_ref(),
                    "--batch-size",
                    batch_arg.as_str(),
                    "--ai-provider",
                    provider,
                ])
                .output();

            let message = match result {
                Ok(out) if out.status.success() => UiMessage::AnalyzeDone {
                    ok: true,
                    message: "Analyze complete".to_string(),
                    output: Some(output),
                },
                Ok(out) => {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    UiMessage::AnalyzeDone {
                        ok: false,
                        message: format!("Analyze failed: {}", stderr.trim()),
                        output: None,
                    }
                }
                Err(err) => UiMessage::AnalyzeDone {
                    ok: false,
                    message: format!("Analyze failed: {err}"),
                    output: None,
                },
            };
            let _ = tx.send(message);
        });
    }

    fn poll_messages(&mut self) {
        while let Ok(msg) = self.thumb_rx.try_recv() {
            self.thumb_inflight.remove(&msg.path);
            self.pending_thumbs.push(msg);
        }

        if let Some(rx) = &self.analyze_rx {
            if let Ok(msg) = rx.try_recv() {
                if let UiMessage::AnalyzeDone { ok, message, output } = msg {
                    self.analyze_status = message;
                    self.analyzing = false;
                    self.analyze_rx = None;
                    if ok {
                        if let Some(path) = output {
                            if let Err(err) = self.load_from_path(&path) {
                                self.analyze_status = format!("Load result failed: {err}");
                            }
                        }
                    }
                }
            }
        }

        if let Some(rx) = &self.export_rx {
            if let Ok(msg) = rx.try_recv() {
                if let UiMessage::ExportDone { message } = msg {
                    self.export_status = message;
                    self.exporting = false;
                    self.export_rx = None;
                }
            }
        }
    }
}

pub fn configure_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let candidates = [
        r"C:\Windows\Fonts\meiryo.ttc",
        r"C:\Windows\Fonts\msgothic.ttc",
        "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
        "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    ];

    for path in candidates {
        if let Ok(data) = std::fs::read(path) {
            fonts.font_data.insert("jp_fallback".to_string(), FontData::from_owned(data));
            fonts.families
                .entry(FontFamily::Proportional)
                .or_default()
                .insert(0, "jp_fallback".to_string());
            fonts.families
                .entry(FontFamily::Monospace)
                .or_default()
                .insert(0, "jp_fallback".to_string());
            ctx.set_fonts(fonts);
            return;
        }
    }
}

impl Default for DesktopApp {
    fn default() -> Self {
        let (thumb_tx, thumb_rx) = mpsc::channel();
        Self {
            state: AppState::default(),
            drag_index: None,
            status: String::new(),
            export_status: String::new(),
            export_format: ExportFormat::default(),
            analyze_status: String::new(),
            analyze_batch_size: 5,
            analyze_provider: AnalyzeProvider::default(),
            analyze_rx: None,
            export_rx: None,
            analyzing: false,
            exporting: false,
            thumbs: HashMap::new(),
            thumb_rx,
            thumb_tx,
            thumb_inflight: HashSet::new(),
            pending_thumbs: Vec::new(),
        }
    }
}

impl eframe::App for DesktopApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.analyzing || self.exporting || !self.thumb_inflight.is_empty() || !self.pending_thumbs.is_empty() {
            ctx.request_repaint();
        }
        self.poll_messages();
        self.process_pending_thumbs(ctx);

        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open JSON").clicked() {
                        self.open_json();
                        ui.close_menu();
                    }
                    if ui.button("Reload JSON").clicked() {
                        self.reload_json();
                        ui.close_menu();
                    }
                    let save_enabled = !self.state.items.is_empty();
                    if ui.add_enabled(save_enabled, egui::Button::new("Save Sorted")).clicked() {
                        self.save_sorted();
                        ui.close_menu();
                    }
                    if ui.add_enabled(save_enabled, egui::Button::new("Reset Order")).clicked() {
                        self.reset_order();
                        ui.close_menu();
                    }
                });

                ui.menu_button("Analyze", |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Batch");
                        ui.add(egui::DragValue::new(&mut self.analyze_batch_size).clamp_range(1..=50));
                    });
                    ui.radio_value(&mut self.analyze_provider, AnalyzeProvider::Claude, "Claude");
                    ui.radio_value(&mut self.analyze_provider, AnalyzeProvider::Codex, "Codex");
                    if ui.add_enabled(!self.analyzing, egui::Button::new("Run Analyze")).clicked() {
                        self.run_analyze();
                        ui.close_menu();
                    }
                });

                ui.menu_button("Export", |ui| {
                    ui.radio_value(&mut self.export_format, ExportFormat::Pdf, "PDF");
                    ui.radio_value(&mut self.export_format, ExportFormat::Excel, "Excel");
                    ui.radio_value(&mut self.export_format, ExportFormat::Both, "Both");
                    let save_enabled = !self.state.items.is_empty();
                    if ui.add_enabled(save_enabled && !self.exporting, egui::Button::new("Run Export")).clicked() {
                        self.run_export(self.export_format);
                        ui.close_menu();
                    }
                });

                ui.separator();
                if !self.analyze_status.is_empty() {
                    ui.label(RichText::new(&self.analyze_status).color(Color32::from_gray(170)));
                }
                if !self.export_status.is_empty() {
                    ui.label(RichText::new(&self.export_status).color(Color32::from_rgb(246, 196, 69)));
                }
                if !self.status.is_empty() {
                    ui.label(RichText::new(&self.status).color(Color32::from_gray(170)));
                }
            });
        });

        egui::SidePanel::left("list").resizable(true).show(ctx, |ui| {
            ui.heading("Cards");
            ui.label(format!("{} items", self.state.items.len()));
            ui.separator();
            let row_height = 160.0;
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show_rows(ui, row_height, self.state.items.len(), |ui, range| {
                for index in range {
                    if index < self.state.items.len() {
                        let item = self.state.items[index].clone();
                        self.render_card(ui, ctx, index, &item);
                        ui.add_space(8.0);
                    }
                }
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Details");
            ui.separator();
            egui::ScrollArea::vertical().show(ui, |ui| {
                self.render_details(ui);
            });
        });
    }
}

fn resolve_cli_binary() -> PathBuf {
    let exe = std::env::current_exe().ok();
    if let Some(base_dir) = exe.as_ref().and_then(|p| p.parent()) {
        let local = base_dir.join("photo-ai-rust.exe");
        if local.exists() {
            return local;
        }
        if let Some(target_dir) = base_dir.parent() {
            let sibling = target_dir.join("debug").join("photo-ai-rust.exe");
            if sibling.exists() {
                return sibling;
            }
            let release = target_dir.join("release").join("photo-ai-rust.exe");
            if release.exists() {
                return release;
            }
        }
    }
    PathBuf::from("photo-ai-rust")
}

fn value_by_key<'a>(item: &'a ResultItem, key: &str) -> &'a str {
    match key {
        "date" => item.date.as_str(),
        "photoCategory" => item.photo_category.as_str(),
        "workType" => item.work_type.as_str(),
        "variety" => item.variety.as_str(),
        "subphase" => item.subphase.as_str(),
        "station" => item.station.as_str(),
        "remarks" => item.remarks.as_str(),
        "measurements" => item.measurements.as_str(),
        _ => "",
    }
}
