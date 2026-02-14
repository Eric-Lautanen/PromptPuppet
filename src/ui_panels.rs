// ui_panels.rs
use egui::{Ui, CollapsingHeader, ComboBox, Grid, Slider, ScrollArea};
use crate::app::PromptPuppetApp;
use crate::json_loader::{OptionCategory, UiConfig, PanelConfig};

pub fn render_ui_from_config(app: &mut PromptPuppetApp, ui: &mut Ui, config: &UiConfig) -> bool {
    config.panels.clone().iter().fold(false, |ch, panel| {
        ui.add_space(2.0);
        let changed = egui::Frame::NONE.inner_margin(egui::Margin::symmetric(4, 2)).show(ui, |ui| {
            CollapsingHeader::new(egui::RichText::new(&panel.title).strong())
                .default_open(panel.default_open)
                .show(ui, |ui| {
                    ui.add_space(4.0);
                    let c = render_panel(app, ui, panel);
                    ui.add_space(4.0);
                    c
                }).body_returned.unwrap_or(false)
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
            let ckey = comp.data_source.trim_end_matches(".json");
            ui.label(&comp.label);
            ch | render_component(ui, ckey, &comp.component_type, app)
        }),
        "sequence" => if app.state.video_mode { render_sequence_panel(ui, app) } else { false },
        _ => false,
    }
}

fn render_options_panel(ui: &mut Ui, key: &str, app: &mut PromptPuppetApp) -> bool {
    let Some(lib) = app.libraries.get(key).cloned() else { return false };
    let data = app.state.options.entry(key.to_string()).or_default();
    let visible_cats: Vec<_> = lib.categories.iter().filter(|cat| cat.should_show(data)).cloned().collect();
    Grid::new(key).num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
        visible_cats.iter().fold(false, |ch, cat| {
            ui.label(format!("{}:", cat.label));
            let changed = if let Some(current) = data.get_mut(&cat.id) {
                if cat.is_text_field          { ui.text_edit_singleline(current).changed() }
                else if cat.has_search.unwrap_or(false) { render_searchable_dropdown(ui, cat, current) }
                else                          { render_dropdown(ui, cat, current) }
            } else { false };
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
        meta.settings.iter().fold(false, |ch, setting| {
            ui.label(format!("{}:", setting.label));
            let changed = match setting.setting_type.as_str() {
                "slider" => {
                    let mut num = data.values.get(&setting.id).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                    if ui.add(Slider::new(&mut num, setting.min.unwrap_or(0.0)..=setting.max.unwrap_or(100.0))).changed() {
                        data.values.insert(setting.id.clone(), serde_json::json!(num));
                        true
                    } else { false }
                }
                "dropdown" => {
                    let mut current = data.values.get(&setting.id).and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let changed = ComboBox::from_id_salt(&setting.id).selected_text(&current).show_ui(ui, |ui| {
                        setting.options.iter().fold(false, |c, opt| {
                            if ui.selectable_value(&mut current, opt.value.clone(), &opt.display).changed() {
                                data.values.insert(setting.id.clone(), serde_json::json!(current.clone()));
                                true
                            } else { c }
                        })
                    }).inner.unwrap_or(false);
                    changed
                }
                _ => false,
            };
            ui.end_row();
            ch | changed
        })
    }).inner
}

fn render_component(ui: &mut Ui, key: &str, component_type: &str, app: &mut PromptPuppetApp) -> bool {
    match component_type {
        "dropdown"            => render_simple_dropdown(ui, key, app),
        "searchable_dropdown" => render_preset_selector(ui, key, app),
        _ => false,
    }
}

fn render_simple_dropdown(ui: &mut Ui, key: &str, app: &mut PromptPuppetApp) -> bool {
    let items = match app.preset_items.get(key).cloned() {
        Some(items) if !items.is_empty() => items,
        _ => return false,
    };

    let selection = app.state.selections.entry(key.to_string()).or_default();
    let current_id = selection.selected.first().cloned().unwrap_or_default();
    let current_name = items.iter().find(|i| i.id == current_id)
        .map(|i| i.name.clone()).unwrap_or_else(|| "Select...".to_string());

    let mut new_selection = selection.selected.clone();
    let changed = ComboBox::from_id_salt(key).selected_text(&current_name).show_ui(ui, |ui| {
        items.iter().fold(false, |ch, item| {
            ch | ui.selectable_value(&mut new_selection, vec![item.id.clone()], &item.name).changed()
        })
    }).inner.unwrap_or(false);

    if changed {
        app.state.selections.entry(key.to_string()).or_default().selected = new_selection.clone();
        if let Some(id) = new_selection.first() {
            update_state_from_selection(app, id, &items);
        }
    }

    if app.preset_metadata.get(key).and_then(|m| m.allow_custom).unwrap_or(false) {
        if let Some(id) = app.state.selections.get(key).and_then(|s| s.selected.first()) {
            if items.iter().any(|i| &i.id == id && i.allow_custom) {
                ui.label("Custom:");
                return changed | ui.text_edit_multiline(
                    app.state.custom_data.entry(key.to_string()).or_default()
                ).changed();
            }
        }
    }
    changed
}

fn render_preset_selector(ui: &mut Ui, key: &str, app: &mut PromptPuppetApp) -> bool {
    let meta = app.preset_metadata.get(key).cloned();
    let has_search = meta.as_ref().and_then(|m| m.has_search).unwrap_or(false);
    let items = app.preset_items.get(key).cloned().unwrap_or_default();
    if items.is_empty() { return false; }

    let search = app.search.entry(key.to_string()).or_default();
    let mut popup_open = *app.popup_open.entry(key.to_string()).or_insert(false);

    let current_selected: Vec<String> = app.state.selections.get(key)
        .map(|s| s.selected.clone()).unwrap_or_default();
    
    let multi_mode = meta.as_ref().and_then(|m| m.multiple_selection.as_ref()).map(|s| s.as_str()).unwrap_or("never");
    let allow_multi = match multi_mode {
        "always" => true,
        "video" => app.state.video_mode,
        "image" => !app.state.video_mode,
        _ => false,
    };
    
    let selected_name = current_selected.first()
        .and_then(|id| items.iter().find(|i| &i.id == id))
        .map(|i| i.name.clone()).unwrap_or_else(|| "Select…".to_string());

    let popup_id = ui.make_persistent_id(format!("{}_popup", key));
    let mut changed = false;
    
    let button_resp = if has_search {
        ui.horizontal(|ui| {
            let btn = ui.button("🔽");
            let sr = ui.add(
                egui::TextEdit::singleline(search)
                    .hint_text(if allow_multi { "Search…" } else { &selected_name })
                    .desired_width(ui.available_width() - 60.0)
            );
            if sr.changed() && !search.is_empty() && !popup_open { popup_open = true; }
            if ui.button("✖").clicked() { search.clear(); }
            btn
        }).inner
    } else {
        ui.button("🔽 Select…")
    };

    let just_opened = button_resp.clicked() && !popup_open;
    if button_resp.clicked() { popup_open = !popup_open; }

    if allow_multi && !current_selected.is_empty() {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.spacing_mut().item_spacing.y = 4.0;
            let mut to_remove = None;
            for sel_id in &current_selected {
                if let Some(item) = items.iter().find(|i| &i.id == sel_id) {
                    let galley = ui.painter().layout_no_wrap(
                        item.name.clone(),
                        egui::FontId::proportional(ui.text_style_height(&egui::TextStyle::Small)),
                        egui::Color32::WHITE
                    );
                    let chip_width = galley.size().x + 40.0;
                    
                    ui.allocate_ui(egui::vec2(chip_width, 20.0), |ui| {
                        egui::Frame::NONE
                            .fill(ui.visuals().widgets.inactive.weak_bg_fill)
                            .inner_margin(egui::Margin::symmetric(6, 2))
                            .corner_radius(3.0)
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(&item.name).small());
                                    if ui.small_button("✖").clicked() {
                                        to_remove = Some(sel_id.clone());
                                    }
                                });
                            });
                    });
                }
            }
            if let Some(id) = to_remove {
                app.state.selections.get_mut(key).map(|s| s.selected.retain(|i| i != &id));
                changed = true;
            }
        });
    }

    let search_lower = search.to_lowercase();
    let mut ranked: Vec<_> = items.iter()
        .filter_map(|item| {
            let prompt_text = item.prompt.as_deref().unwrap_or("");
            search_rank(&item.name, prompt_text, &search_lower).map(|s| (s, item))
        }).collect();
    ranked.sort_by(|a, b| b.0.cmp(&a.0));

    let use_grid = meta.as_ref().and_then(|m| m.use_grid).unwrap_or(false);
    let mut should_close = false;
    let mut should_clear = false;

    changed = changed | if let Some(inner) = egui::Popup::new(popup_id, ui.ctx().clone(),
        egui::PopupAnchor::from(&button_resp), ui.layer_id())
        .open_memory(Some(egui::SetOpenCommand::Bool(popup_open)))
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ui.set_min_width(300.0);
            ScrollArea::vertical().max_height(340.0).auto_shrink([false, false]).show(ui, |ui| {
                if use_grid {
                    Grid::new(format!("{}_grid", key)).num_columns(3).spacing([4.0, 4.0]).show(ui, |ui| {
                        let mut ch = false;
                        for (i, (_, item)) in ranked.iter().enumerate() {
                            if i > 0 && i % 3 == 0 { ui.end_row(); }
                            let is_selected = current_selected.contains(&item.id);
                        let resp = ui.vertical(|ui| {
                                let r = ui.selectable_label(is_selected,
                                    egui::RichText::new(&item.name).strong());
                                if let Some(prompt) = &item.prompt {
                                    if !prompt.is_empty() {
                                        ui.label(egui::RichText::new(prompt).small()
                                            .color(ui.visuals().weak_text_color()));
                                    }
                                }
                                r
                            });
                            if just_opened && is_selected {
                                resp.inner.scroll_to_me(Some(egui::Align::Center));
                            }
                            if resp.inner.clicked() {
                                ch = handle_selection(app, key, &item.id, &items, meta.as_ref());
                                should_close = true;
                                if !allow_multi { should_clear = true; }
                            }
                        }
                        ch
                    }).inner
                } else {
                    ranked.iter().fold(false, |ch, (_, item)| {
                        let is_selected = current_selected.contains(&item.id);
                        let resp = ui.vertical(|ui| {
                            let r = ui.selectable_label(is_selected,
                                egui::RichText::new(&item.name).strong());
                            if let Some(prompt) = &item.prompt {
                                if !prompt.is_empty() {
                                    ui.label(egui::RichText::new(prompt).small()
                                        .color(ui.visuals().weak_text_color()));
                                }
                            }
                            r
                        });
                        if just_opened && is_selected {
                            resp.inner.scroll_to_me(Some(egui::Align::Center));
                        }
                        if resp.inner.clicked() {
                            let c = handle_selection(app, key, &item.id, &items, meta.as_ref());
                            should_close = true;
                            if !allow_multi { should_clear = true; }
                            c
                        } else { ui.separator(); ch }
                    })
                }
            }).inner
        }) {
        if inner.response.should_close() || should_close {
            egui::Popup::close_id(ui.ctx(), popup_id);
            popup_open = false;
        }
        inner.inner
    } else {
        popup_open = false;
        false
    };

    if should_clear { app.search.get_mut(key).map(|s| s.clear()); }
    app.popup_open.insert(key.to_string(), popup_open);
    changed
}

