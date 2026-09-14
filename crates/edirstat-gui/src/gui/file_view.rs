use eframe::egui;
use fluent_zero::t;
use prettier_bytes::ByteFormatter;

use super::GuiApp;
use crate::arena::FileArenaSnapshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExplorerTab {
    Tree,
    Files,
}

impl GuiApp {
    pub(crate) fn draw_explorer_tabs(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.selectable_value(
                &mut self.explorer_tab,
                ExplorerTab::Tree,
                t!("explorer-tab-tree"),
            );
            ui.selectable_value(
                &mut self.explorer_tab,
                ExplorerTab::Files,
                t!("explorer-tab-files"),
            );
        });
    }

    pub(crate) fn ensure_file_view_cache(&mut self, snapshot: &FileArenaSnapshot) {
        self.query_coordinator.update(
            snapshot,
            &self.search_query,
            self.filter_case_sensitive,
            self.filter_regex,
            self.highlight_duplicates,
            &self.selected_duplicates,
        );

        let ptr = std::sync::Arc::as_ptr(&snapshot.nodes) as usize;
        if self.file_view_snapshot_ptr == ptr
            && self.file_view_query == self.search_query
            && self.file_view_filter_case == self.filter_case_sensitive
            && self.file_view_filter_regex == self.filter_regex
        {
            return;
        }
        self.file_view_snapshot_ptr = ptr;
        self.file_view_query.clone_from(&self.search_query);
        self.file_view_filter_case = self.filter_case_sensitive;
        self.file_view_filter_regex = self.filter_regex;

        let filtering = !self.search_query.is_empty();
        self.file_view_indices = snapshot
            .nodes
            .iter()
            .enumerate()
            .filter(|(idx, node)| {
                !node.is_directory()
                    && (!filtering || self.query_coordinator.cached_node_matches.contains(*idx))
            })
            .map(|(idx, _)| idx as u32)
            .collect();
        self.file_view_indices.sort_by(|&a, &b| {
            snapshot.nodes[b as usize]
                .allocated
                .cmp(&snapshot.nodes[a as usize].allocated)
                .then_with(|| {
                    snapshot.nodes[b as usize]
                        .size
                        .cmp(&snapshot.nodes[a as usize].size)
                })
        });
    }

    pub(crate) fn render_file_view(&mut self, ui: &mut egui::Ui, snapshot: &FileArenaSnapshot) {
        self.ensure_file_view_cache(snapshot);
        if self.file_view_indices.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label(t!("file-view-empty"));
            });
            return;
        }

        ui.columns(5, |cols| {
            cols[0].strong(t!("explorer-hdr-name"));
            cols[1].strong(t!("explorer-hdr-size"));
            cols[2].strong(t!("explorer-hdr-allocated"));
            cols[3].strong(t!("explorer-hdr-percentage"));
            cols[4].strong(t!("explorer-hdr-modified"));
        });
        ui.separator();

        let row_height = 22.0;
        let indices = self.file_view_indices.clone();
        let selected_rows = self.table_state.selected_rows.clone();
        let time_format = self.time_format.clone();
        let mut clicked: Option<u32> = None;

        egui::ScrollArea::both()
            .id_salt("file_view_scroll")
            .auto_shrink([false, false])
            .show_rows(ui, row_height, indices.len(), |ui, row_range| {
                for row in row_range {
                    let idx = indices[row];
                    let node = &snapshot.nodes[idx as usize];
                    let full_path_raw = snapshot.get_full_path(idx);
                    let name = crate::arena::clean_unc_path(&full_path_raw);
                    let parent_metric = node.parent_opt().map_or_else(
                        || node.allocated.max(node.size).max(1),
                        |p| {
                            let parent = &snapshot.nodes[p as usize];
                            if parent.allocated == 0 {
                                parent.size.max(1)
                            } else {
                                parent.allocated
                            }
                        },
                    );
                    let child_metric = if node.allocated == 0 {
                        node.size
                    } else {
                        node.allocated
                    };
                    #[allow(clippy::cast_precision_loss)]
                    let pct = (child_metric as f32 / parent_metric as f32).clamp(0.0, 1.0);
                    let selected = selected_rows.contains(idx);
                    let inner = ui.horizontal(|ui| {
                        ui.set_min_height(row_height);
                        ui.set_max_height(row_height);
                        if selected {
                            ui.painter().rect_filled(
                                ui.max_rect(),
                                0.0,
                                ui.visuals().selection.bg_fill,
                            );
                        }
                        ui.columns(5, |cols| {
                            let mut text = egui::RichText::new(name.as_ref());
                            if selected {
                                text = text.strong();
                            }
                            cols[0].label(text);
                            cols[1].label(ByteFormatter::new().format(node.size).to_string());
                            cols[2].label(ByteFormatter::new().format(node.allocated).to_string());
                            cols[3].label(format!("{:.1}%", pct * 100.0));
                            cols[4].label(crate::time_utils::format_epoch(
                                node.modified_timestamp,
                                &time_format,
                            ));
                        });
                    });
                    if inner.response.interact(egui::Sense::click()).clicked() {
                        clicked = Some(idx);
                    }
                }
            });

        if let Some(idx) = clicked {
            self.table_state.selected_rows.clear();
            self.table_state.selected_rows.insert(idx);
            self.focus_node_idx = Some(idx);
            self.scroll_to_selected = true;
            let mut curr = Some(idx);
            while let Some(c) = curr {
                if let Some(parent) = snapshot.nodes[c as usize].parent_opt() {
                    self.table_state.expanded_rows.insert(parent);
                    curr = Some(parent);
                } else {
                    break;
                }
            }
        }
    }
}
