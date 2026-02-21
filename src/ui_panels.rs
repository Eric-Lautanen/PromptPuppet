// ui_panels.rs
use egui::{Ui, CollapsingHeader, ComboBox, Grid, Slider, ScrollArea};
use crate::app::{PresetItem, PresetMetadata, PromptPuppetApp};
use crate::json_loader::{OptionCategory, UiConfig, PanelConfig};

pub fn render_ui_from_config(app: &mut PromptPuppetApp, ui: &mut Ui, config: &UiConfig) -> bool {
    config.panels.clone().iter().fold(false, |ch, panel| {
        ui.add_space(2.0);
        let changed = egui::Frame::NONE.inner_margin(egui::Margin::symmetric(4, 2)).show(ui, |ui| {
            CollapsingHeader::new(egui::RichText::new(&panel.title).strong())
                .default_open(panel.default_open)
                .show(ui, |ui| { ui.add_space(4.0); let c = render_panel(app, ui, panel); ui.add_space(4.0); c })
                .body_returned.unwrap_or(false)
        }).inner;
        ui.separator();
        ch | changed
    })
}

fn render_panel(app: &mut PromptPuppetApp, ui: &mut Ui, panel: &PanelConfig) -> bool {
    let key = panel.data_source.trim_end_matches(".json");
    match panel.panel_type.as_str() {
        "options_grid"    => render_options_panel(ui, key, app),
        "controls"        => render_settings_panel(ui, key, app),
        "preset_selector" => render_preset_selector(ui, key, app),
        "composite"       => panel.components.iter().fold(false, |ch, comp| {
            ui.label(&comp.label);
            ch | render_component(ui, comp.data_source.trim_end_matches(".json"), &comp.component_type, app)
        }),
        "sequence" => if app.state.video_mode { render_sequence_panel(ui, app) } else { false },
        _ => false,
    }
}

fn render_options_panel(ui: &mut Ui, key: &str, app: &mut PromptPuppetApp) -> bool {
    let Some(lib) = app.libraries.get(key).cloned() else { return false };
    let data = app.state.options.entry(key.to_string()).or_default();
    let cats: Vec<_> = lib.categories.iter().filter(|c| c.should_show(data)).cloned().collect();
    Grid::new(key).num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
        cats.iter().fold(false, |ch, cat| {
            ui.label(format!("{}:", cat.label));
            let changed = data.get_mut(&cat.id).map_or(false, |cur| {
                if cat.is_text_field                    { ui.text_edit_singleline(cur).changed() }
                else if cat.has_search.unwrap_or(false) { render_searchable_dropdown(ui, cat, cur) }
                else                                    { render_dropdown(ui, cat, cur) }
            });
            ui.end_row();
            ch | changed
        })
    }).inner
}

fn render_dropdown(ui: &mut Ui, cat: &OptionCategory, current: &mut String) -> bool {
    ComboBox::from_id_salt(&cat.id).selected_text(cat.get_display_text(current)).show_ui(ui, |ui| {
        cat.options.iter().fold(false, |ch, opt| {
            ch | ui.selectable_value(current, opt.value.clone(), &opt.display).changed()
        }) | (cat.allow_custom && ui.selectable_value(current, current.clone(), "Custom...").changed())
    }).inner.unwrap_or(false)
}

fn render_searchable_dropdown(ui: &mut Ui, cat: &OptionCategory, current: &mut String) -> bool {
    let popup_id = ui.make_persistent_id(format!("{}_popup", cat.id));
    let btn = ui.button(cat.get_display_text(current));
    if btn.clicked() { egui::Popup::toggle_id(ui.ctx(), popup_id); }
    egui::Popup::new(popup_id, ui.ctx().clone(), egui::PopupAnchor::from(&btn), ui.layer_id())
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                cat.options.iter().fold(false, |ch, opt| {
                    if ui.selectable_label(*current == opt.value, &opt.display).clicked() {
                        *current = opt.value.clone();
                        egui::Popup::close_id(ui.ctx(), popup_id);
                        true
                    } else { ch }
                })
            }).inner
        }).map(|r| r.inner).unwrap_or(false)
}

