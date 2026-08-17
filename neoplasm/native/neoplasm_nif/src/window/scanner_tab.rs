use super::with_gui_state;
use crate::types::{value_to_hex, ScanRegion, ScanStatus, TypeRef, ValueType};
use crate::window::events::{send_event, GuiEvent};

#[derive(Default, PartialEq, Clone, Copy)]
pub enum SearchMode {
    #[default]
    Primitive,
    Schema,
}

#[derive(Default)]
pub struct ScannerTab {
    pub search_mode: SearchMode,
    pub value_input: String,
    pub pattern_input: String,
    pub value_type: ValueType,
    pub array_element_type: ValueType,
    pub array_count: String,
    pub region: ScanRegion,
    pub results: Vec<u64>,
    pub scan_status: ScanStatus,
    pub scan_pattern: String,
    pub scan_errors: Vec<(String, String)>,
    pub schemas: Vec<crate::schema::SchemaInfo>,
    pub selected_schema: Option<usize>,
    pub schema_for_states: Option<usize>,
    pub field_states: Vec<FieldState>,
    pub saved: Vec<SavedAddress>,
    pub selected_saved: Option<usize>,
    /// Inferred field values for schema mode: (field_name, display_value)
    pub inferred_fields: Vec<(String, String)>,
    /// Type name (primitive or schema) used in the most recent scan — used when saving results.
    pub last_scan_type_name: String,
}

#[derive(Default, Clone)]
pub struct FieldState {
    pub checked: bool,
    pub value_input: String,
}

pub struct SavedAddress {
    pub addr: u64,
    pub label: String,
    pub type_name: String,
}

