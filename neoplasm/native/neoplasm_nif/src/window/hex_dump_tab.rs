use super::events::{send_event, GuiEvent};
use super::scanner_tab::SearchMode;
use super::schema_builder_tab::type_size_hint;
use crate::types::{HexRow, MemKind, MemRegion, TypeRef, ValueType};

const WATCH_COLORS: [egui::Color32; 8] = [
    egui::Color32::from_rgba_premultiplied(220, 120, 40, 55),
    egui::Color32::from_rgba_premultiplied(60, 160, 220, 55),
    egui::Color32::from_rgba_premultiplied(180, 60, 200, 55),
    egui::Color32::from_rgba_premultiplied(60, 200, 100, 55),
    egui::Color32::from_rgba_premultiplied(220, 60, 80, 55),
    egui::Color32::from_rgba_premultiplied(200, 200, 40, 55),
    egui::Color32::from_rgba_premultiplied(40, 200, 200, 55),
    egui::Color32::from_rgba_premultiplied(200, 120, 180, 55),
];

pub struct WatchedAddress {
    pub addr: u64,
    pub label: String,
    pub type_name: String,
    pub auto_refresh: bool,
    pub bare_map: bool,
    pub result: Result<String, String>,
}

pub struct HexDumpTab {
    pub rows: Vec<HexRow>,
    pub memory_layout: Vec<MemRegion>,
    pub base_addr: u64,
    pub addr_input: String,
    /// Selected byte range: (start_global_byte, end_global_byte), inclusive.
    /// Global byte index = flat index across all rows' bytes.
    pub sel: Option<(usize, usize)>,
    /// Display-only — canonical list is owned by Elixir; updated via set_watch_entries.
    pub watched: Vec<WatchedAddress>,
    pub schemas: Vec<crate::schema::SchemaInfo>,
    pub interp_mode: SearchMode,
    pub interp_type: ValueType,
    pub interp_schema: Option<usize>,
    pub cast_preview: Option<Result<String, String>>,
    last_interp_addr: Option<u64>,
    // Compound type sub-inputs
    pub array_element_type: ValueType,
    pub array_count: String,
    pub string_buffer_size: String,
    pub word_size: u64,
}

impl Default for HexDumpTab {
    fn default() -> Self {
        Self {
            rows: vec![],
            memory_layout: vec![],
            base_addr: 0,
            addr_input: String::new(),
            sel: None,
            watched: vec![],
            schemas: vec![],
            interp_mode: SearchMode::Primitive,
            interp_type: ValueType::U32,
            interp_schema: None,
            cast_preview: None,
            last_interp_addr: None,
            array_element_type: ValueType::U8,
            array_count: String::new(),
            string_buffer_size: String::new(),
            word_size: 4,
        }
    }
}

impl HexDumpTab {
    /// All bytes in a flat array across rows.
    fn all_bytes(&self) -> Vec<u8> {
        self.rows.iter().flat_map(|r| r.bytes.iter().copied()).collect()
    }

    pub fn selected_bytes(&self) -> Vec<u8> {
        let Some((start, end)) = self.sel else {
            return vec![];
        };
        let all = self.all_bytes();
        let end = end.min(all.len().saturating_sub(1));
        all[start..=end].to_vec()
    }

    /// Address of the first selected byte, if any.
    pub fn selected_addr(&self) -> Option<u64> {
        let (start, _) = self.sel?;
        let mut offset = start;
        for row in &self.rows {
            if offset < row.bytes.len() {
                return Some(row.addr + offset as u64);
            }
            offset -= row.bytes.len();
        }
        None
    }