fn render_settings_panel(ui: &mut Ui, key: &str, app: &mut PromptPuppetApp) -> bool {
    let Some(meta) = app.settings_meta.get(key).cloned() else { return false };
    let data = app.state.settings.entry(key.to_string()).or_default();
    Grid::new(key).num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
        meta.settings.iter().fold(false, |ch, s| {
            ui.label(format!("{}:", s.label));
            let changed = match s.setting_type.as_str() {
                "slider" => {
                    let mut n = data.values.get(&s.id).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                    let c = ui.add(Slider::new(&mut n, s.min.unwrap_or(0.0)..=s.max.unwrap_or(100.0))).changed();
                    if c { data.values.insert(s.id.clone(), serde_json::json!(n)); }
                    c
                }
                "dropdown" => {
                    let mut cur = data.values.get(&s.id).and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let c = ComboBox::from_id_salt(&s.id).selected_text(&cur).show_ui(ui, |ui| {
                        s.options.iter().fold(false, |c, opt| {
                            let hit = ui.selectable_value(&mut cur, opt.value.clone(), &opt.display).changed();
                            if hit { data.values.insert(s.id.clone(), serde_json::json!(cur.clone())); }
                            c | hit
                        })
                    }).inner.unwrap_or(false);
                    c
                }
                _ => false,
            };
            ui.end_row();
            ch | changed
        })
    }).inner
}

fn render_component(ui: &mut Ui, key: &str, kind: &str, app: &mut PromptPuppetApp) -> bool {
    match kind {
        "dropdown"            => render_simple_dropdown(ui, key, app),
        "searchable_dropdown" => render_preset_selector(ui, key, app),
        _ => false,
    }
}

fn render_simple_dropdown(ui: &mut Ui, key: &str, app: &mut PromptPuppetApp) -> bool {
    let items = match app.preset_items.get(key).cloned() {
        Some(v) if !v.is_empty() => v,
        _ => return false,
    };
    let sel   = app.state.selections.entry(key.to_string()).or_default();
    let cur   = sel.selected.first().cloned().unwrap_or_default();
    let label = items.iter().find(|i| i.id == cur).map(|i| i.name.clone()).unwrap_or_else(|| "Select...".into());
    let mut nxt = sel.selected.clone();

    let changed = ComboBox::from_id_salt(key).selected_text(&label).show_ui(ui, |ui| {
        items.iter().fold(false, |ch, item| {
            ch | ui.selectable_value(&mut nxt, vec![item.id.clone()], &item.name).changed()
        })
    }).inner.unwrap_or(false);

    if changed {
        app.state.selections.entry(key.to_string()).or_default().selected = nxt.clone();
        if let Some(id) = nxt.first() { update_pose(app, id, &items); }
    }
    if app.preset_metadata.get(key).and_then(|m| m.allow_custom).unwrap_or(false) {
        if let Some(id) = app.state.selections.get(key).and_then(|s| s.selected.first()) {
            if items.iter().any(|i| &i.id == id && i.allow_custom) {
                ui.label("Custom:");
                return changed | ui.text_edit_multiline(
                    app.state.custom_data.entry(key.to_string()).or_default()).changed();
            }
        }
    }
    changed
}

// ── Preset selector ───────────────────────────────────────────────────────────