impl ScannerTab {
    /// Renders the scanner tab. Returns `Some((addr, type_ref))` when the user clicks "View"
    /// on a result, signalling the app to switch to the hex dump tab and optionally add a watch.
    /// The inner `Option<TypeRef>` is `None` when no watchable type is selected (e.g. schema mode
    /// with no schema chosen).
    pub fn central(
        &mut self,
        ui: &mut egui::Ui,
        attached: &Option<(u64, String)>,
    ) -> Option<(u64, Option<TypeRef>)> {
        let mut view_addr: Option<(u64, Option<TypeRef>)> = None;
        ui.columns(2, |cols| {
            // ── Left: results ────────────────────────────────────────────
            let results_bg = cols[0].visuals().extreme_bg_color;
            egui::Frame::NONE
                .fill(results_bg)
                .inner_margin(egui::Margin::symmetric(8, 6))
                .corner_radius(egui::CornerRadius::same(4))
                .show(&mut cols[0], |ui| {
                    ui.label(match self.scan_status {
                        ScanStatus::Done(n) => format!("Results ({} found)", n),
                        ScanStatus::Scanning => "Results (scanning…)".into(),
                        ScanStatus::Idle => "Results".into(),
                    });
                    egui::ScrollArea::vertical()
                        .id_salt("results_scroll")
                        .show(ui, |ui| {
                            for &addr in &self.results {
                                ui.horizontal(|ui| {
                                    ui.monospace(format!("0x{:016X}", addr));
                                    if ui.small_button("Save").clicked() {
                                        self.saved.push(SavedAddress {
                                            addr,
                                            label: String::new(),
                                            type_name: self.last_scan_type_name.clone(),
                                        });
                                    }
                                    if ui.small_button("View").clicked() {
                                        let type_ref = match self.search_mode {
                                            SearchMode::Schema => self
                                                .selected_schema
                                                .and_then(|i| self.schemas.get(i))
                                                .map(|s| TypeRef::Schema(s.module_atom)),
                                            SearchMode::Primitive => {
                                                Some(TypeRef::Primitive(self.value_type))
                                            }
                                        };
                                        view_addr = Some((addr, type_ref));
                                    }
                                });
                            }
                        });
                });

            // ── Right: scan controls ─────────────────────────────────────
            let ui = &mut cols[1];

            ui.horizontal(|ui| {
                ui.label("Region:");
                ui.selectable_value(&mut self.region, ScanRegion::Heap, "Heap");
                ui.selectable_value(&mut self.region, ScanRegion::Module, "Module");
            });

            ui.separator();

            let mode_changed = ui
                .horizontal(|ui| {
                    ui.selectable_value(&mut self.search_mode, SearchMode::Primitive, "Primitive")
                        .clicked()
                        || ui
                            .selectable_value(&mut self.search_mode, SearchMode::Schema, "Schema")
                            .clicked()
                })
                .inner;

            if mode_changed {
                self.scan_pattern.clear();
                self.scan_errors.clear();
                with_gui_state(|gs| {
                    gs.scan_pattern.clear();
                    gs.scan_errors.clear();
                });
            }

            ui.separator();

            let scanning = self.scan_status == ScanStatus::Scanning;
            let mut scan_event: Option<GuiEvent> = None;
            let mut schema_fields_changed = false;
            let can_scan;

            match self.search_mode {
                SearchMode::Schema => {
                    let schema_label = self
                        .selected_schema
                        .and_then(|i| self.schemas.get(i))
                        .map(|s| s.name.as_str())
                        .unwrap_or("— no schema —");
                    egui::ComboBox::from_id_salt("schema_select")
                        .selected_text(schema_label)
                        .width(ui.available_width() - 8.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.selected_schema, None, "— no schema —");
                            for (i, schema) in self.schemas.iter().enumerate() {
                                ui.selectable_value(
                                    &mut self.selected_schema,
                                    Some(i),
                                    &schema.name,
                                );
                            }
                        });

                    if self.selected_schema != self.schema_for_states {
                        self.schema_for_states = self.selected_schema;
                        let n = self
                            .selected_schema
                            .and_then(|i| self.schemas.get(i))
                            .map(|s| s.fields.len())
                            .unwrap_or(0);
                        self.field_states = vec![FieldState::default(); n];
                        schema_fields_changed = true;
                    }

                    if let Some(schema_idx) = self.selected_schema {
                        if let Some(schema) = self.schemas.get(schema_idx) {
                            ui.label(egui::RichText::new(format!("{} bytes", schema.size)).weak());

                            let field_meta: Vec<(String, String, Option<String>)> = schema
                                .fields
                                .iter()
                                .map(|f| {
                                    let type_label = match &f.points_to.0 {
                                        Some(pts) => format!("*{}", pts),
                                        None => f.data_type.0.clone(),
                                    };
                                    (f.name.0.clone(), f.data_type.0.clone(), Some(type_label))
                                })
                                .collect();

                            egui::ScrollArea::vertical()
                                .id_salt("schema_fields_scroll")
                                .max_height(280.0)
                                .show(ui, |ui| {
                                    for (fs, (name, dtype, type_label)) in
                                        self.field_states.iter_mut().zip(field_meta.iter())
                                    {
                                        let cb = ui.horizontal(|ui| {
                                            let r = ui.checkbox(&mut fs.checked, "");
                                            ui.label(name);
                                            ui.label(
                                                egui::RichText::new(
                                                    type_label.as_deref().unwrap_or(""),
                                                )
                                                .weak(),
                                            );
                                            r
                                        });
                                        if cb.inner.changed() {
                                            schema_fields_changed = true;
                                        }
                                        let input_resp = ui.add_enabled(
                                            fs.checked,
                                            egui::TextEdit::singleline(&mut fs.value_input)
                                                .desired_width(f32::INFINITY),
                                        );
                                        if input_resp.changed() {
                                            schema_fields_changed = true;
                                        }
                                        let mut bytes_display =
                                            value_to_hex(&fs.value_input, dtype);
                                        ui.add_enabled(
                                            false,
                                            egui::TextEdit::singleline(&mut bytes_display)
                                                .desired_width(f32::INFINITY)
                                                .font(egui::TextStyle::Monospace),
                                        );
                                        if !fs.checked {
                                            if let Some((_, val)) =
                                                self.inferred_fields.iter().find(|(n, _)| n == name)
                                            {
                                                fs.value_input = val.clone();
                                            }
                                        }
                                        ui.add_space(2.0);
                                    }
                                });
                        }
                    }

                    let schema_and_fields = self.selected_schema.and_then(|i| {
                        let schema = self.schemas.get(i)?;
                        let fields: Vec<(String, String)> = schema
                            .fields
                            .iter()
                            .zip(self.field_states.iter())
                            .filter(|(_, fs)| fs.checked && !fs.value_input.is_empty())
                            .map(|(f, fs)| (f.name.0.clone(), fs.value_input.clone()))
                            .collect();
                        if fields.is_empty() {
                            None
                        } else {
                            Some((schema.name.clone(), fields))
                        }
                    });
                    can_scan = schema_and_fields.is_some();

                    if schema_fields_changed {
                        if let Some((schema_name, fields)) = &schema_and_fields {
                            send_event(GuiEvent::SchemaPreviewRequested {
                                schema: schema_name.clone(),
                                fields: fields.clone(),
                            });
                        } else {
                            self.scan_pattern.clear();
                            self.scan_errors.clear();
                            with_gui_state(|gs| {
                                gs.scan_pattern.clear();
                                gs.scan_errors.clear();
                            });
                        }
                    }

                    if !self.scan_pattern.is_empty() {
                        ui.separator();
                        ui.label(egui::RichText::new("Pattern:").weak());
                        let mut pat_display = self.scan_pattern.clone();
                        ui.add_enabled(
                            false,
                            egui::TextEdit::multiline(&mut pat_display)
                                .desired_width(f32::INFINITY)
                                .font(egui::TextStyle::Monospace)
                                .desired_rows(2),
                        );
                    }

                    ui.separator();
                    if ui
                        .add_enabled(
                            attached.is_some()
                                && can_scan
                                && !scanning
                                && self.scan_errors.is_empty(),
                            egui::Button::new("Scan"),
                        )
                        .clicked()
                    {
                        if let Some((schema_name, fields)) = schema_and_fields {
                            self.results.clear();
                            with_gui_state(|gs| gs.results.clear());
                            self.last_scan_type_name = schema_name.clone();
                            scan_event = Some(GuiEvent::QueryRequested {
                                schema: schema_name,
                                fields,
                            });
                            self.scan_status = ScanStatus::Scanning;
                        }
                    }
                }

                SearchMode::Primitive => {
                    let mut preview_changed = mode_changed;

                    ui.horizontal(|ui| {
                        ui.label("Type:");
                        egui::ComboBox::from_id_salt("type_select")
                            .selected_text(self.value_type.label())
                            .show_ui(ui, |ui| {
                                for &t in ValueType::all() {
                                    if ui
                                        .selectable_value(&mut self.value_type, t, t.label())
                                        .changed()
                                    {
                                        with_gui_state(|gs| {
                                            gs.scan_pattern.clear();
                                            gs.scan_errors.clear();
                                        });
                                        preview_changed = true;
                                    }
                                }
                            });
                    });

                    ui.separator();

                    let is_bytes = self.value_type == ValueType::Bytes;
                    let is_array = self.value_type == ValueType::Array;
                    let is_pointer = self.value_type == ValueType::Pointer;
                    let is_saved_address = self.value_type == ValueType::SavedAddress;

                    if is_array {
                        ui.horizontal(|ui| {
                            ui.label("Element:");
                            let prev_elem = self.array_element_type;
                            egui::ComboBox::from_id_salt("array_elem_type")
                                .selected_text(self.array_element_type.label())
                                .width(100.0)
                                .show_ui(ui, |ui| {
                                    for &t in ValueType::primitive_types() {
                                        ui.selectable_value(
                                            &mut self.array_element_type,
                                            t,
                                            t.label(),
                                        );
                                    }
                                });
                            if self.array_element_type != prev_elem {
                                preview_changed = true;
                            }
                            ui.label("Count:");
                            if ui
                                .add(
                                    egui::TextEdit::singleline(&mut self.array_count)
                                        .desired_width(60.0),
                                )
                                .changed()
                            {
                                preview_changed = true;
                            }
                        });
                    }

                    if is_saved_address {
                        let saved_options: Vec<(u64, String)> = self
                            .saved
                            .iter()
                            .map(|s| (s.addr, s.label.clone()))
                            .collect();
                        let selected_label = self
                            .selected_saved
                            .and_then(|i| saved_options.get(i))
                            .map(|(addr, lbl)| {
                                if lbl.is_empty() {
                                    format!("0x{:016X}", addr)
                                } else {
                                    format!("0x{:016X}  {}", addr, lbl)
                                }
                            })
                            .unwrap_or_else(|| "— select saved address —".to_string());
                        let inner_changed = egui::ComboBox::from_id_salt("saved_addr_select")
                            .selected_text(selected_label)
                            .width(ui.available_width() - 8.0)
                            .show_ui(ui, |ui| {
                                let mut changed = false;
                                for (i, (addr, lbl)) in saved_options.iter().enumerate() {
                                    let entry_label = if lbl.is_empty() {
                                        format!("0x{:016X}", addr)
                                    } else {
                                        format!("0x{:016X}  {}", addr, lbl)
                                    };
                                    if ui
                                        .selectable_value(
                                            &mut self.selected_saved,
                                            Some(i),
                                            entry_label,
                                        )
                                        .changed()
                                    {
                                        changed = true;
                                    }
                                }
                                changed
                            })
                            .inner
                            .unwrap_or(false);
                        if inner_changed {
                            preview_changed = true;
                        }
                    } else {
                        let value_label = if is_bytes {
                            "Pattern (hex):"
                        } else if is_array {
                            "Values (space-separated):"
                        } else if is_pointer {
                            "Address (hex):"
                        } else {
                            "Value:"
                        };
                        ui.label(value_label);
                        let input_ref = if is_bytes {
                            &mut self.pattern_input
                        } else {
                            &mut self.value_input
                        };
                        if ui
                            .add(egui::TextEdit::singleline(input_ref).desired_width(f32::INFINITY))
                            .changed()
                        {
                            preview_changed = true;
                        }
                    }

                    if preview_changed {
                        let type_name = self.build_type_name(is_array);
                        let value = self.build_value(is_bytes);
                        if !value.is_empty() {
                            send_event(GuiEvent::PrimitivePreviewRequested { type_name, value });
                        }
                    }

                    ui.label("Bytes:");
                    let mut pat_display = self.scan_pattern.clone();
                    ui.add_enabled(
                        false,
                        egui::TextEdit::singleline(&mut pat_display)
                            .desired_width(f32::INFINITY)
                            .font(egui::TextStyle::Monospace),
                    );

                    can_scan = if is_saved_address {
                        self.selected_saved
                            .map(|i| i < self.saved.len())
                            .unwrap_or(false)
                    } else if is_bytes {
                        !self.pattern_input.is_empty()
                    } else if is_array {
                        !self.value_input.is_empty() && !self.array_count.is_empty()
                    } else if is_pointer {
                        Self::parse_pointer_input(&self.value_input).is_some()
                    } else {
                        !self.value_input.is_empty()
                    };

                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(
                                attached.is_some()
                                    && can_scan
                                    && !scanning
                                    && self.scan_errors.is_empty(),
                                egui::Button::new("Scan"),
                            )
                            .clicked()
                        {
                            self.results.clear();
                            with_gui_state(|gs| gs.results.clear());
                            self.last_scan_type_name = self.build_type_name(is_array);
                            scan_event = Some(GuiEvent::PrimitiveScanRequested {
                                type_name: self.build_type_name(is_array),
                                value: self.build_value(is_bytes),
                                region: self.region,
                                next: false,
                            });
                            self.scan_status = ScanStatus::Scanning;
                        }

                        let can_next = attached.is_some()
                            && can_scan
                            && !scanning
                            && !self.results.is_empty()
                            && self.scan_errors.is_empty();
                        if ui
                            .add_enabled(can_next, egui::Button::new("Next Scan"))
                            .clicked()
                        {
                            self.last_scan_type_name = self.build_type_name(is_array);
                            scan_event = Some(GuiEvent::PrimitiveScanRequested {
                                type_name: self.build_type_name(is_array),
                                value: self.build_value(is_bytes),
                                region: self.region,
                                next: true,
                            });
                            self.scan_status = ScanStatus::Scanning;
                        }
                    });
                }
            }

            if !self.scan_errors.is_empty() {
                ui.separator();
                egui::ScrollArea::vertical()
                    .id_salt("scan_errors_scroll")
                    .max_height(120.0)
                    .show(ui, |ui| {
                        let red = egui::Color32::from_rgb(220, 80, 80);
                        for (i, (key, msg)) in self.scan_errors.iter().enumerate() {
                            if i > 0 {
                                ui.add_space(2.0);
                            }
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(key)
                                        .small()
                                        .color(egui::Color32::from_gray(160))
                                        .monospace(),
                                )
                                .wrap(),
                            );
                            ui.add(
                                egui::Label::new(egui::RichText::new(msg).small().color(red))
                                    .wrap(),
                            );
                        }
                    });
            }

            if let Some(event) = scan_event {
                send_event(event);
            }
        });
        view_addr
    }

    pub fn saved_panel(&mut self, ui: &mut egui::Ui) {
        ui.label("Saved Addresses");
        egui::ScrollArea::vertical()
            .id_salt("saved_scroll")
            .max_height(220.0)
            .show(ui, |ui| {
                let mut to_remove = None;
                for (i, entry) in self.saved.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        ui.monospace(format!("0x{:016X}", entry.addr));
                        ui.text_edit_singleline(&mut entry.label);
                        ui.label(&entry.type_name);
                        if ui.small_button("✕").clicked() {
                            to_remove = Some(i);
                        }
                    });
                }
                if let Some(i) = to_remove {
                    self.saved.remove(i);
                }
            });
    }

    fn build_type_name(&self, is_array: bool) -> String {
        match self.value_type {
            ValueType::Pointer | ValueType::SavedAddress => "word".to_string(),
            _ if is_array => format!(
                "array/{}/{}",
                self.array_element_type.type_name(),
                self.array_count
            ),
            _ => self.value_type.type_name().to_string(),
        }
    }

    fn build_value(&self, is_bytes: bool) -> String {
        match self.value_type {
            ValueType::Pointer => Self::parse_pointer_input(&self.value_input)
                .map(|v| v.to_string())
                .unwrap_or_default(),
            ValueType::SavedAddress => self
                .selected_saved
                .and_then(|i| self.saved.get(i))
                .map(|s| s.addr.to_string())
                .unwrap_or_default(),
            _ if is_bytes => self.pattern_input.clone(),
            _ => self.value_input.clone(),
        }
    }

    /// Parse a hex address string (with or without `0x` prefix) into a u64.
    fn parse_pointer_input(s: &str) -> Option<u64> {
        let s = s.trim().trim_start_matches("0x").trim_start_matches("0X");
        u64::from_str_radix(s, 16).ok()
    }
}
