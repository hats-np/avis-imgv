use crate::app::AppState;
use crate::components::dropdown::DropDownBox;
use crate::config::FilterConfig;
use crate::db::{DbRepository, SqlOperator, SqlOrder};
use crate::metadata::{METADATA_DATE, METADATA_DIRECTORY, Metadata, XMPRating};
use crate::utils::get_path_string_without_trailing_slash;
use crate::worker::Worker;
use eframe::egui::{self};
use eframe::egui::{Align, Id, Layout};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use uuid::Uuid;

pub struct Filters {
    filter_fields: Vec<FilterField>,
    order_field: OrderField,
    rating_field: RatingField,
    tag_field: Vec<String>,
    imgs_in_db: u32,
    imgs_in_db_job: Option<JoinHandle<Option<u32>>>,
    last_query_count: Option<u32>,
    query_handle: Option<JoinHandle<Option<Vec<PathBuf>>>>,
    unique_exif_tags: Vec<String>,
    unique_exif_tags_job: Option<JoinHandle<Option<Vec<String>>>>,
    unique_xmp_tags: Vec<String>,
    unique_xmp_tags_job: Option<JoinHandle<Option<Vec<String>>>>,
    worker: Arc<Mutex<Worker>>,
    group_raw_jpeg: bool,
    db_repo: DbRepository,
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
    pub fn new(name: &str, default_value: &str, db_repo: &DbRepository) -> FilterField {
        let mut ff = FilterField {
            id: Id::new(Uuid::new_v4()),
            name: name.to_string(),
            value: String::from(default_value),
            operator: SqlOperator::Like,
            default_values: vec![String::new()],
            default_values_job: None,
        };

        let name = ff.name.to_string();
        let mut repo = db_repo.clone();
        ff.default_values_job = Some(thread::spawn(move || {
            repo.get_distinct_values_for_exif_tag(&name).ok()
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

pub struct RatingField {
    first_operator: SqlOperator,
    first_rating: String,
    second_operator: SqlOperator,
    second_rating: String,
}

impl Filters {
    pub fn new(
        filter_config: FilterConfig,
        opened_path: &Path,
        worker: Arc<Mutex<Worker>>,
        db_repo: &DbRepository,
    ) -> Filters {
        let mut job_repo = db_repo.clone();
        let imgs_in_db_job = Some(thread::spawn(move || job_repo.get_img_count().ok()));

        job_repo = db_repo.clone();
        let unique_exif_tags_job =
            Some(thread::spawn(move || job_repo.get_unique_exif_tags().ok()));

        job_repo = db_repo.clone();
        let unique_xmp_tags_job = Some(thread::spawn(move || job_repo.get_unique_xmp_tags().ok()));

        let mut ffs: Vec<FilterField> = filter_config
            .exif_tags
            .iter()
            .map(|x| FilterField::new(&x.name, "", db_repo))
            .collect();
        ffs.push(FilterField::new(
            METADATA_DIRECTORY,
            &get_path_string_without_trailing_slash(opened_path),
            db_repo,
        ));

        Filters {
            filter_fields: ffs,
            order_field: OrderField {
                tag: String::from(METADATA_DATE),
                order: SqlOrder::Asc,
            },
            rating_field: RatingField {
                first_operator: SqlOperator::None,
                first_rating: String::new(),
                second_operator: SqlOperator::None,
                second_rating: String::new(),
            },
            tag_field: vec![],
            imgs_in_db: 0,
            imgs_in_db_job,
            unique_exif_tags_job,
            unique_exif_tags: vec![],
            unique_xmp_tags_job,
            unique_xmp_tags: vec![],
            last_query_count: None,
            query_handle: None,
            worker,
            group_raw_jpeg: true,
            db_repo: db_repo.clone(),
        }
    }

    pub fn set_metadata_directory_value(&mut self, path: &Path) {
        if let Some(f) = self
            .filter_fields
            .iter_mut()
            .find(|x| x.name == METADATA_DIRECTORY)
        {
            f.value = get_path_string_without_trailing_slash(path);
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, state: &mut AppState) -> Option<Vec<PathBuf>> {
        let mut return_paths: Option<Vec<PathBuf>> = None;

        self.finish_jobs();

        ui.vertical(|ui| {
            ui.add_space(5.);
            ui.heading("Filter & Order");
            ui.add_space(10.);

            ui.strong("Filter");

            for field in &mut self.filter_fields {
                let default_values = field.get_default_values();
                ui.take_available_width(); //must use this call so the panel doesn't grow infinitely
                let width = ui.available_width();
                ui.horizontal(|ui| {
                    ui.set_max_width(width);
                    if ui
                        .add(
                            DropDownBox::from_iter(
                                &self.unique_exif_tags,
                                format!("{}_tag", &field.id.value()),
                                &mut field.name,
                                |ui, text| ui.selectable_label(false, text),
                            )
                            .max_height(600.)
                            .desired_width(width - 60.)
                            .filter_by_input(true)
                            .select_on_focus(true),
                        )
                        .changed()
                        && self.unique_exif_tags.contains(&field.name)
                    {
                        let name = field.name.clone();
                        let mut repo = self.db_repo.clone();
                        field.default_values_job = Some(thread::spawn(move || {
                            repo.get_distinct_values_for_exif_tag(&name).ok()
                        }));
                    }

                    egui::ComboBox::from_id_salt(format!("{}_operator", &field.id.value()))
                        .width(50.)
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
                });

                ui.add(
                    DropDownBox::from_iter(
                        &default_values,
                        format!("{}_{}_value", &field.id.value(), &field.name),
                        &mut field.value,
                        |ui, text| ui.selectable_label(false, text),
                    )
                    .desired_width(width)
                    .filter_by_input(true)
                    .select_on_focus(true)
                    .max_height(500.),
                );

                ui.add_space(5.);
            }

            ui.add_space(5.);

            ui.horizontal(|ui| {
                if ui.button("+").clicked() {
                    self.filter_fields
                        .push(FilterField::new("", "", &self.db_repo));
                }

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("🔄").clicked() {
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

            ui.strong("XMP Rating & Tags");

            ui.label("Rating");
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt("rating_first_operator")
                    .width(50.)
                    .selected_text(self.rating_field.first_operator.to_string())
                    .show_ui(ui, |ui| {
                        for op in SqlOperator::list() {
                            ui.selectable_value(
                                &mut self.rating_field.first_operator,
                                op.clone(),
                                op.to_string(),
                            );
                        }
                    });
                egui::ComboBox::from_id_salt("rating_first_rating")
                    .width(50.)
                    .selected_text(self.rating_field.first_rating.to_string())
                    .show_ui(ui, |ui| {
                        for op in XMPRating::list() {
                            ui.selectable_value(
                                &mut self.rating_field.first_rating,
                                op.to_string(),
                                op.to_string(),
                            );
                        }
                    });
                ui.add_space(10.);
                ui.label("&");
                ui.add_space(10.);
                egui::ComboBox::from_id_salt("rating_second_operator")
                    .width(50.)
                    .selected_text(self.rating_field.second_operator.to_string())
                    .show_ui(ui, |ui| {
                        for op in SqlOperator::list() {
                            ui.selectable_value(
                                &mut self.rating_field.second_operator,
                                op.clone(),
                                op.to_string(),
                            );
                        }
                    });
                egui::ComboBox::from_id_salt("rating_second_rating")
                    .width(50.)
                    .selected_text(self.rating_field.second_rating.to_string())
                    .show_ui(ui, |ui| {
                        for op in XMPRating::list() {
                            ui.selectable_value(
                                &mut self.rating_field.second_rating,
                                op.to_string(),
                                op.to_string(),
                            );
                        }
                    });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("🔄").clicked() {
                        self.rating_field.first_operator = SqlOperator::None;
                        self.rating_field.first_rating = String::new();
                        self.rating_field.second_operator = SqlOperator::None;
                        self.rating_field.second_rating = String::new();
                    }
                });
            });

            ui.label("Tags");

            let mut remove_idx: Option<usize> = None;
            for (i, tag) in self.tag_field.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                        if ui.button("🗑").clicked() {
                            remove_idx = Some(i);
                        }

                        ui.add(
                            DropDownBox::from_iter(&self.unique_xmp_tags, i, tag, |ui, text| {
                                ui.selectable_label(false, text)
                            })
                            .max_height(600.)
                            .filter_by_input(true)
                            .select_on_focus(true)
                            .desired_width(ui.available_width()),
                        );
                    });
                });
            }
            if let Some(idx) = remove_idx {
                self.tag_field.remove(idx);
            }

            ui.horizontal(|ui| {
                if ui.button("+").clicked() {
                    self.tag_field.push(String::new());
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("🔄").clicked() {
                        self.tag_field.clear();
                    }
                });
            });

            ui.add_space(10.);

            ui.checkbox(&mut self.group_raw_jpeg, "Group RAW + JPEG");

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

                        let order_tag = self.order_field.tag.clone();
                        let order_direction = self.order_field.order.clone();
                        let mut repo = self.db_repo.clone();
                        let worker_mutex = self.worker.clone();
                        state.grouping.enabled = self.group_raw_jpeg;
                        let rating_one_f = (
                            self.rating_field.first_operator.clone(),
                            self.rating_field.first_rating.clone(),
                        );
                        let rating_two_f = (
                            self.rating_field.second_operator.clone(),
                            self.rating_field.second_rating.clone(),
                        );
                        let xmp_tags = self.tag_field.clone();
                        self.query_handle = Some(thread::spawn(move || {
                            let filtered_paths = repo
                                .get_paths_filtered_by_metadata(
                                    &fields,
                                    &order_tag,
                                    &order_direction,
                                    &rating_one_f,
                                    &rating_two_f,
                                    &xmp_tags,
                                )
                                .ok();

                            if let Some(paths) = filtered_paths.clone() {
                                if let Ok(worker) = worker_mutex.try_lock() {
                                    worker.send_job(crate::worker::Job::ClearMovedFiles(
                                        paths.clone(),
                                    ));
                                } else {
                                    tracing::error!(
                                        "Failure locking worker mutex to clear moved files"
                                    );
                                }
                            }

                            filtered_paths
                        }));
                    }

                    if self.query_handle.is_some() {
                        let qh = self.query_handle.take().unwrap();
                        if qh.is_finished() {
                            if let Ok(Some(paths)) = qh.join() {
                                self.last_query_count = Some(paths.len() as u32);
                                return_paths = Some(paths.clone());

                                if state.grouping.enabled {
                                    state.grouping.enabled = true;
                                    return_paths = Some(Metadata::group_raw_pairs(
                                        &paths,
                                        &mut state.grouping.lookup,
                                    ));
                                } else {
                                    state.grouping.enabled = false;
                                }
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

    pub fn finish_jobs(&mut self) {
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

        if self.unique_xmp_tags_job.is_some() {
            let qh = self.unique_xmp_tags_job.take().unwrap();
            if qh.is_finished() {
                if let Ok(Some(values)) = qh.join() {
                    self.unique_xmp_tags = values;
                }
            } else {
                self.unique_xmp_tags_job = Some(qh);
            }
        }
    }
}
