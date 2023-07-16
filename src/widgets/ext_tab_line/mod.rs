use adw::subclass::prelude::*;
use gtk::glib;

use crate::nvim;
use std::cell::RefCell;

mod imp {
    use super::*;

    #[derive(Debug, Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/polymeilex/vimdicator/widgets/ext_tab_line/ext_tab_line.ui")]
    pub struct ExtTabLine {
        #[template_child]
        pub tab_view: TemplateChild<adw::TabView>,
        pub ext_tabline: RefCell<Option<nvim::ExtTabline>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ExtTabLine {
        const NAME: &'static str = "ExtTabLine";
        type Type = super::ExtTabLine;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for ExtTabLine {}
    impl WidgetImpl for ExtTabLine {}
    impl BinImpl for ExtTabLine {}
}

glib::wrapper! {
    pub struct ExtTabLine(ObjectSubclass<imp::ExtTabLine>)
        @extends gtk::Widget;
}

struct HashItem {
    tabpage: nvim::Tabpage,
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

impl ExtTabLine {
    pub fn update_tabs(&self, tabline: &crate::nvim::ExtTabline) {
        let tab_view = self.imp().tab_view.get();

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
