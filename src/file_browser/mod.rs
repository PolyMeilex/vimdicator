mod list_view;
mod tree_view;

use std::cell::RefCell;
use std::cmp::Ordering;
use std::fs;
use std::fs::DirEntry;
use std::io;
use std::ops::Deref;
use std::path::{Component, Path};
use std::rc::Rc;
use std::sync::Arc;

use log::error;

use gio::prelude::*;
use gtk::{prelude::*, Inhibit};

use crate::misc::escape_filename;
use crate::nvim::NvimSession;
use crate::shell;
use crate::spawn_timeout;
use crate::subscriptions::SubscriptionKey;
use crate::ui::UiMutex;

use tree_view::TreeView;

use self::list_view::FileTreeView;

const ICON_FOLDER_CLOSED: &str = "folder-symbolic";
const ICON_FOLDER_OPEN: &str = "folder-open-symbolic";
const ICON_FILE: &str = "text-x-generic-symbolic";

struct Components {
    context_menu: gtk::PopoverMenu,
    show_hidden_action: gio::SimpleAction,
    cd_action: gio::SimpleAction,
}

struct State {
    current_dir: String,
    show_hidden: bool,
    selected_path: Option<String>,
}

pub struct FileBrowserWidget {
    store: gtk::TreeStore,
    file_tree_view: FileTreeView,
    tree: TreeView,
    widget: gtk::Box,
    shell_state: Arc<UiMutex<shell::State>>,
    comps: Components,
    state: Rc<RefCell<State>>,
}

impl Deref for FileBrowserWidget {
    type Target = gtk::Box;

    fn deref(&self) -> &gtk::Box {
        &self.widget
    }
}

#[derive(Copy, Clone, Debug)]
enum FileType {
    File,
    Dir,
}

enum Column {
    Filename,
    Path,
    FileType,
    IconName,
}

impl FileBrowserWidget {
    pub fn new(shell_state: &Arc<UiMutex<shell::State>>) -> Self {
        let widget = gtk::Box::builder()
            .focusable(false)
            .sensitive(false)
            .width_request(150)
            .orientation(gtk::Orientation::Vertical)
            .build();
        widget.style_context().add_class("view");

        let store = gtk::TreeStore::new(&[
            glib::Type::STRING,
            glib::Type::STRING,
            glib::Type::U8,
            glib::Type::STRING,
        ]);
        let tree = TreeView::new();
        tree.set_focusable(false);
        tree.set_headers_visible(false);
        tree.set_show_expanders(false);
        tree.set_level_indentation(20);
        tree.set_activate_on_single_click(true);
        tree.set_model(Some(&store));
        tree.selection().set_mode(gtk::SelectionMode::Single);

        let context_menu = gtk::PopoverMenu::builder()
            .position(gtk::PositionType::Bottom)
            .build();
        tree.set_context_menu(&context_menu);

        let tree_column = gtk::TreeViewColumn::builder()
            .sizing(gtk::TreeViewColumnSizing::Autosize)
            .build();

        #[rustfmt::skip]
        let pixbuf_renderer = gtk::CellRendererPixbuf::builder()
            .xpad(6)
            .build();
        tree_column.pack_start(&pixbuf_renderer, false);
        tree_column.add_attribute(&pixbuf_renderer, "icon-name", 3);

        let text_renderer = gtk::CellRendererText::new();
        tree_column.pack_start(&text_renderer, false);
        tree_column.add_attribute(&text_renderer, "text", 0);

        tree.append_column(&tree_column);

        let file_tree_view = FileTreeView::new();

        let window = gtk::ScrolledWindow::builder()
            .focusable(false)
            .vexpand(true)
            .valign(gtk::Align::Fill)
            .child(&file_tree_view)
            .build();
        widget.append(&window);

        let menu = gio::Menu::new();

        let section = gio::Menu::new();
        section.append(Some("Go to directory"), Some("filebrowser.cd"));
        menu.append_section(None, &section);

        let section = gio::Menu::new();
        section.append(Some("Reload"), Some("filebrowser.reload"));
        section.append(Some("Show hidden files"), Some("filebrowser.show-hidden"));
        menu.append_section(None, &section);

        context_menu.set_menu_model(Some(&menu));

        FileBrowserWidget {
            store,
            file_tree_view,
            tree,
            widget,
            comps: Components {
                context_menu,
                cd_action: gio::SimpleAction::new("cd", None),
                show_hidden_action: gio::SimpleAction::new_stateful(
                    "show-hidden",
                    None,
                    false.to_variant(),
                ),
            },
            state: Rc::new(RefCell::new(State {
                current_dir: "".to_owned(),
                show_hidden: false,
                selected_path: None,
            })),
            shell_state: shell_state.clone(),
        }
    }

