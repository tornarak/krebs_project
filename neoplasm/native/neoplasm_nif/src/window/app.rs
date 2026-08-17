use std::sync::{Arc, Mutex};

use super::events::{send_event, GuiEvent};
use super::hex_dump_tab::HexDumpTab;
use super::scanner_tab::ScannerTab;
use super::schema_builder_tab::{sanitize_field_name, FieldBuildState, SchemaBuilderTab, WatchEntry};
use super::{with_gui_state, GuiState};

#[derive(Default, PartialEq, Clone, Copy)]
pub enum ActiveTab {
    #[default]
    Scanner,
    HexDump,
    SchemaBuilder,
}

pub struct NeoplasmApp {
    pub scanner_tab: ScannerTab,
    pub hex_dump_tab: HexDumpTab,
    pub schema_builder_tab: SchemaBuilderTab,
    pub active_tab: ActiveTab,
    pub gui_state: Arc<Mutex<GuiState>>,
    // app-level state snapshotted each frame
    pub processes: Vec<(u32, String)>,
    pub selected_process: Option<usize>,
    pub attached: Option<(u64, String)>,
    pub error: Option<String>,
}

impl NeoplasmApp {
    pub fn new(cc: &eframe::CreationContext, gui_state: Arc<Mutex<GuiState>>) -> Self {
        let mut style = (*cc.egui_ctx.style()).clone();
        style.text_styles.insert(
            egui::TextStyle::Body,
            egui::FontId::new(16.0, egui::FontFamily::Proportional),
        );
        style.text_styles.insert(
            egui::TextStyle::Button,
            egui::FontId::new(16.0, egui::FontFamily::Proportional),
        );
        style.text_styles.insert(
            egui::TextStyle::Monospace,
            egui::FontId::new(15.0, egui::FontFamily::Monospace),
        );
        style.text_styles.insert(
            egui::TextStyle::Small,
            egui::FontId::new(13.0, egui::FontFamily::Proportional),
        );
        cc.egui_ctx.set_style(style);
        Self {
            scanner_tab: ScannerTab::default(),
            hex_dump_tab: HexDumpTab::default(),
            schema_builder_tab: SchemaBuilderTab::default(),
            active_tab: ActiveTab::default(),
            gui_state,
            processes: vec![],
            selected_process: None,
            attached: None,
            error: None,
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let selected_label = self
                .selected_process
                .and_then(|i| self.processes.get(i))
                .map(|(pid, name)| format!("{} ({})", name, pid))
                .unwrap_or_else(|| "— no process —".into());

            egui::ComboBox::from_id_salt("process_select")
                .selected_text(egui::RichText::new(&selected_label).size(15.0))
                .width(280.0)
                .show_ui(ui, |ui| {
                    for (i, (pid, name)) in self.processes.iter().enumerate() {
                        let label = format!("{} ({})", name, pid);
                        ui.selectable_value(&mut self.selected_process, Some(i), label);
                    }
                });

            if ui
                .button(egui::RichText::new("Refresh").size(15.0))
                .clicked()
            {
                send_event(GuiEvent::ListProcessesRequested);
            }

            ui.separator();
            ui.selectable_value(&mut self.active_tab, ActiveTab::Scanner, "Scanner");
            ui.selectable_value(&mut self.active_tab, ActiveTab::HexDump, "Hex Dump");
            ui.selectable_value(&mut self.active_tab, ActiveTab::SchemaBuilder, "Schema Builder");
            ui.separator();

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(egui::RichText::new("IEx").size(15.0)).clicked() {
                    send_event(GuiEvent::IexRequested);
                }
                ui.separator();
                if self.attached.is_some() {
                    if ui
                        .button(egui::RichText::new("Detach").size(15.0))
                        .clicked()
                    {
                        send_event(GuiEvent::DetachRequested);
                    }
                } else {
                    let can_attach = self.selected_process.is_some();
                    if ui
                        .add_enabled(
                            can_attach,
                            egui::Button::new(egui::RichText::new("Attach").size(15.0)),
                        )
                        .clicked()
                    {
                        if let Some(i) = self.selected_process {
                            let pid = self.processes[i].0;
                            send_event(GuiEvent::AttachRequested(pid));
                        }
                    }
                }
                ui.separator();
                let attach_label = match &self.attached {
                    Some((pid, name)) => format!("● {} ({})", name, pid),
                    None => "not attached".into(),
                };
                ui.centered_and_justified(|ui| {
                    ui.label(egui::RichText::new(attach_label).size(15.0));
                });
            });
        });
    }
}

