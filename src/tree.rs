use eframe::{
    egui::{self, Area, Id},
    emath::Align,
    epaint::{Color32, Pos2, Shadow},
};
use std::path::{Path, PathBuf};

use crate::utils;

#[derive(Clone)]
struct Tree {
    pub opened_path: String,
    pub selected_index: usize,
    pub should_scroll: bool,
    pub entries: Vec<TreeEntry>,
}

#[derive(Clone)]
struct TreeEntry {
    pub depth: usize,
    pub path: PathBuf,
    pub name: String,
    pub expanded: bool,
    pub img_count: u32,
}

//Would've been easier to build if we used recursive structs
//But building it flat like this is easier to traverse and display
//A good trade off in my opinion
impl Tree {
    pub fn from(path: &str) -> Tree {
        let parents = Self::get_all_parents(&PathBuf::from(path));

        let mut tree_entries: Vec<TreeEntry> = vec![];

        for (depth, (i, parent)) in parents.iter().enumerate().enumerate() {
            let path_entries = Self::get_entries_from_path(parent, depth, parents.get(i + 1));
            let parent_pos = match tree_entries.iter().position(|x| &x.path == parent) {
                Some(pos) => pos + 1,
                None => 0,
            };

            for (j, entry) in path_entries.iter().enumerate() {
                tree_entries.insert(parent_pos + j, entry.clone());
            }
        }

        Tree {
            opened_path: path.to_string(),
            selected_index: tree_entries
                .iter()
                .position(|x| &x.path == parents.last().unwrap_or(&PathBuf::default()))
                .unwrap_or(0),
            entries: tree_entries,
            should_scroll: true,
        }
    }

    fn get_all_parents(path: &Path) -> Vec<PathBuf> {
        let mut parents: Vec<PathBuf> = vec![];
        let mut path = path;
        loop {
            match path.parent() {
                Some(parent) => {
                    parents.push(parent.to_path_buf());
                    path = parent;
                }
                None => {
                    parents.reverse();
                    return parents;
                }
            }
        }
    }

    fn toggled_at(&mut self, i: usize) {
        if self.entries[i].expanded {
            self.close_at(i);
        } else {
            self.open_at(i);
        }
    }

    fn close_at(&mut self, i: usize) {
        let toggled_entry = &mut self.entries[i];

        if !toggled_entry.expanded {
            return;
        }

        let toggled_index = i;
        let i = i + 1;

        toggled_entry.expanded = false;

        let depth = toggled_entry.depth;

        let mut removed_count = 0;
        loop {
            let next_entry = &mut self.entries[i];

            //If any entry we are removing was previously selected, we move the selection to
            //the parent
            if self.selected_index == i + removed_count {
                self.selected_index = toggled_index;
            }

            if next_entry.depth > depth {
                self.entries.remove(i);
                removed_count += 1;
            } else {
                break;
            }
        }
    }

    fn open_at(&mut self, i: usize) {
        let toggled_entry = &mut self.entries[i];

        if toggled_entry.expanded {
            return;
        }

        let toggled_index = i;
        let i = i + 1;

        toggled_entry.expanded = true;
        self.selected_index = toggled_index;
        let children =
            Self::get_entries_from_path(&toggled_entry.path, toggled_entry.depth + 1, None);

        for (j, child) in children.iter().enumerate() {
            self.entries.insert(i + j, child.clone());
        }
    }

    fn get_entries_from_path(
        path: &Path,
        depth: usize,
        parent: Option<&PathBuf>,
    ) -> Vec<TreeEntry> {
        let dir_info = match path.read_dir() {
            Ok(dir_info) => dir_info,
            Err(_) => return vec![],
        };

        let mut entries = dir_info
            .filter_map(|p| {
                let path = match p {
                    Ok(p) => p,
                    Err(_) => return None,
                };

                let metadata = match path.metadata() {
                    Ok(m) => m,
                    Err(_) => return None,
                };

                if metadata.is_dir() && !utils::is_dir_hidden(&path.path()) {
                    let expanded = match parent {
                        Some(parent) => parent == &path.path(),
                        None => false,
                    };

                    let mut img_count = 0;
                    if let Ok(rd) = path.path().read_dir() {
                        rd.for_each(|x| {
                            if let Ok(p) = &x {
                                if utils::is_valid_file(&p.path()) {
                                    img_count += 1
                                }
                            }
                        })
                    };

                    Some(TreeEntry {
                        path: path.path(),
                        name: path.file_name().to_str().unwrap_or("").to_string(),
                        depth,
                        expanded,
                        img_count,
                    })
                } else {
                    None
                }
            })
            .collect::<Vec<TreeEntry>>();

        entries.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

        entries
    }
}