    pub fn file_tree_view(&self) -> &FileTreeView {
        &self.file_tree_view
    }

    fn nvim(&self) -> Option<NvimSession> {
        self.shell_state.borrow().nvim()
    }

    pub fn init(&mut self) {
        // Initialize values.
        if let Some(dir) = get_current_dir(&self.nvim().unwrap()) {
            self.state.borrow_mut().current_dir = dir;
        }

        // Populate tree.
        tree_reload(&self.store, &self.state.borrow());

        let store = &self.store;
        let state_ref = &self.state;
        self.tree
            .connect_test_expand_row(clone!(store, state_ref => move |_, iter, _| {
                store.set(iter, &[(Column::IconName as u32, &ICON_FOLDER_OPEN)]);
                // We cannot recursively populate all directories. Instead, we have prepared a single
                // empty child entry for all non-empty directories, so the row will be expandable. Now,
                // when a directory is expanded, populate its children.
                let state = state_ref.borrow();
                if let Some(child) = store.iter_children(Some(iter)) {
                    let filename = store.get_value(&child, Column::Filename as i32);
                    if filename.get::<&str>().is_err() {
                        store.remove(&child);
                        let dir: String = store.get(iter, Column::Path as i32);
                        populate_tree_nodes(&store, &state, &dir, Some(iter));
                    } else {
                        // This directory is already populated, i.e. it has been expanded and collapsed
                        // again. Rows further down the tree might have been silently collapsed without
                        // getting an event. Update their folder icon.
                        let mut tree_path = store.path(&child);
                        while let Some(iter) = store.iter(&tree_path) {
                            tree_path.next();
                            let file_type: u8 = store.get(&iter, Column::FileType as i32);
                            if file_type == FileType::Dir as u8 {
                                store.set(&iter, &[(Column::IconName as u32, &ICON_FOLDER_CLOSED)]);
                            }
                        }
                    }
                }
                Inhibit(false)
            }));

        self.tree
            .connect_row_collapsed(clone!(store => move |_, iter, _| {
                store.set(iter, &[(Column::IconName as u32, &ICON_FOLDER_CLOSED)]);
            }));

        // Further initialization.
        self.init_actions();
        self.init_subscriptions(&self.shell_state.borrow());
        self.connect_events();
    }

    fn init_actions(&self) {
        let actions = gio::SimpleActionGroup::new();

        let store = &self.store;
        let state_ref = &self.state;
        let nvim_ref = self.shell_state.borrow().nvim_clone();

        let reload_action = gio::SimpleAction::new("reload", None);
        reload_action.connect_activate(clone!(store, state_ref => move |_, _| {
            tree_reload(&store, &state_ref.borrow());
        }));
        actions.add_action(&reload_action);

        let cd_action = &self.comps.cd_action;
        cd_action.connect_activate(clone!(state_ref, nvim_ref => move |_, _| {
            let nvim = nvim_ref.nvim().unwrap();
            if let Some(ref path) = &state_ref.borrow().selected_path {
                let path = path.clone();
                spawn_timeout!(nvim.set_current_dir(&path));
            }
        }));
        actions.add_action(cd_action);

        // Show / hide hidden files when corresponding menu item is toggled.
        let show_hidden_action = &self.comps.show_hidden_action;
        show_hidden_action.connect_activate(clone!(state_ref, store => move |action, _| {
            let mut state = state_ref.borrow_mut();
            state.show_hidden = !state.show_hidden;
            action.set_state(state.show_hidden.to_variant());
            tree_reload(&store, &state);
        }));
        actions.add_action(show_hidden_action);

        self.comps
            .context_menu
            .insert_action_group("filebrowser", Some(&actions));
    }

    fn init_subscriptions(&self, shell_state: &shell::State) {
        // Always set the current working directory as the root of the file browser.
        let store = &self.store;
        let state_ref = &self.state;
        shell_state.subscribe(
            SubscriptionKey::from("DirChanged"),
            &["getcwd()"],
            clone!(store, state_ref => move |args| {
                let dir = args.into_iter().next().unwrap();
                if dir != state_ref.borrow().current_dir {
                    state_ref.borrow_mut().current_dir = dir;
                    tree_reload(&store, &state_ref.borrow());
                }
            }),
        );

        // Reveal the file of an entered buffer in the file browser and select the entry.
        let file_tree_view = self.file_tree_view.clone();
        let subscription = shell_state.subscribe(
            SubscriptionKey::from("BufEnter"),
            &["getcwd()", "expand('%:p')"],
            move |args| {
                let mut args_iter = args.into_iter();
                let dir = args_iter.next().unwrap();
                let file_path = args_iter.next().unwrap();
                let could_reveal =
                    if let Ok(rel_path) = Path::new(&file_path).strip_prefix(Path::new(&dir)) {
                        reveal_path_in_tree(&file_tree_view, rel_path)
                    } else {
                        false
                    };
                if !could_reveal {
                    file_tree_view
                        .list_view()
                        .model()
                        .and_downcast::<gtk::SingleSelection>()
                        .unwrap()
                        .unselect_all();
                }
            },
        );
        shell_state.run_now(&subscription);
    }