impl eframe::App for NeoplasmApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Snapshot shared state — hold lock as briefly as possible.
        {
            let mut gs = self.gui_state.lock().unwrap();
            if gs.processes.len() != self.processes.len() {
                self.processes = gs.processes.clone();
            }
            self.scanner_tab.scan_status = gs.scan_status;
            self.attached = gs.attached.clone();
            if gs.word_size > 0 {
                self.hex_dump_tab.word_size = gs.word_size;
                self.schema_builder_tab.word_size = gs.word_size;
            }
            if gs.error.is_some() {
                self.error = gs.error.clone();
            }
            if gs.results.len() > self.scanner_tab.results.len() {
                self.scanner_tab.results = gs.results.clone();
            }
            if gs.schemas.len() != self.scanner_tab.schemas.len() {
                self.scanner_tab.schemas = gs.schemas.clone();
                self.hex_dump_tab.schemas = self.scanner_tab.schemas.clone();
            }
            if gs.cast_preview.is_some() {
                self.hex_dump_tab.cast_preview = gs.cast_preview.take();
            }
            if gs.scan_pattern != self.scanner_tab.scan_pattern {
                self.scanner_tab.scan_pattern = gs.scan_pattern.clone();
            }
            if gs.scan_errors.len() != self.scanner_tab.scan_errors.len() {
                self.scanner_tab.scan_errors = gs.scan_errors.clone();
            }
            if gs.inferred_fields != self.scanner_tab.inferred_fields {
                self.scanner_tab.inferred_fields = gs.inferred_fields.clone();
            }
            if !gs.hex_rows.is_empty() {
                self.hex_dump_tab.rows = gs.hex_rows.clone();
            } else {
                self.hex_dump_tab.rows.clear();
            }
            if gs.memory_layout.len() != self.hex_dump_tab.memory_layout.len() {
                self.hex_dump_tab.memory_layout = gs.memory_layout.clone();
            }
            // Sync watch list from server. Structural changes replace the list;
            // value-only refreshes update only the result field.
            {
                let server_addrs: Vec<u64> =
                    gs.watch_entries.iter().map(|(a, _, _, _, _, _)| *a).collect();
                let display_addrs: Vec<u64> =
                    self.hex_dump_tab.watched.iter().map(|w| w.addr).collect();
                if server_addrs != display_addrs {
                    self.hex_dump_tab.watched = gs
                        .watch_entries
                        .iter()
                        .map(|(addr, label, type_name, auto_refresh, bare_map, result)| {
                            super::hex_dump_tab::WatchedAddress {
                                addr: *addr,
                                label: label.clone(),
                                type_name: type_name.clone(),
                                auto_refresh: *auto_refresh,
                                bare_map: *bare_map,
                                result: result.clone(),
                            }
                        })
                        .collect();
                } else {
                    for (w, (_, _, _, _, _, result)) in self
                        .hex_dump_tab
                        .watched
                        .iter_mut()
                        .zip(gs.watch_entries.iter())
                    {
                        w.result = result.clone();
                    }
                }
            }

            // Sync watches into schema builder tab
            {
                let watch_addrs: Vec<u64> =
                    gs.watch_entries.iter().map(|(a, _, _, _, _, _)| *a).collect();
                let builder_addrs: Vec<u64> =
                    self.schema_builder_tab.watches.iter().map(|w| w.addr).collect();
                if watch_addrs != builder_addrs {
                    self.schema_builder_tab.watches = gs
                        .watch_entries
                        .iter()
                        .map(|(addr, _label, type_name, _, _, result)| WatchEntry {
                            addr: *addr,
                            label: _label.clone(),
                            type_name: type_name.clone(),
                            result: result.clone(),
                        })
                        .collect();
                    let old_len = self.schema_builder_tab.field_states.len();
                    let new_len = self.schema_builder_tab.watches.len();
                    self.schema_builder_tab
                        .field_states
                        .resize_with(new_len, FieldBuildState::default);
                    for i in old_len..new_len {
                        let label = &self.schema_builder_tab.watches[i].label;
                        self.schema_builder_tab.field_states[i].field_name =
                            sanitize_field_name(label);
                    }
                } else {
                    for (w, (_, _, _, _, _, result)) in self
                        .schema_builder_tab
                        .watches
                        .iter_mut()
                        .zip(gs.watch_entries.iter())
                    {
                        w.result = result.clone();
                    }
                }
            }

            // Schema builder results (take-semantics)
            if !gs.schema_validation.is_empty() {
                self.schema_builder_tab.validation_errors =
                    std::mem::take(&mut gs.schema_validation);
            }
            if let Some(result) = gs.schema_generated_code.take() {
                match result {
                    Ok(code) => {
                        self.schema_builder_tab.generated_code = code;
                        self.schema_builder_tab.code_expanded = true;
                    }
                    Err(msg) => {
                        self.schema_builder_tab.validation_errors = vec![msg];
                    }
                }
            }
            if let Some(status) = gs.schema_load_status.take() {
                self.schema_builder_tab.load_status = status;
            }
        }

        egui::TopBottomPanel::top("toolbar")
            .frame(
                egui::Frame::NONE
                    .fill(ctx.style().visuals.panel_fill)
                    .inner_margin(egui::Margin::symmetric(12, 10)),
            )
            .show(ctx, |ui| {
                self.toolbar(ui);
            });

        match self.active_tab {
            ActiveTab::Scanner => {
                egui::TopBottomPanel::bottom("saved_addresses")
                    .min_height(160.0)
                    .show(ctx, |ui| {
                        self.scanner_tab.saved_panel(ui);
                    });

                let view_addr = egui::CentralPanel::default()
                    .show(ctx, |ui| self.scanner_tab.central(ui, &self.attached))
                    .inner;

                if let Some((addr, type_ref)) = view_addr {
                    self.active_tab = ActiveTab::HexDump;
                    self.hex_dump_tab.base_addr = addr;
                    self.hex_dump_tab.addr_input = format!("{:X}", addr);
                    self.hex_dump_tab.sel = None;
                    if let Some(tr) = type_ref {
                        send_event(GuiEvent::WatchAddRequested { addr, type_ref: tr });
                    }
                    send_event(GuiEvent::HexDumpRequested(addr));
                    send_event(GuiEvent::MemoryLayoutRequested);
                }
            }
            ActiveTab::HexDump => {
                egui::SidePanel::right("mem_map")
                    .exact_width(36.0)
                    .resizable(false)
                    .show(ctx, |ui| {
                        self.hex_dump_tab.mem_map(ui);
                    });
                egui::SidePanel::right("watched_panel")
                    .min_width(200.0)
                    .default_width(220.0)
                    .max_width(340.0)
                    .show(ctx, |ui| {
                        self.hex_dump_tab.watched_panel(ui);
                    });
                let watch_req = egui::TopBottomPanel::bottom("hex_interpretation")
                    .min_height(120.0)
                    .show(ctx, |ui| self.hex_dump_tab.interpretation(ui))
                    .inner;
                if let Some((addr, type_ref)) = watch_req {
                    send_event(GuiEvent::WatchAddRequested { addr, type_ref });
                }
                egui::CentralPanel::default().show(ctx, |ui| {
                    self.hex_dump_tab.central(ui);
                });
            }
            ActiveTab::SchemaBuilder => {
                egui::CentralPanel::default().show(ctx, |ui| {
                    self.schema_builder_tab.central(ui);
                });
            }
        }

        if let Some(ref msg) = self.error.clone() {
            let mut dismiss = false;
            egui::Window::new("Error")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(msg);
                    if ui.button("OK").clicked() {
                        dismiss = true;
                    }
                });
            if dismiss {
                self.error = None;
                with_gui_state(|gs| gs.error = None);
            }
        }
    }
}
