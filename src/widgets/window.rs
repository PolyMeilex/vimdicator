use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gio, glib};

use crate::widgets::{ExtLineGrid, ExtPopupMenu};
use std::cell::RefCell;

mod imp {
    use super::*;

    #[derive(Debug, Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/polymeilex/vimdicator/widgets/window.ui")]
    pub struct VimdicatorWindow {
        #[template_child]
        pub header_bar_revealer: TemplateChild<gtk::Revealer>,
        #[template_child]
        pub header_bar: TemplateChild<gtk::HeaderBar>,
        #[template_child]
        pub tab_view: TemplateChild<adw::TabView>,
        #[template_child]
        pub main_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub ext_line_grid: TemplateChild<ExtLineGrid>,
        #[template_child]
        pub popover: TemplateChild<gtk::Popover>,
        #[template_child]
        pub ext_popup_menu: TemplateChild<ExtPopupMenu>,

        pub ext_tabline: RefCell<Option<crate::nvim::ExtTabline>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for VimdicatorWindow {
        const NAME: &'static str = "VimdicatorWindow";
        type Type = super::VimdicatorWindow;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            ExtPopupMenu::static_type();
            ExtLineGrid::static_type();
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for VimdicatorWindow {}
    impl WidgetImpl for VimdicatorWindow {}
    impl WindowImpl for VimdicatorWindow {}
    impl ApplicationWindowImpl for VimdicatorWindow {}
    impl AdwApplicationWindowImpl for VimdicatorWindow {}
}

glib::wrapper! {
    pub struct VimdicatorWindow(ObjectSubclass<imp::VimdicatorWindow>)
        @extends gtk::Widget, gtk::Window, gtk::ApplicationWindow, adw::ApplicationWindow,        @implements gio::ActionGroup, gio::ActionMap;
}

impl VimdicatorWindow {
    pub fn new<P: glib::IsA<gtk::Application>>(application: &P) -> Self {
        glib::Object::builder()
            .property("application", application)
            .build()
    }

    pub fn header_bar_revealer(&self) -> gtk::Revealer {
        self.imp().header_bar_revealer.clone()
    }

    pub fn ext_line_grid(&self) -> ExtLineGrid {
        self.imp().ext_line_grid.clone()
    }

    pub fn main_box(&self) -> gtk::Box {
        self.imp().main_box.clone()
    }

    pub fn popover(&self) -> gtk::Popover {
        self.imp().popover.clone()
    }

    pub fn ext_popup_menu(&self) -> ExtPopupMenu {
        self.imp().ext_popup_menu.get()
    }

    pub fn update_tabs(&self, tabline: &crate::nvim::ExtTabline) {
        let tab_view = self.imp().tab_view.get();

        struct HashItem {
            tabpage: crate::Tabpage,
            page: Option<adw::TabPage>,
            id: usize,
        }

        impl std::cmp::Eq for HashItem {}
        impl std::cmp::PartialEq for HashItem {
            fn eq(&self, other: &Self) -> bool {
                self.tabpage == other.tabpage
            }
        }
        impl std::hash::Hash for HashItem {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                self.tabpage.hash(state);
            }
        }

        let mut old_set = std::collections::HashSet::new();
        let mut new_set = std::collections::HashSet::new();

        for (id, (_, tabpage)) in self
            .imp()
            .ext_tabline
            .borrow()
            .as_ref()
            .map(|last| last.tabs())
            .into_iter()
            .flatten()
            .enumerate()
        {
            let page = tab_view.nth_page(id as i32);

            old_set.insert(HashItem {
                tabpage: tabpage.clone(),
                page: Some(page),
                id,
            });
        }

        for (id, (_, tabpage)) in tabline.tabs().iter().enumerate() {
            new_set.insert(HashItem {
                tabpage: tabpage.clone(),
                page: None,
                id,
            });
        }

        for item in old_set.difference(&new_set) {
            if let Some(page) = item.page.as_ref() {
                tab_view.close_page(page);
            }
        }

        let pages: Vec<_> = (0..tab_view.n_pages())
            .map(|id| tab_view.nth_page(id))
            .collect();

        for item in new_set.difference(&old_set) {
            let page = item.id.checked_sub(1).and_then(|id| pages.get(id));
            tab_view.add_page(&gtk::Label::new(None), page);
        }

        for (id, (name, tab)) in tabline.tabs().iter().enumerate() {
            let page = tab_view.nth_page(id as i32);

            page.set_title(name);
            page.is_pinned();

            if Some(tab) == tabline.current_tab() {
                tab_view.set_selected_page(&page);
            }
        }

        *self.imp().ext_tabline.borrow_mut() = Some(tabline.clone());
    }
}
