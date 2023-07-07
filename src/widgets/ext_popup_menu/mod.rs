use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::glib;
use std::{cell::RefCell, rc::Rc};

mod model;
mod row;
use row::{PopupMenuListRow, PopupMenuListRowState};

use crate::nvim::event::PopupMenuItem;
use std::cell::{Cell, OnceCell};

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub struct ExtPopupMenu {
        pub selected: Cell<Option<usize>>,
        pub selection_model: OnceCell<gtk::SingleSelection>,
        pub items_model: OnceCell<model::ExtPopupMenuModel>,

        list_view: OnceCell<gtk::ListView>,
        scroll: OnceCell<gtk::ScrolledWindow>,
    }

    impl ExtPopupMenu {
        pub fn init(&self) {
            let list_model = self.selection_model.get().unwrap();

            if let Some(selected) = self.selected.get() {
                list_model.select_item(selected as u32, true);
            } else {
                list_model.unselect_all();
            }
        }

        pub fn set_items(&self, items: Vec<PopupMenuItem>) {
            self.items_model.get().unwrap().set_items(items);
        }

        pub fn select(&self, selected: Option<usize>) {
            self.selected.set(selected);

            let selection_model = self.selection_model.get().unwrap();
            if let Some(selected) = self.selected.get() {
                selection_model.select_item(selected as u32, true);

                let list_view = self.list_view.get().unwrap();
                let selected = selected as u32;

                let len = selection_model.n_items();
                let scrol_to = selected.min(len);

                list_view
                    .activate_action("list.scroll-to-item", Some(&scrol_to.to_variant()))
                    .unwrap();
            } else {
                selection_model.unselect_all();
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ExtPopupMenu {
        const NAME: &'static str = "ExtPopupMenu";
        type Type = super::ExtPopupMenu;
        type ParentType = adw::Bin;
    }

    impl ObjectImpl for ExtPopupMenu {
        fn constructed(&self) {
            self.obj().set_widget_name("ext_popup_menu");

            let model = model::ExtPopupMenuModel::new(vec![]);

            let list_model = gtk::SingleSelection::builder()
                .can_unselect(true)
                .autoselect(false)
                .model(&model)
                .build();

            self.items_model.set(model).unwrap();
            self.selection_model.set(list_model.clone()).unwrap();

            let list_view = gtk::ListView::builder()
                .show_separators(false)
                .single_click_activate(false)
                .model(&list_model)
                .build();

            let item_factory = gtk::SignalListItemFactory::new();
            let list_state: Rc<RefCell<PopupMenuListRowState>> = Default::default();

            item_factory.connect_setup(move |_, list_item| {
                list_item.set_child(Some(&PopupMenuListRow::new(&list_state)));
            });

            item_factory.connect_teardown(|_, list_item| {
                list_item.set_child(Option::<&gtk::Widget>::None);
            });

            item_factory.connect_bind(|_, list_item| {
                let row: PopupMenuListRow = list_item.child().unwrap().downcast().unwrap();
                row.set_row(
                    list_item
                        .item()
                        .map(|obj| {
                            obj.downcast::<glib::BoxedAnyObject>()
                                .unwrap()
                                .borrow::<PopupMenuItem>()
                                .clone()
                        })
                        .as_ref(),
                );
            });

            item_factory.connect_unbind(|_, list_item| {
                let row: PopupMenuListRow = list_item.child().unwrap().downcast().unwrap();
                row.set_row(Option::<&PopupMenuItem>::None);
            });

            list_view.set_factory(Some(&item_factory));

            let scroll = gtk::ScrolledWindow::builder().child(&list_view).build();

            self.list_view.set(list_view).unwrap();

            self.obj().set_child(Some(&scroll));

            self.scroll.set(scroll).unwrap();
        }
    }
    impl WidgetImpl for ExtPopupMenu {}
    impl BinImpl for ExtPopupMenu {}
}

glib::wrapper! {
    pub struct ExtPopupMenu(ObjectSubclass<imp::ExtPopupMenu>)
        @extends gtk::Widget, adw::Bin;
}

impl ExtPopupMenu {
    pub fn new(items: Vec<PopupMenuItem>, selected: Option<usize>) -> Self {
        let this: Self = glib::Object::builder().build();
        this.imp().set_items(items);
        this.imp().select(selected);
        this.imp().init();
        this
    }

    pub fn set_items(&self, items: Vec<PopupMenuItem>) {
        self.imp().set_items(items);
    }

    pub fn select(&self, selected: Option<usize>) {
        self.imp().select(selected);
    }
}