    fn connect_events(&self) {
        // Open file / go to dir, when user clicks on an entry.
        let store = &self.store;
        let state_ref = &self.state;
        let shell_state_ref = &self.shell_state;

        self.tree
            .connect_row_activated(clone!(store, shell_state_ref => move |tree, path, _| {
                let iter = store.iter(path).unwrap();
                let file_type: u8 = store.get(&iter, Column::FileType as i32);
                let file_path: String = store.get(&iter, Column::Path as i32);
                if file_type == FileType::Dir as u8 {
                    let expanded = tree.row_expanded(path);
                    if expanded {
                        tree.collapse_row(path);
                    } else {
                        tree.expand_row(path, false);
                    }
                } else {
                    // FileType::File
                    let file_path = escape_filename(file_path.as_str()).to_string();

                    shell_state_ref.borrow().open_file(&file_path);
                }
            }));

        self.file_tree_view.list_view().connect_activate({
            let shell_state_ref = shell_state_ref.clone();
            move |list_view, position| {
                let item = list_view.model().unwrap().item(position).unwrap();
                let item = item.downcast::<gtk::TreeListRow>().unwrap();

                let item = item.item().and_downcast::<list_view::ListItem>().unwrap();

                let tree = item.tree();
                if tree.is_regular() {
                    let file_path = &tree.path().to_string_lossy();
                    let file_path = escape_filename(file_path);

                    shell_state_ref.borrow().open_file(&file_path);
                }
            }
        });

        let context_menu = &self.comps.context_menu;
        let cd_action = &self.comps.cd_action;
        #[rustfmt::skip]
        let right_click_controller = gtk::GestureClick::builder()
            .button(3)
            .build();
        right_click_controller.connect_pressed(
            clone!(store, state_ref, context_menu, cd_action => move |controller, _, x, y| {
                open_context_menu(
                    controller,
                    x,
                    y,
                    &mut state_ref.borrow_mut(),
                    &store,
                    &context_menu,
                    &cd_action
                )
            }),
        );
        self.tree.add_controller(right_click_controller);

        #[rustfmt::skip]
        let long_tap_controller = gtk::GestureLongPress::builder()
            .touch_only(true)
            .build();
        long_tap_controller.connect_pressed(
            clone!(store, state_ref, context_menu, cd_action => move |controller, x, y| {
                open_context_menu(
                    controller,
                    x,
                    y,
                    &mut state_ref.borrow_mut(),
                    &store,
                    &context_menu,
                    &cd_action
                )
            }),
        );
        self.tree.add_controller(long_tap_controller);
    }
}

fn open_context_menu<E>(
    controller: &E,
    x: f64,
    y: f64,
    state: &mut State,
    store: &gtk::TreeStore,
    context_menu: &gtk::PopoverMenu,
    cd_action: &gio::SimpleAction,
) where
    E: glib::IsA<gtk::EventController>,
{
    // Open context menu on right click.
    context_menu.set_pointing_to(Some(&gdk::Rectangle::new(
        x.round() as i32,
        y.round() as i32,
        0,
        0,
    )));
    context_menu.popup();
    let iter = controller
        .widget()
        .downcast::<gtk::TreeView>()
        .unwrap()
        .path_at_pos(x as i32, y as i32)
        .and_then(|(path, _, _, _)| path)
        .and_then(|path| store.iter(&path));
    let file_type = iter
        .as_ref()
        .map(|iter| store.get::<u8>(iter, Column::FileType as i32));
    // Enable the "Go To Directory" action only if the user clicked on a folder.
    cd_action.set_enabled(file_type == Some(FileType::Dir as u8));

    let path = iter.map(|iter| store.get::<String>(&iter, Column::Path as i32));
    state.selected_path = path;
}

/// Compare function for dir entries.
///
/// Sorts directories above files.
fn cmp_dirs_first(lhs: &DirEntry, rhs: &DirEntry) -> io::Result<Ordering> {
    let lhs_metadata = fs::metadata(lhs.path())?;
    let rhs_metadata = fs::metadata(rhs.path())?;
    if lhs_metadata.is_dir() == rhs_metadata.is_dir() {
        Ok(lhs
            .path()
            .to_string_lossy()
            .to_lowercase()
            .cmp(&rhs.path().to_string_lossy().to_lowercase()))
    } else if lhs_metadata.is_dir() {
        Ok(Ordering::Less)
    } else {
        Ok(Ordering::Greater)
    }
}

