use crate::window::events::{send_event, GuiEvent};

// ---------------------------------------------------------------------------
// Schema Builder tab — builds a Rekto schema from the watch list
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct SchemaBuilderTab {
    pub module_name: String,
    pub base_addr_input: String,
    pub base_addr: Option<u64>,
    /// Mirrors the watch list — synced each frame from GuiState.
    pub watches: Vec<WatchEntry>,
    /// Parallel to `watches`; per-watch builder state.
    pub field_states: Vec<FieldBuildState>,
    // Results pushed from Elixir
    pub validation_errors: Vec<String>,
    pub generated_code: String,
    pub code_expanded: bool,
    pub load_status: String,
    pub word_size: u64,
}

pub struct WatchEntry {
    pub addr: u64,
    pub label: String,
    pub type_name: String,
    pub result: Result<String, String>,
}

#[derive(Clone)]
pub struct FieldBuildState {
    pub checked: bool,
    pub field_name: String,
    pub constraints: String,
}

impl Default for FieldBuildState {
    fn default() -> Self {
        Self {
            checked: false,
            field_name: String::new(),
            constraints: String::new(),
        }
    }
}

impl SchemaBuilderTab {
    /// Auto-compute base address from the lowest checked watch.
    fn auto_base_addr(&self) -> Option<u64> {
        self.watches
            .iter()
            .zip(self.field_states.iter())
            .filter(|(_, fs)| fs.checked)
            .map(|(w, _)| w.addr)
            .min()
    }

    /// Collect checked watches into field specs for the Elixir side.
    fn checked_field_specs(&self) -> Vec<(String, String, u64, u64, String)> {
        let base = self.base_addr.unwrap_or(0);
        let mut specs: Vec<_> = self
            .watches
            .iter()
            .zip(self.field_states.iter())
            .filter(|(_, fs)| fs.checked)
            .map(|(w, fs)| {
                let offset = w.addr.saturating_sub(base);
                let size = type_size_hint(&w.type_name, self.word_size);
                (
                    fs.field_name.clone(),
                    w.type_name.clone(),
                    offset,
                    size,
                    fs.constraints.clone(),
                )
            })
            .collect();
        specs.sort_by_key(|s| s.2); // sort by offset
        specs
    }

