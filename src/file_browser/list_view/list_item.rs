use gtk::subclass::prelude::*;

mod imp {
    use gtk::subclass::prelude::*;
    use once_cell::unsync::OnceCell;

    pub struct ListItem {
        pub name: OnceCell<String>,
        pub tree: OnceCell<fs_tree::FileTree>,
    }

    impl Default for ListItem {
        fn default() -> Self {
            Self {
                name: OnceCell::new(),
                tree: OnceCell::new(),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ListItem {
        const NAME: &'static str = "ListItem";
        type Type = super::ListItem;
    }

    impl ObjectImpl for ListItem {}
}

glib::wrapper! {
    pub struct ListItem(ObjectSubclass<imp::ListItem>);
}

impl ListItem {
    pub fn new(tree: fs_tree::FileTree) -> Self {
        let this = glib::Object::new::<Self>();

        let name = tree.path().file_name().unwrap().to_string_lossy();

        this.imp().name.set(name.into()).unwrap();
        this.imp().tree.set(tree).unwrap();
        this
    }

    pub fn name(&self) -> &str {
        self.imp().name.get().unwrap()
    }

    pub fn tree(&self) -> &fs_tree::FileTree {
        self.imp().tree.get().unwrap()
    }
}
