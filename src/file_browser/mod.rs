mod list_view;
mod tree_view;

use std::path::{Component, Path};
use std::sync::Arc;

use adw::prelude::*;
use glib::subclass::types::ObjectSubclassIsExt;
use gtk::glib;

use list_view::FileTreeView;

use crate::{misc::escape_filename, shell, subscriptions::SubscriptionKey, ui::UiMutex};

mod imp {
    use std::sync::Arc;

    use adw::subclass::prelude::*;
    use gtk::glib;
    use once_cell::unsync::OnceCell;

    use crate::{file_browser::list_view::FileTreeView, shell, ui::UiMutex};

    #[derive(Default)]
    pub struct VimdicatorFileBrowser {
        pub(super) file_tree_view: OnceCell<FileTreeView>,
        pub(super) shell_state: OnceCell<Arc<UiMutex<shell::State>>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for VimdicatorFileBrowser {
        const NAME: &'static str = "VimdicatorFileBrowser";
        type Type = super::VimdicatorFileBrowser;
        type ParentType = adw::Bin;
    }

    impl ObjectImpl for VimdicatorFileBrowser {}
    impl WidgetImpl for VimdicatorFileBrowser {}
    impl BinImpl for VimdicatorFileBrowser {}
}

glib::wrapper! {
    pub struct VimdicatorFileBrowser(ObjectSubclass<imp::VimdicatorFileBrowser>)
        @extends adw::Bin,
        @implements gtk::Widget;
}

impl VimdicatorFileBrowser {
    pub fn new(shell_state: &Arc<UiMutex<shell::State>>) -> Self {
        let file_tree_view = FileTreeView::new();

        let window = gtk::ScrolledWindow::builder()
            .focusable(false)
            .vexpand(true)
            .valign(gtk::Align::Fill)
            .child(&file_tree_view)
            .build();

        let this: Self = glib::Object::new();
        this.set_focusable(false);
        this.set_width_request(150);
        this.style_context().add_class("view");
        this.set_child(Some(&window));
        this.set_sensitive(false);

        this.imp().file_tree_view.set(file_tree_view).unwrap();
        this.imp()
            .shell_state
            .set(shell_state.clone())
            .ok()
            .unwrap();

        this
    }

    pub fn file_tree_view(&self) -> &FileTreeView {
        self.imp().file_tree_view.get().unwrap()
    }

    pub fn init(&self) {
        // Further initialization.
        self.init_subscriptions(&self.imp().shell_state.get().unwrap().borrow());
        self.connect_events();
    }

    fn init_subscriptions(&self, shell_state: &shell::State) {
        // Reveal the file of an entered buffer in the file browser and select the entry.
        let file_tree_view = self.imp().file_tree_view.get().unwrap().clone();
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
        let shell_state_ref = self.imp().shell_state.get().unwrap();

        self.imp()
            .file_tree_view
            .get()
            .unwrap()
            .list_view()
            .connect_activate({
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
