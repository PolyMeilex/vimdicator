use adw::traits::BinExt;
use gtk::prelude::*;

mod list_item;
pub use list_item::ListItem;

mod imp {
    use adw::subclass::prelude::*;

    pub struct FileTreeView {}

    impl Default for FileTreeView {
        fn default() -> Self {
            Self {}
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for FileTreeView {
        const NAME: &'static str = "FileTreeView";
        type Type = super::FileTreeView;
        type ParentType = adw::Bin;
    }

    impl ObjectImpl for FileTreeView {}
    impl WidgetImpl for FileTreeView {}
    impl BinImpl for FileTreeView {}
}

glib::wrapper! {
    pub struct FileTreeView(ObjectSubclass<imp::FileTreeView>) @extends adw::Bin, @implements gtk::Widget;
}

impl FileTreeView {
    pub fn new() -> Self {
        let this = glib::Object::new::<Self>();

        this.set_child(Some(&get_list_view()));
        this
    }

    pub fn list_view(&self) -> gtk::ListView {
        let child = self.child().unwrap();
        child.downcast().unwrap()
    }
}

fn get_tree_model() -> gtk::TreeListModel {
    let path = "./";

    let tree = fs_tree::FileTree::from_path(path).unwrap();

    let model = gio::ListStore::new(ListItem::static_type());

    for child in tree.children().cloned().unwrap_or_default() {
        model.append(&ListItem::new(child));
    }

    gtk::TreeListModel::new(model, false, false, |item| {
        let item = item.downcast_ref::<ListItem>().unwrap();

        let children = item.tree().children().cloned().unwrap_or_default();

        if children.is_empty() {
            None
        } else {
            let model = gio::ListStore::new(ListItem::static_type());

            for child in children {
                model.append(&ListItem::new(child));
            }

            Some(model.upcast())
        }
    })
}

fn get_list_view() -> gtk::ListView {
    let tree_model = get_tree_model();

    let selection_model = gtk::SingleSelection::new(Some(tree_model));

    let factory = gtk::SignalListItemFactory::new();

    // TODO: just use xml

    factory.connect_setup(move |_factory, item| {
        // in gtk4 > 4.8 it was switched to taking a GObject
        let item = item.downcast_ref::<gtk::ListItem>().unwrap();

        let row = gtk::TreeExpander::new();

        let b = gtk::Box::new(gtk::Orientation::Horizontal, 5);
        b.append(&gtk::Image::new());
        b.append(&gtk::Label::new(None));

        row.set_child(Some(&b));

        item.set_child(Some(&row));
    });

    // the bind stage is used for "binding" the data to the created widgets on the "setup" stage
    factory.connect_bind(move |_factory, item| {
        // in gtk4 > 4.8 it was switched to taking a GObject
        let item = item.downcast_ref::<gtk::ListItem>().unwrap();

        let row: gtk::TreeExpander = item.child().and_downcast().unwrap();
        let item: gtk::TreeListRow = item.item().and_downcast().unwrap();

        let data: ListItem = item.item().and_downcast().unwrap();

        row.child()
            .and_downcast::<gtk::Box>()
            .unwrap()
            .first_child()
            .and_downcast::<gtk::Image>()
            .unwrap()
            .set_icon_name(Some(if data.tree().is_dir() {
                "system-file-manager-symbolic"
            } else {
                "emblem-documents-symbolic"
            }));

        row.child()
            .and_downcast::<gtk::Box>()
            .unwrap()
            .last_child()
            .and_downcast::<gtk::Label>()
            .unwrap()
            .set_label(data.name());

        row.set_list_row(Some(&item));
    });

    let view = gtk::ListView::new(Some(selection_model), Some(factory));

    view.add_css_class("navigation-sidebar");

    view
}
