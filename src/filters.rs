use crate::config::FilterConfig;
use crate::db::{Db, SqlOperator, SqlOrder};
use crate::dropdown::DropDownBox;
use crate::metadata::{METADATA_DATE, METADATA_DIRECTORY};
use crate::worker::Worker;
use eframe::egui;
use eframe::egui::{Align, Id, Layout};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use uuid::Uuid;

pub struct Filters {
    filter_fields: Vec<FilterField>,
    order_field: OrderField,
    imgs_in_db: u32,
    imgs_in_db_job: Option<JoinHandle<Option<u32>>>,
    last_query_count: Option<u32>,
    query_handle: Option<JoinHandle<Option<Vec<PathBuf>>>>,
    unique_exif_tags: Vec<String>,
    unique_exif_tags_job: Option<JoinHandle<Option<Vec<String>>>>,
    worker: Arc<Worker>,
}

pub struct FilterField {
    id: Id,
    name: String,
    value: String,
    operator: SqlOperator,
    default_values: Vec<String>,
    default_values_job: Option<JoinHandle<Option<Vec<String>>>>,
}

impl FilterField {
    pub fn new(name: &str, default_value: &str) -> FilterField {
        let mut ff = FilterField {
            id: Id::new(Uuid::new_v4()),
            name: name.to_string(),
            value: String::from(default_value),
            operator: SqlOperator::Like,
            default_values: vec![],
            default_values_job: None,
        };

        let name = ff.name.to_string();
        ff.default_values_job = Some(thread::spawn(move || {
            Db::get_distinct_values_for_exif_tag(&name).ok()
        }));

        ff
    }

    pub fn get_default_values(&mut self) -> Vec<String> {
        if self.default_values_job.is_some() {
            let qh = self.default_values_job.take().unwrap();
            if qh.is_finished() {
                if let Ok(Some(values)) = qh.join() {
                    self.default_values = values;
                }
            } else {
                self.default_values_job = Some(qh);
            }
        }

        self.default_values.clone()
    }
}

pub struct OrderField {
    tag: String,
    order: SqlOrder,
}

impl Filters {
    pub fn new(filter_config: FilterConfig, opened_path: &str, worker: Arc<Worker>) -> Filters {
        let mut ffs: Vec<FilterField> = filter_config
            .exif_tags
            .iter()
            .map(|x| FilterField::new(&x.name, ""))
            .collect();
        ffs.push(FilterField::new(METADATA_DIRECTORY, opened_path));
        Filters {
            filter_fields: ffs,
            order_field: OrderField {
                tag: String::from(METADATA_DATE),
                order: SqlOrder::Desc,
            },
            imgs_in_db: 0,
            imgs_in_db_job: Some(thread::spawn(move || Db::get_img_count().ok())),
            unique_exif_tags_job: Some(thread::spawn(move || Db::get_unique_exif_tags().ok())),
            unique_exif_tags: vec![],
            last_query_count: None,
            query_handle: None,
            worker,
        }
    }