pub fn ui(path: &str, ctx: &egui::Context) -> Option<PathBuf> {
    let mut result: Option<PathBuf> = None;
    let mut area_width = 700.;
    let available_width = ctx.available_rect().width();
    if available_width < area_width {
        area_width = available_width;
    }
    let pos_x = (available_width / 2.) - area_width / 2.;
    Area::new(Id::new("tree"))
        .fixed_pos(Pos2::new(pos_x, 5.))
        .order(egui::Order::Foreground)
        .interactable(true)
        .movable(false)
        .show(ctx, |ui| {
            egui::Frame::window(ui.style())
                .shadow(Shadow {
                    offset: [0, 0],
                    blur: 0,
                    spread: 0,
                    color: (Color32::from_white_alpha(0)),
                })
                .show(ui, |ui| {
                    ui.set_width(area_width);
                    ui.heading("Directory Tree");
                    ui.separator();

                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.set_width(ui.available_width());

                        let mut tree = match get_tree(ctx) {
                            Some(tree) => {
                                if tree.opened_path != path {
                                    Tree::from(path)
                                } else {
                                    tree
                                }
                            }
                            None => Tree::from(path),
                        };

                        let mut toggled_at = None;

                        for (i, entry) in tree.entries.iter().enumerate() {
                            let is_selected = i == tree.selected_index;

                            if is_selected && tree.should_scroll {
                                ui.scroll_to_cursor(Some(Align::Center));
                                tree.should_scroll = false;
                            }

                            let label = ui.selectable_label(
                                is_selected,
                                format!(
                                    "{} {} {}",
                                    "   ".repeat(entry.depth * 2),
                                    get_expanded_char(entry.expanded),
                                    if entry.img_count > 0 {
                                        format!("{} ({})", entry.name, entry.img_count)
                                    } else {
                                        entry.name.clone()
                                    }
                                ),
                            );
                            if label.clicked() {
                                toggled_at = Some(i);
                            };

                            if label.secondary_clicked() {
                                result = get_selected_path(&tree.entries[i].path);
                            }
                        }

                        if let Some(i) = toggled_at {
                            tree.toggled_at(i);
                        };

                        if ctx.input(|i| i.key_pressed(egui::Key::ArrowDown))
                            && tree.selected_index < tree.entries.len() - 1
                        {
                            tree.selected_index += 1;
                            tree.should_scroll = true;
                        }

                        if ctx.input(|i| i.key_pressed(egui::Key::ArrowUp))
                            && tree.selected_index != 0
                        {
                            tree.selected_index -= 1;
                            tree.should_scroll = true;
                        }

                        if ctx.input(|i| i.key_pressed(egui::Key::ArrowRight)) {
                            tree.open_at(tree.selected_index);
                        }

                        if ctx.input(|i| i.key_pressed(egui::Key::ArrowLeft)) {
                            tree.close_at(tree.selected_index);
                        }

                        if ctx.input(|i| i.key_pressed(egui::Key::Space)) {
                            tree.toggled_at(tree.selected_index);
                        }

                        if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                            result = get_selected_path(&tree.entries[tree.selected_index].path);
                        }

                        set_tree(ctx, &tree);
                    })
                });
        });

    result
}

fn get_tree_id() -> Id {
    Id::new("tree")
}

fn get_tree(ctx: &egui::Context) -> Option<Tree> {
    ctx.memory_mut(|mem| mem.data.get_temp::<Tree>(get_tree_id()))
}

fn set_tree(ctx: &egui::Context, tree: &Tree) {
    ctx.memory_mut(|mem| {
        mem.data.insert_temp::<Tree>(get_tree_id(), tree.clone());
    })
}

fn get_expanded_char(expanded: bool) -> String {
    if expanded {
        "⮩".to_string()
    } else {
        "➡".to_string()
    }
}

fn get_selected_path(path: &Path) -> Option<PathBuf> {
    if utils::is_valid_path(path) {
        Some(path.to_path_buf())
    } else {
        None
    }
}