    pub fn central(&mut self, ui: &mut egui::Ui) {
        // ── Header: module name + base address ──────────────────────────
        ui.horizontal(|ui| {
            ui.label("Module:");
            ui.add(
                egui::TextEdit::singleline(&mut self.module_name)
                    .desired_width(240.0)
                    .hint_text("MyGame.Player"),
            );
            ui.separator();
            ui.label("Base addr:");
            let resp = ui.add(
                egui::TextEdit::singleline(&mut self.base_addr_input)
                    .desired_width(160.0)
                    .font(egui::TextStyle::Monospace)
                    .hint_text("auto"),
            );
            // Parse base address from input
            let trimmed = self
                .base_addr_input
                .trim()
                .trim_start_matches("0x")
                .trim_start_matches("0X");
            if trimmed.is_empty() {
                // Auto mode: compute from lowest checked watch
                self.base_addr = self.auto_base_addr();
            } else {
                self.base_addr = u64::from_str_radix(trimmed, 16).ok();
            }
            // Show effective base address
            if let Some(base) = self.base_addr {
                ui.label(
                    egui::RichText::new(format!("= 0x{:X}", base))
                        .monospace()
                        .weak(),
                );
            }
            let _ = resp;
        });

        ui.separator();

        // ── Watch / field table ─────────────────────────────────────────
        if self.watches.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label(
                    egui::RichText::new(
                        "No watched addresses. Use the Hex Dump tab to watch addresses first.",
                    )
                    .weak(),
                );
            });
            return;
        }

        let base = self.base_addr.unwrap_or(0);

        egui::ScrollArea::vertical()
            .id_salt("schema_builder_fields")
            .max_height(ui.available_height() - 200.0)
            .show(ui, |ui| {
                // Header row
                ui.horizontal(|ui| {
                    ui.add_space(28.0); // checkbox width
                    ui.label(egui::RichText::new("Address").weak().small());
                    ui.add_space(60.0);
                    ui.label(egui::RichText::new("Type").weak().small());
                    ui.add_space(40.0);
                    ui.label(egui::RichText::new("Offset").weak().small());
                    ui.add_space(20.0);
                    ui.label(egui::RichText::new("Label").weak().small());
                    ui.add_space(40.0);
                    ui.label(egui::RichText::new("→ Field Name").weak().small());
                    ui.add_space(40.0);
                    ui.label(egui::RichText::new("Value").weak().small());
                });

                for (i, (w, fs)) in self
                    .watches
                    .iter()
                    .zip(self.field_states.iter_mut())
                    .enumerate()
                {
                    let offset = w.addr.saturating_sub(base);

                    ui.horizontal(|ui| {
                        ui.checkbox(&mut fs.checked, "");
                        ui.label(
                            egui::RichText::new(format!("0x{:X}", w.addr))
                                .monospace()
                                .small(),
                        );
                        ui.label(egui::RichText::new(&w.type_name).weak().small());
                        if fs.checked {
                            ui.label(
                                egui::RichText::new(format!("+0x{:X}", offset))
                                    .monospace()
                                    .small()
                                    .color(egui::Color32::from_rgb(100, 180, 100)),
                            );
                        } else {
                            ui.label(
                                egui::RichText::new(format!("+0x{:X}", offset))
                                    .monospace()
                                    .small()
                                    .weak(),
                            );
                        }
                        if !w.label.is_empty() {
                            ui.label(egui::RichText::new(&w.label).small().weak());
                        }
                        ui.label("→");
                        ui.add_enabled(
                            fs.checked,
                            egui::TextEdit::singleline(&mut fs.field_name)
                                .desired_width(120.0)
                                .hint_text("field_name"),
                        );
                        // Value preview
                        match &w.result {
                            Ok(val) if !val.is_empty() => {
                                let short = if val.len() > 40 {
                                    format!("{}…", &val[..40])
                                } else {
                                    val.clone()
                                };
                                ui.label(
                                    egui::RichText::new(short)
                                        .monospace()
                                        .small()
                                        .color(egui::Color32::from_gray(170)),
                                );
                            }
                            Err(err) => {
                                let short = if err.len() > 30 {
                                    format!("{}…", &err[..30])
                                } else {
                                    err.clone()
                                };
                                ui.label(
                                    egui::RichText::new(short)
                                        .small()
                                        .color(egui::Color32::from_rgb(220, 80, 80)),
                                );
                            }
                            _ => {}
                        }
                    });

                    // Constraints row (only when checked)
                    if fs.checked {
                        ui.horizontal(|ui| {
                            ui.add_space(28.0);
                            ui.label(egui::RichText::new("constraints:").weak().small());
                            ui.add(
                                egui::TextEdit::singleline(&mut fs.constraints)
                                    .desired_width(300.0)
                                    .hint_text("range: {0, 100}")
                                    .font(egui::TextStyle::Monospace),
                            );
                        });
                    }

                    if i < self.watches.len() - 1 {
                        ui.add_space(2.0);
                    }
                }
            });

        ui.separator();

        // ── Offset preview ──────────────────────────────────────────────
        let specs = self.checked_field_specs();
        if !specs.is_empty() {
            let preview: String = specs
                .iter()
                .map(|(name, _typ, offset, size, _)| {
                    let n = if name.is_empty() { "?" } else { name.as_str() };
                    format!("{} +0x{:X} ({}B)", n, offset, size)
                })
                .collect::<Vec<_>>()
                .join(", ");
            ui.label(egui::RichText::new(preview).monospace().small().weak());
            ui.separator();
        }

        // ── Action buttons ──────────────────────────────────────────────
        ui.horizontal(|ui| {
            let has_checked = self.field_states.iter().any(|fs| fs.checked);
            let has_module = !self.module_name.trim().is_empty();

            if ui
                .add_enabled(has_checked && has_module, egui::Button::new("Validate"))
                .clicked()
            {
                send_event(GuiEvent::SchemaValidateRequested {
                    module_name: self.module_name.clone(),
                    fields: self.checked_field_specs(),
                });
            }

            if ui
                .add_enabled(has_checked && has_module, egui::Button::new("Generate"))
                .clicked()
            {
                let base = self.base_addr.unwrap_or(0);
                send_event(GuiEvent::SchemaGenerateRequested {
                    module_name: self.module_name.clone(),
                    base_addr: base,
                    fields: self.checked_field_specs(),
                });
            }

            if ui
                .add_enabled(!self.generated_code.is_empty(), egui::Button::new("Compile"))
                .on_hover_text("Compile the generated code into the running VM")
                .clicked()
            {
                send_event(GuiEvent::SchemaLoadRequested {
                    code: self.generated_code.clone(),
                });
            }

            if ui
                .add_enabled(!self.generated_code.is_empty(), egui::Button::new("Save…"))
                .on_hover_text("Save generated code to a .exs file")
                .clicked()
            {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Elixir", &["exs", "ex"])
                    .set_file_name("schema.exs")
                    .save_file()
                {
                    send_event(GuiEvent::SchemaSaveRequested {
                        code: self.generated_code.clone(),
                        path: path.display().to_string(),
                    });
                }
            }

            ui.separator();

            if ui
                .button("Load File…")
                .on_hover_text("Load and compile a .exs schema file")
                .clicked()
            {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Elixir", &["exs", "ex"])
                    .pick_file()
                {
                    if let Ok(code) = std::fs::read_to_string(&path) {
                        send_event(GuiEvent::SchemaLoadRequested { code });
                    }
                }
            }
        });

        // ── Validation errors ───────────────────────────────────────────
        if !self.validation_errors.is_empty() {
            ui.separator();
            for err in &self.validation_errors {
                ui.label(
                    egui::RichText::new(err)
                        .small()
                        .color(egui::Color32::from_rgb(220, 80, 80)),
                );
            }
        }

        // ── Load/save status ────────────────────────────────────────────
        if !self.load_status.is_empty() {
            ui.separator();
            let color = if self.load_status.starts_with("Error") {
                egui::Color32::from_rgb(220, 80, 80)
            } else {
                egui::Color32::from_rgb(100, 200, 100)
            };
            ui.label(egui::RichText::new(&self.load_status).small().color(color));
        }

        // ── Collapsible generated code ──────────────────────────────────
        if !self.generated_code.is_empty() {
            ui.separator();
            egui::CollapsingHeader::new("Generated Code")
                .default_open(self.code_expanded)
                .show(ui, |ui| {
                    let mut code = self.generated_code.clone();
                    ui.add(
                        egui::TextEdit::multiline(&mut code)
                            .desired_width(f32::INFINITY)
                            .desired_rows(12)
                            .font(egui::TextStyle::Monospace)
                            .interactive(false),
                    );
                });
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Best-effort name sanitisation: "My Label!" → "my_label"
pub fn sanitize_field_name(label: &str) -> String {
    let mut s = String::with_capacity(label.len());
    for c in label.chars() {
        if c.is_alphanumeric() || c == '_' {
            s.push(c.to_ascii_lowercase());
        } else if c == ' ' || c == '-' {
            s.push('_');
        }
    }
    // Ensure it starts with a lowercase letter or underscore
    if s.starts_with(|c: char| c.is_ascii_digit()) {
        s.insert(0, '_');
    }
    s
}

/// Map a type name to its expected byte size.
/// Handles primitives, `[type; N]` arrays, and `[string_buffer; N]`.
/// `word_size` is the configured word width in bytes (4 or 8).
pub fn type_size_hint(type_name: &str, word_size: u64) -> u64 {
    match type_name {
        "bool" | "u8" | "i8" | "byte" => 1,
        "u16" | "i16" => 2,
        "u32" | "i32" | "f32" => 4,
        "u64" | "i64" | "f64" => 8,
        "word" => word_size,
        s if s.starts_with('[') && s.ends_with(']') => {
            // "[elem; count]" or "[string_buffer; N]"
            let inner = &s[1..s.len() - 1];
            if let Some((lhs, rhs)) = inner.split_once(';') {
                let elem = lhs.trim();
                let count: u64 = rhs.trim().parse().unwrap_or(1);
                if elem == "string_buffer" {
                    count
                } else {
                    let elem_size = type_size_hint(elem, word_size);
                    elem_size * count
                }
            } else {
                word_size
            }
        }
        _ => word_size,
    }
}