    pub fn set_metadata_directory_value(&mut self, path: &Path) {
        if let Some(f) = self
            .filter_fields
            .iter_mut()
            .find(|x| x.name == METADATA_DIRECTORY)
        {
            f.value = path.to_string_lossy().to_string();
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) -> Option<Vec<PathBuf>> {
        let mut return_paths: Option<Vec<PathBuf>> = None;

        self.finish_imgs_in_db_job();
        self.finish_unique_filter_tags_job();

        ui.vertical(|ui| {
            ui.add_space(5.);
            ui.heading("Filter & Order");
            ui.add_space(10.);

            ui.strong("Filter");

            for field in &mut self.filter_fields {
                ui.horizontal(|ui| {
                    let default_values = field.get_default_values();
                    let desired_first_dd_width = ui.available_width() / 2. - 15.;
                    if ui
                        .add(
                            DropDownBox::from_iter(
                                &self.unique_exif_tags,
                                format!("{}_tag", &field.id.value()),
                                &mut field.name,
                                |ui, text| ui.selectable_label(false, text),
                            )
                            .max_height(600.)
                            .desired_width(desired_first_dd_width)
                            .filter_by_input(true)
                            .select_on_focus(true),
                        )
                        .changed()
                        && self.unique_exif_tags.contains(&field.name)
                    {
                        let name = field.name.clone();
                        field.default_values_job = Some(thread::spawn(move || {
                            Db::get_distinct_values_for_exif_tag(&name).ok()
                        }));
                    }

                    egui::ComboBox::from_id_salt(format!("{}_operator", &field.id.value()))
                        .width(15.)
                        .selected_text(field.operator.to_string())
                        .show_ui(ui, |ui| {
                            for op in SqlOperator::list() {
                                ui.selectable_value(
                                    &mut field.operator,
                                    op.clone(),
                                    op.to_string(),
                                );
                            }
                        });

                    ui.add(
                        DropDownBox::from_iter(
                            &default_values,
                            format!("{}_value", &field.id.value()),
                            &mut field.value,
                            |ui, text| ui.selectable_label(false, text),
                        )
                        .max_height(600.)
                        .desired_width(f32::INFINITY)
                        .filter_by_input(true)
                        .select_on_focus(true),
                    );
                });
            }

            ui.add_space(5.);

            ui.horizontal(|ui| {
                if ui.button("+").clicked() {
                    self.filter_fields.push(FilterField::new("", ""));
                }

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("Clear").clicked() {
                        self.filter_fields
                            .iter_mut()
                            .for_each(|f| f.value = String::new());
                    }
                });
            });

            ui.add_space(10.);
            ui.strong("Order");
            ui.horizontal(|ui| {
                ui.add(
                    DropDownBox::from_iter(
                        &self.unique_exif_tags,
                        "order_tag",
                        &mut self.order_field.tag,
                        |ui, text| ui.selectable_label(false, text),
                    )
                    .max_height(600.)
                    .filter_by_input(true)
                    .select_on_focus(true),
                );

                egui::ComboBox::from_id_salt("{}_order_direction")
                    .width(40.)
                    .selected_text(self.order_field.order.to_string())
                    .show_ui(ui, |ui| {
                        for op in SqlOrder::list() {
                            ui.selectable_value(
                                &mut self.order_field.order,
                                op.clone(),
                                op.to_string(),
                            );
                        }
                    });
            });

            ui.add_space(10.);
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                    if ui.button("Filter").clicked() {
                        let fields: Vec<(String, String, SqlOperator)> = self
                            .filter_fields
                            .iter()
                            .filter(|x| !x.value.is_empty() && !x.name.is_empty())
                            .map(|x| (x.name.clone(), x.value.clone(), x.operator.clone()))
                            .collect();
                        if !fields.is_empty() {
                            let order_tag = self.order_field.tag.clone();
                            let order_direction = self.order_field.order.clone();
                            let worker = self.worker.clone();
                            self.query_handle = Some(thread::spawn(move || {
                                let paths = Db::get_paths_filtered_by_metadata(
                                    &fields,
                                    &order_tag,
                                    &order_direction,
                                )
                                .ok();

                                if let Some(paths) = paths.clone() {
                                    worker.send_job(crate::worker::Job::ClearMovedFiles(
                                        paths.clone(),
                                    ));
                                }

                                paths
                            }));
                        }
                    }

                    if self.query_handle.is_some() {
                        let qh = self.query_handle.take().unwrap();
                        if qh.is_finished() {
                            if let Ok(Some(paths)) = qh.join() {
                                self.last_query_count = Some(paths.len() as u32);
                                return_paths = Some(paths.clone());
                            }
                        } else {
                            self.query_handle = Some(qh);
                            ui.spinner();
                        }
                    }
                });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                    if let Some(last_query_count) = self.last_query_count {
                        ui.label(format!("{} / {}", last_query_count, self.imgs_in_db));
                    } else {
                        ui.label(format!("{} Imgs", self.imgs_in_db));
                    }
                });
            });
        });

        return_paths
    }

    pub fn finish_imgs_in_db_job(&mut self) {
        if self.imgs_in_db_job.is_some() {
            let qh = self.imgs_in_db_job.take().unwrap();
            if qh.is_finished() {
                if let Ok(Some(values)) = qh.join() {
                    self.imgs_in_db = values;
                }
            } else {
                self.imgs_in_db_job = Some(qh);
            }
        }
    }

    pub fn finish_unique_filter_tags_job(&mut self) {
        if self.unique_exif_tags_job.is_some() {
            let qh = self.unique_exif_tags_job.take().unwrap();
            if qh.is_finished() {
                if let Ok(Some(values)) = qh.join() {
                    self.unique_exif_tags = values;
                }
            } else {
                self.unique_exif_tags_job = Some(qh);
            }
        }
    }
}