fn render_preset_selector(ui: &mut Ui, key: &str, app: &mut PromptPuppetApp) -> bool {
    let meta  = app.preset_metadata.get(key).cloned();
    let items = app.preset_items.get(key).cloned().unwrap_or_default();
    if items.is_empty() { return false; }

    let allow_multi  = meta.as_ref().map_or(false, |m| m.allow_multi(app.state.video_mode));
    let has_search   = meta.as_ref().and_then(|m| m.has_search).unwrap_or(false);
    let use_grid     = meta.as_ref().and_then(|m| m.use_grid).unwrap_or(false);
    let selected     = app.state.selections.get(key).map(|s| s.selected.clone()).unwrap_or_default();
    let sel_name     = selected.first()
        .and_then(|id| items.iter().find(|i| &i.id == id))
        .map(|i| i.name.clone()).unwrap_or_else(|| "Select…".into());

    let popup_id     = ui.make_persistent_id(format!("{}_popup", key));
    let mut popup_open = *app.popup_open.entry(key.to_string()).or_insert(false);
    let mut changed  = false;

    // ── Header row (search bar or plain button) ───────────────────────────────
    let btn = if has_search {
        ui.horizontal(|ui| {
            let b = ui.button("🔽");
            let search = app.search.entry(key.to_string()).or_default();
            let sr = ui.add(egui::TextEdit::singleline(search)
                .hint_text(if allow_multi { "Search…" } else { &sel_name })
                .desired_width(ui.available_width() - 60.0));
            if sr.changed() && !search.is_empty() && !popup_open {
                popup_open = true;
                *app.popup_open.get_mut(key).unwrap() = true;
            }
            if ui.button("✖").clicked() {
                app.search.entry(key.to_string()).or_default().clear();
                popup_open = false;
                *app.popup_open.get_mut(key).unwrap() = false;
            }
            b
        }).inner
    } else {
        ui.button("🔽 Select…")
    };

    let just_opened = btn.clicked() && !popup_open;
    if btn.clicked() {
        popup_open = !popup_open;
        *app.popup_open.get_mut(key).unwrap() = popup_open;
    }

    // ── Multi-select chips ────────────────────────────────────────────────────
    if allow_multi && !selected.is_empty() {
        let mut to_remove: Option<String> = None;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
            for sel_id in &selected {
                let Some(item) = items.iter().find(|i| &i.id == sel_id) else { continue };
                let chip_w = ui.painter().layout_no_wrap(item.name.clone(),
                    egui::FontId::proportional(ui.text_style_height(&egui::TextStyle::Small)),
                    egui::Color32::WHITE).size().x + 40.0;
                ui.allocate_ui(egui::vec2(chip_w, 20.0), |ui| {
                    egui::Frame::NONE
                        .fill(ui.visuals().widgets.inactive.weak_bg_fill)
                        .inner_margin(egui::Margin::symmetric(6, 2))
                        .corner_radius(3.0)
                        .show(ui, |ui| { ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(&item.name).small());
                            if ui.small_button("✖").clicked() { to_remove = Some(sel_id.clone()); }
                        }); });
                });
            }
        });
        if let Some(id) = to_remove {
            app.state.selections.get_mut(key).map(|s| s.selected.retain(|i| i != &id));
            changed = true;
        }
    }

    // ── Ranked items ──────────────────────────────────────────────────────────
    let query = app.search.get(key).map(|s| s.to_lowercase()).unwrap_or_default();
    let mut ranked: Vec<_> = items.iter()
        .filter_map(|item| search_rank(&item.name, item.prompt.as_deref().unwrap_or(""), &query).map(|s| (s, item)))
        .collect();
    ranked.sort_by(|a, b| b.0.cmp(&a.0));

    // ── Popup ─────────────────────────────────────────────────────────────────
    let mut should_close = false;
    let mut should_clear = false;

    changed |= egui::Popup::new(popup_id, ui.ctx().clone(), egui::PopupAnchor::from(&btn), ui.layer_id())
        .open_memory(Some(egui::SetOpenCommand::Bool(popup_open)))
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ui.set_min_width(300.0);
            ScrollArea::vertical().max_height(340.0).auto_shrink([false, false]).show(ui, |ui| {
                if use_grid {
                    Grid::new(format!("{}_grid", key)).num_columns(3).spacing([4.0, 4.0]).show(ui, |ui| {
                        ranked.iter().enumerate().fold(false, |ch, (i, (_, item))| {
                            if i > 0 && i % 3 == 0 { ui.end_row(); }
                            let clicked = render_item(ui, item, selected.contains(&item.id), just_opened);
                            if clicked { should_close = true; if !allow_multi { should_clear = true; } }
                            ch | (clicked && handle_selection(app, key, &item.id, &items, meta.as_ref()))
                        })
                    }).inner
                } else {
                    ranked.iter().fold(false, |ch, (_, item)| {
                        let clicked = render_item(ui, item, selected.contains(&item.id), just_opened);
                        if clicked { should_close = true; if !allow_multi { should_clear = true; } }
                        ui.separator();
                        ch | (clicked && handle_selection(app, key, &item.id, &items, meta.as_ref()))
                    })
                }
            }).inner
        })
        .map(|r| { if r.response.should_close() || should_close { egui::Popup::close_id(ui.ctx(), popup_id); popup_open = false; } r.inner })
        .unwrap_or_else(|| { popup_open = false; false });

    if should_clear { app.search.entry(key.to_string()).or_default().clear(); }
    app.popup_open.insert(key.to_string(), popup_open);
    changed
}