fn handle_selection(app: &mut PromptPuppetApp, key: &str, id: &str,
    items: &[crate::app::PresetItem], meta: Option<&crate::app::PresetMetadata>) -> bool
{
    let multi_mode = meta.and_then(|m| m.multiple_selection.as_ref()).map(|s| s.as_str()).unwrap_or("never");
    let allow_multi = match multi_mode {
        "always" => true,
        "video" => app.state.video_mode,
        "image" => !app.state.video_mode,
        _ => false,
    };
    
    let selection = app.state.selections.entry(key.to_string()).or_default();
    if allow_multi {
        if selection.selected.contains(&id.to_string()) {
            selection.selected.retain(|i| i != id);
        } else {
            selection.selected.push(id.to_string());
        }
        true
    } else {
        selection.selected = vec![id.to_string()];
        update_state_from_selection(app, id, items);
        if let Some(item) = items.iter().find(|i| i.id == id) {
            app.set_status(&format!("✅ {}", item.name), 2.0);
        }
        true
    }
}

fn update_state_from_selection(app: &mut PromptPuppetApp, id: &str, items: &[crate::app::PresetItem]) {
    if let Some(pose) = items.iter().find(|i| i.id == id).and_then(|i| i.pose_data.clone()) {
        app.state.pose = pose;
    }
}

fn search_rank(name: &str, prompt: &str, query: &str) -> Option<u8> {
    if query.is_empty() { return Some(255); }
    let n = name.to_lowercase();
    let p = prompt.to_lowercase();
    if n.starts_with(query)      { Some(3) }
    else if n.contains(query)    { Some(2) }
    else if p.contains(query)    { Some(1) }
    else { None }
}

pub fn render_sequence_panel(ui: &mut Ui, app: &mut PromptPuppetApp) -> bool {
    let mut changed = app.state.selections.clone().iter().fold(false, |mut changed, (key, selection)| {
        if !selection.sequence.is_empty() {
            ui.label(format!("{} Sequence:", key));
            let mut remove = None;
            for (i, id) in selection.sequence.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(id);
                    if ui.button("❌").clicked() { remove = Some(i); }
                });
            }
            if let Some(i) = remove {
                app.state.selections.get_mut(key).unwrap().sequence.remove(i);
                changed = true;
            }
            ui.add_space(8.0);
        }
        changed
    });
    if ui.button("Clear All Sequences").clicked() {
        for sel in app.state.selections.values_mut() { sel.sequence.clear(); }
        changed = true;
    }
    changed
}