/// Clears an repopulate the entire tree.
fn tree_reload(store: &gtk::TreeStore, state: &State) {
    let dir = &state.current_dir;
    store.clear();
    populate_tree_nodes(store, state, dir, None);
}

/// Populates one level, i.e. one directory of the file browser tree.
fn populate_tree_nodes(
    store: &gtk::TreeStore,
    state: &State,
    dir: &str,
    parent: Option<&gtk::TreeIter>,
) {
    let path = Path::new(dir);
    let read_dir = match path.read_dir() {
        Ok(read_dir) => read_dir,
        Err(err) => {
            error!("Couldn't populate tree: {}", err);
            return;
        }
    };
    let iter = read_dir.filter_map(Result::ok);
    let mut entries: Vec<DirEntry> = if state.show_hidden {
        iter.collect()
    } else {
        iter.filter(|entry| !entry.file_name().to_string_lossy().starts_with('.'))
            .filter(|entry| !entry.file_name().to_string_lossy().ends_with('~'))
            .collect()
    };
    entries.sort_unstable_by(|lhs, rhs| cmp_dirs_first(lhs, rhs).unwrap_or(Ordering::Equal));
    for entry in entries {
        let path = if let Some(path) = entry.path().to_str() {
            path.to_owned()
        } else {
            // Skip paths that contain invalid unicode.
            continue;
        };
        let filename = entry.file_name().to_str().unwrap().to_owned();
        let file_type = if let Ok(metadata) = fs::metadata(entry.path()) {
            let file_type = metadata.file_type();
            if file_type.is_dir() {
                FileType::Dir
            } else if file_type.is_file() {
                FileType::File
            } else {
                continue;
            }
        } else {
            // In case of invalid symlinks, we cannot obtain metadata.
            continue;
        };
        let icon = match file_type {
            FileType::Dir => ICON_FOLDER_CLOSED,
            FileType::File => ICON_FILE,
        };
        // When we get until here, we want to show the entry. Append it to the tree.
        let iter = store.append(parent);
        store.set(
            &iter,
            &[
                (0, &filename),
                (1, &path),
                (2, &(file_type as u8)),
                (3, &icon),
            ],
        );
        // For directories, check whether the directory is empty. If not, append a single empty
        // entry, so the expand arrow is shown. Its contents are dynamically populated when
        // expanded (see `init`).
        if let FileType::Dir = file_type {
            let not_empty = if let Ok(mut dir) = entry.path().read_dir() {
                dir.next().is_some()
            } else {
                false
            };
            if not_empty {
                let iter = store.append(Some(&iter));
                store.set(&iter, &[]);
            }
        }
    }
}

fn get_current_dir(nvim: &NvimSession) -> Option<String> {
    match nvim.block_timeout(nvim.eval("getcwd()")) {
        Ok(cwd) => cwd.as_str().map(|s| s.to_owned()),
        Err(err) => {
            error!("Couldn't get cwd: {}", err);
            None
        }
    }
}

/// Reveals and selects the given file in the file browser.
///
/// Returns `true` if the file could be successfully revealed.
fn reveal_path_in_tree(file_tree_view: &FileTreeView, rel_file_path: &Path) -> bool {
    let list_view = file_tree_view.list_view();
    let single_selection_model = list_view
        .model()
        .and_downcast::<gtk::SingleSelection>()
        .unwrap();
    let tree_model = single_selection_model
        .model()
        .and_downcast::<gtk::TreeListModel>()
        .unwrap();

    // TODO:
    let mut segments = rel_file_path.components();
    {
        let root_model = tree_model.model();

        let n1 = segments.next().unwrap();
        if let Component::Normal(name) = n1 {
            let (id, _) = root_model
                .iter::<list_view::ListItem>()
                .enumerate()
                .flat_map(|(id, item)| item.ok().map(|item| (id, item)))
                .find(|(_, item)| item.name() == name.to_string_lossy())
                .unwrap();

            let root = tree_model.child_row(id as u32).unwrap();
            root.set_expanded(true);

            if let Some(model) = root.children() {
                let n2 = segments.next().unwrap();
                if let Component::Normal(name) = n2 {
                    let (id, _) = model
                        .iter::<list_view::ListItem>()
                        .enumerate()
                        .flat_map(|(id, item)| item.ok().map(|item| (id, item)))
                        .find(|(_, item)| item.name() == name.to_string_lossy())
                        .unwrap();

                    let root = root.child_row(id as u32).unwrap();
                    root.set_expanded(true);
                    let list_pos = root.position();
                    single_selection_model.select_item(list_pos, true);
                    return true;
                }
            }
        }
    }

    false
}