/// Render one item row (shared between grid and list). Returns true if clicked.
fn render_item(ui: &mut Ui, item: &PresetItem, is_selected: bool, just_opened: bool) -> bool {
    let resp = ui.vertical(|ui| {
        let r = ui.selectable_label(is_selected, egui::RichText::new(&item.name).strong());
        if let Some(p) = &item.prompt {
            if !p.is_empty() { ui.label(egui::RichText::new(p).small().color(ui.visuals().weak_text_color())); }
        }
        r
    });
    if just_opened && is_selected { resp.inner.scroll_to_me(Some(egui::Align::Center)); }
    resp.inner.clicked()
}

fn handle_selection(app: &mut PromptPuppetApp, key: &str, id: &str,
    items: &[PresetItem], meta: Option<&PresetMetadata>) -> bool
{
    let allow_multi = meta.map_or(false, |m| m.allow_multi(app.state.video_mode));
    let sel = app.state.selections.entry(key.to_string()).or_default();
    if allow_multi {
        if sel.selected.contains(&id.to_string()) { sel.selected.retain(|i| i != id); }
        else                                       { sel.selected.push(id.to_string()); }
    } else {
        sel.selected = vec![id.to_string()];
        update_pose(app, id, items);
        if let Some(item) = items.iter().find(|i| i.id == id) {
            app.set_status(&format!("✅ {}", item.name), 2.0);
        }
    }
    true
}

fn update_pose(app: &mut PromptPuppetApp, id: &str, items: &[PresetItem]) {
    if let Some(pose) = items.iter().find(|i| i.id == id).and_then(|i| i.pose_data.clone()) {
        app.state.pose = pose;
        app.pose_is_manual = false;
    }
}

fn search_rank(name: &str, prompt: &str, query: &str) -> Option<u8> {
    if query.is_empty() { return Some(255); }
    let n = name.to_lowercase();
    if n.starts_with(query)           { Some(3) }
    else if n.contains(query)         { Some(2) }
    else if prompt.to_lowercase().contains(query) { Some(1) }
    else                              { None }
}

pub fn render_sequence_panel(ui: &mut Ui, app: &mut PromptPuppetApp) -> bool {
    let keys: Vec<_> = app.state.selections.keys().cloned().collect();
    let mut changed = keys.iter().fold(false, |ch, key| {
        let Some(seq) = app.state.selections.get(key).filter(|s| !s.sequence.is_empty()).map(|s| s.sequence.clone()) else { return ch };
        ui.label(format!("{} Sequence:", key));
        let remove = seq.iter().enumerate().find_map(|(i, id)| {
            let mut r = None;
            ui.horizontal(|ui| { ui.label(id); if ui.button("❌").clicked() { r = Some(i); } });
            r
        });
        if let Some(i) = remove { app.state.selections.get_mut(key).unwrap().sequence.remove(i); return true; }
        ui.add_space(8.0);
        ch
    });
    if ui.button("Clear All Sequences").clicked() {
        for s in app.state.selections.values_mut() { s.sequence.clear(); }
        changed = true;
    }
    changed
}