    pub fn central(&mut self, ui: &mut egui::Ui) {
        // Address navigation bar
        ui.horizontal(|ui| {
            ui.label("Address:");
            let resp = ui.add(
                egui::TextEdit::singleline(&mut self.addr_input)
                    .desired_width(180.0)
                    .font(egui::TextStyle::Monospace),
            );
            let go = ui.button("Go").clicked()
                || (resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));
            if go {
                let trimmed = self.addr_input.trim_start_matches("0x");
                if let Ok(addr) = u64::from_str_radix(trimmed, 16) {
                    self.base_addr = addr;
                    self.sel = None;
                    send_event(GuiEvent::HexDumpRequested(addr));
                    send_event(GuiEvent::MemoryLayoutRequested);
                }
            }
            if !self.rows.is_empty() {
                ui.label(
                    egui::RichText::new(format!("0x{:X}", self.base_addr))
                        .weak()
                        .monospace(),
                );
            }
        });

        ui.separator();

        if self.rows.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label(
                    egui::RichText::new("Click 'View' on a result address to inspect memory")
                        .weak(),
                );
            });
            return;
        }

        // Pre-compute watch byte ranges: (start_addr, end_addr_exclusive, color_index)
        let watch_ranges: Vec<(u64, u64, usize)> = self
            .watched
            .iter()
            .enumerate()
            .map(|(i, w)| {
                let size = type_size_hint(&w.type_name, self.word_size);
                (w.addr, w.addr + size, i % WATCH_COLORS.len())
            })
            .collect();

        // Snapshot row data to avoid borrow issues with self.sel mutation
        let row_data: Vec<(u64, Vec<u8>, Vec<String>)> = self
            .rows
            .iter()
            .map(|r| (r.addr, r.bytes.clone(), r.annotations.clone()))
            .collect();
        let cur_sel = self.sel;

        let mut new_sel = cur_sel;
        egui::ScrollArea::vertical()
            .id_salt("hex_dump_scroll")
            .show(ui, |ui| {
                let mut global_byte_offset: usize = 0;

                for (addr, bytes, annotations) in &row_data {
                    let row_byte_start = global_byte_offset;
                    let row_byte_end = row_byte_start + bytes.len();
                    let row_last = row_byte_end.saturating_sub(1);

                    let row_has_selection = cur_sel.is_some_and(|(s, e)| {
                        s < row_byte_end && e >= row_byte_start
                    });

                    let row_resp = ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("{:016X}", addr))
                                .monospace()
                                .color(egui::Color32::from_gray(110)),
                        );
                        ui.add_space(6.0);

                        let base_color = primary_byte_color(annotations);
                        let sel_color = egui::Color32::from_rgb(120, 170, 255);

                        for (byte_i, &b) in bytes.iter().enumerate() {
                            let gi = row_byte_start + byte_i;
                            let byte_addr = addr + byte_i as u64;
                            let is_byte_sel =
                                cur_sel.is_some_and(|(s, e)| gi >= s && gi <= e);
                            let color = if is_byte_sel { sel_color } else { base_color };
                            let label = ui.add(
                                egui::Label::new(
                                    egui::RichText::new(format!("{:02X}", b))
                                        .monospace()
                                        .color(color),
                                )
                                .sense(egui::Sense::click()),
                            );

                            // Per-byte watch color overlay
                            if let Some(&(_, _, ci)) = watch_ranges
                                .iter()
                                .find(|(ws, we, _)| byte_addr >= *ws && byte_addr < *we)
                            {
                                ui.painter().rect_filled(
                                    label.rect,
                                    egui::CornerRadius::ZERO,
                                    WATCH_COLORS[ci],
                                );
                            }

                            if label.clicked() {
                                if ui.input(|i| i.modifiers.shift) {
                                    // Shift+click: extend from anchor
                                    new_sel = Some(match cur_sel {
                                        Some((s, _)) => (s.min(gi), s.max(gi)),
                                        None => (gi, gi),
                                    });
                                } else if cur_sel == Some((row_byte_start, row_last)) {
                                    // Row already selected → refine to single byte
                                    new_sel = Some((gi, gi));
                                } else {
                                    // Select whole row
                                    new_sel = Some((row_byte_start, row_last));
                                }
                            }
                        }

                        ui.add_space(4.0);

                        let ascii: String = bytes
                            .iter()
                            .map(|&b| {
                                if (32..=126).contains(&b) {
                                    b as char
                                } else {
                                    '.'
                                }
                            })
                            .collect();
                        ui.label(
                            egui::RichText::new(ascii)
                                .monospace()
                                .color(egui::Color32::from_gray(150)),
                        );
                        ui.add_space(8.0);

                        if !annotations.is_empty() {
                            ui.label(
                                egui::RichText::new(annotations.join("  "))
                                    .small()
                                    .color(egui::Color32::from_rgb(70, 190, 190)),
                            );
                        }
                    });

                    if row_has_selection {
                        ui.painter().rect_filled(
                            row_resp.response.rect,
                            egui::CornerRadius::ZERO,
                            egui::Color32::from_rgba_premultiplied(80, 120, 220, 25),
                        );
                    }

                    global_byte_offset = row_byte_end;
                }
            });
        self.sel = new_sel;
    }

    pub fn watched_panel(&mut self, ui: &mut egui::Ui) {
        ui.label("Watched");
        ui.separator();

        if self.watched.is_empty() {
            ui.label(egui::RichText::new("No watched addresses").weak().small());
            return;
        }

        egui::ScrollArea::vertical()
            .id_salt("watched_scroll")
            .show(ui, |ui| {
                let mut remove_addr: Option<u64> = None;
                let mut info_change: Option<(u64, String, bool, bool)> = None;

                for (wi, w) in self.watched.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        // Color swatch matching hex grid overlay
                        let color = WATCH_COLORS[wi % WATCH_COLORS.len()];
                        let (rect, _) =
                            ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                        ui.painter().rect_filled(rect, egui::CornerRadius::same(2), color);

                        if ui.small_button("✕").clicked() {
                            remove_addr = Some(w.addr);
                        }
                        ui.label(
                            egui::RichText::new(format!("0x{:X}", w.addr))
                                .monospace()
                                .small(),
                        );
                    });

                    let label_resp = ui.text_edit_singleline(&mut w.label);

                    let (auto_changed, bare_changed) = ui
                        .horizontal(|ui| {
                            let a = ui
                                .checkbox(
                                    &mut w.auto_refresh,
                                    egui::RichText::new("Auto").small(),
                                )
                                .changed();
                            let b = ui
                                .checkbox(
                                    &mut w.bare_map,
                                    egui::RichText::new("Map").small(),
                                )
                                .changed();
                            (a, b)
                        })
                        .inner;

                    if label_resp.lost_focus() || auto_changed || bare_changed {
                        info_change =
                            Some((w.addr, w.label.clone(), w.auto_refresh, w.bare_map));
                    }

                    ui.label(egui::RichText::new(&w.type_name).weak().small());

                    match &w.result {
                        Ok(val) if !val.is_empty() => {
                            let mut display = val.clone();
                            ui.add(
                                egui::TextEdit::multiline(&mut display)
                                    .desired_rows(4)
                                    .desired_width(f32::INFINITY)
                                    .font(egui::TextStyle::Monospace)
                                    .interactive(false),
                            );
                        }
                        Ok(_) => {
                            ui.label(egui::RichText::new("—").weak().small());
                        }
                        Err(err) => {
                            let mut display = err.clone();
                            ui.add(
                                egui::TextEdit::multiline(&mut display)
                                    .desired_rows(2)
                                    .desired_width(f32::INFINITY)
                                    .font(egui::TextStyle::Monospace)
                                    .text_color(egui::Color32::from_rgb(220, 80, 80))
                                    .interactive(false),
                            );
                        }
                    }

                    ui.add_space(4.0);
                    ui.separator();
                }

                if let Some(addr) = remove_addr {
                    send_event(GuiEvent::WatchRemoveRequested { addr });
                }
                if let Some((addr, label, auto_refresh, bare_map)) = info_change {
                    send_event(GuiEvent::WatchInfoChanged {
                        addr,
                        label,
                        auto_refresh,
                        bare_map,
                    });
                }
            });
    }

    pub fn mem_map(&self, ui: &mut egui::Ui) {
        let available = ui.available_size();
        let (rect, _) = ui.allocate_exact_size(available, egui::Sense::hover());
        let painter = ui.painter_at(rect);

        painter.rect_filled(rect, egui::CornerRadius::ZERO, egui::Color32::from_gray(15));

        if self.memory_layout.is_empty() {
            return;
        }

        let min_addr = self.memory_layout.iter().map(|r| r.base).min().unwrap_or(0);
        let max_addr = self
            .memory_layout
            .iter()
            .map(|r| r.base.saturating_add(r.size))
            .max()
            .unwrap_or(1);

        let total = (max_addr - min_addr) as f32;
        if total == 0.0 {
            return;
        }

        let h = rect.height();

        for region in &self.memory_layout {
            let y_start = rect.top() + (region.base - min_addr) as f32 / total * h;
            let y_end = rect.top()
                + (region.base.saturating_add(region.size) - min_addr) as f32 / total * h;
            let y_end = y_end.max(y_start + 2.0);

            let color = match region.kind {
                MemKind::Heap => egui::Color32::from_rgb(200, 120, 30),
                MemKind::Module => egui::Color32::from_rgb(130, 60, 200),
                MemKind::Other => egui::Color32::from_gray(50),
            };

            painter.rect_filled(
                egui::Rect::from_x_y_ranges(rect.x_range(), y_start..=y_end),
                egui::CornerRadius::ZERO,
                color,
            );
        }

        if self.base_addr > 0 && min_addr < max_addr {
            let y = rect.top() + self.base_addr.saturating_sub(min_addr) as f32 / total * h;
            painter.hline(
                rect.x_range(),
                y,
                egui::Stroke::new(2.0, egui::Color32::RED),
            );
        }
    }

    /// Returns `Some((addr, type_ref))` when the user clicks "Watch".
    pub fn interpretation(&mut self, ui: &mut egui::Ui) -> Option<(u64, TypeRef)> {
        let Some(addr) = self.selected_addr() else {
            ui.label(egui::RichText::new("Click a byte to select").weak());
            return None;
        };

        let bytes = self.selected_bytes();
        ui.label(egui::RichText::new(format!("{} bytes selected", bytes.len())).weak());
        let hex: String = bytes
            .iter()
            .map(|b| format!("{:02X}", b))
            .collect::<Vec<_>>()
            .join(" ");
        let mut hex_display = hex;
        ui.add(
            egui::TextEdit::singleline(&mut hex_display)
                .desired_width(f32::INFINITY)
                .font(egui::TextStyle::Monospace)
                .interactive(false),
        );

        ui.separator();

        // Mode + type selector — same structure as scanner tab
        let mut preview_changed = ui
            .horizontal(|ui| {
                ui.selectable_value(&mut self.interp_mode, SearchMode::Primitive, "Primitive")
                    .clicked()
                    || ui
                        .selectable_value(&mut self.interp_mode, SearchMode::Schema, "Schema")
                        .clicked()
            })
            .inner;

        let type_ref: Option<TypeRef> = match self.interp_mode {
            SearchMode::Primitive => {
                ui.horizontal(|ui| {
                    ui.label("Type:");
                    egui::ComboBox::from_id_salt("interp_type_select")
                        .selected_text(self.interp_type.label())
                        .show_ui(ui, |ui| {
                            for &t in ValueType::watchable_types() {
                                if ui
                                    .selectable_value(&mut self.interp_type, t, t.label())
                                    .changed()
                                {
                                    preview_changed = true;
                                }
                            }
                        });
                });

                let is_array = self.interp_type == ValueType::Array;
                let is_string_buffer = self.interp_type == ValueType::StringBuffer;

                if is_array {
                    ui.horizontal(|ui| {
                        ui.label("Element:");
                        let prev_elem = self.array_element_type;
                        egui::ComboBox::from_id_salt("interp_array_elem")
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

                if is_string_buffer {
                    ui.horizontal(|ui| {
                        ui.label("Size (bytes):");
                        if ui
                            .add(
                                egui::TextEdit::singleline(&mut self.string_buffer_size)
                                    .desired_width(60.0)
                                    .hint_text("e.g. 32"),
                            )
                            .changed()
                        {
                            preview_changed = true;
                        }
                    });
                }

                // Build the TypeRef based on selected type
                if is_array {
                    let count: usize = self.array_count.trim().parse().unwrap_or(0);
                    if count > 0 {
                        Some(TypeRef::Array(self.array_element_type, count))
                    } else {
                        None
                    }
                } else if is_string_buffer {
                    let n: usize = self.string_buffer_size.trim().parse().unwrap_or(0);
                    if n > 0 {
                        Some(TypeRef::StringBuffer(n))
                    } else {
                        None
                    }
                } else {
                    Some(TypeRef::Primitive(self.interp_type))
                }
            }
            SearchMode::Schema => {
                let schema_label = self
                    .interp_schema
                    .and_then(|i| self.schemas.get(i))
                    .map(|s| s.name.as_str())
                    .unwrap_or("— no schema —");
                let prev = self.interp_schema;
                egui::ComboBox::from_id_salt("interp_schema_select")
                    .selected_text(schema_label)
                    .width(ui.available_width() - 8.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.interp_schema, None, "— no schema —");
                        for (i, schema) in self.schemas.iter().enumerate() {
                            ui.selectable_value(&mut self.interp_schema, Some(i), &schema.name);
                        }
                    });
                if self.interp_schema != prev {
                    preview_changed = true;
                }
                self.interp_schema
                    .and_then(|i| self.schemas.get(i))
                    .map(|s| TypeRef::Schema(s.module_atom))
            }
        };

        let addr_changed = Some(addr) != self.last_interp_addr;
        if preview_changed || addr_changed {
            self.last_interp_addr = Some(addr);
            if let Some(tr) = type_ref {
                send_event(GuiEvent::CastPreviewRequested { addr, type_ref: tr });
                self.cast_preview = None;
            }
        }

        // Cast result
        if let Some(ref result) = self.cast_preview {
            ui.separator();
            match result {
                Ok(val) => {
                    let mut display = val.clone();
                    ui.add(
                        egui::TextEdit::multiline(&mut display)
                            .desired_rows(3)
                            .desired_width(f32::INFINITY)
                            .font(egui::TextStyle::Monospace)
                            .interactive(false),
                    );
                }
                Err(err) => {
                    let mut display = err.clone();
                    ui.add(
                        egui::TextEdit::singleline(&mut display)
                            .desired_width(f32::INFINITY)
                            .font(egui::TextStyle::Monospace)
                            .text_color(egui::Color32::from_rgb(220, 80, 80))
                            .interactive(false),
                    );
                }
            }
        }

        // Watch button — always enabled when there's a selection and a type chosen
        let mut watch_req = None;
        if let Some(tr) = type_ref {
            ui.separator();
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("0x{:X}", addr))
                        .monospace()
                        .weak(),
                );
                if ui.button("Watch").clicked() {
                    watch_req = Some((addr, tr));
                }
            });
        }
        watch_req
    }
}

fn primary_byte_color(annotations: &[String]) -> egui::Color32 {
    for ann in annotations {
        if ann.starts_with("heap:") {
            return egui::Color32::from_rgb(220, 140, 40);
        }
        if ann.starts_with("module:") {
            return egui::Color32::from_rgb(170, 90, 230);
        }
        if ann.starts_with("f32:") || ann.starts_with("f64:") {
            return egui::Color32::from_rgb(90, 210, 90);
        }
        if ann.contains(':') {
            return egui::Color32::from_rgb(90, 140, 230);
        }
    }
    egui::Color32::from_gray(160)
}
