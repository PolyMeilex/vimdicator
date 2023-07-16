use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::glib;

mod model;
mod row;
use row::PopupMenuListRow;

use crate::nvim::event::PopupMenuItem;
use std::cell::{Cell, OnceCell};

mod imp {
    use super::*;

    #[derive(Debug, Default, gtk::CompositeTemplate)]
    #[template(
        resource = "/io/github/polymeilex/vimdicator/widgets/ext_popup_menu/ext_popup_menu.ui"
    )]
    pub struct ExtPopupMenu {
        pub selected: Cell<Option<usize>>,
        pub selection_model: OnceCell<gtk::SingleSelection>,
        pub items_model: OnceCell<model::ExtPopupMenuModel>,

        #[template_child]
        list_view: TemplateChild<gtk::ListView>,
    }

    impl ExtPopupMenu {
        pub fn set_items(&self, items: Vec<PopupMenuItem>) {
            self.items_model.get().unwrap().set_items(items);
        }

        pub fn select(&self, selected: Option<usize>) {
            self.selected.set(selected);

            let selection_model = self.selection_model.get().unwrap();
            if let Some(selected) = self.selected.get() {
                selection_model.select_item(selected as u32, true);

                let selected = selected as u32;

                let len = selection_model.n_items();
                let scrol_to = selected.min(len);

                self.list_view
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
        type ParentType = gtk::Popover;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
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

            self.list_view.set_model(Some(&list_model));

            let item_factory = gtk::SignalListItemFactory::new();

            item_factory.connect_setup(move |_, list_item| {
                list_item.set_child(Some(&PopupMenuListRow::new()));
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

            self.list_view.set_factory(Some(&item_factory));
        }
    }
    impl WidgetImpl for ExtPopupMenu {}
    impl PopoverImpl for ExtPopupMenu {}
}

glib::wrapper! {
    pub struct ExtPopupMenu(ObjectSubclass<imp::ExtPopupMenu>)
        @extends gtk::Widget, gtk::Popover;
}

impl ExtPopupMenu {
    pub fn set_items(&self, items: Vec<PopupMenuItem>) {
        self.imp().set_items(items);
    }

    pub fn select(&self, selected: Option<usize>) {
        self.imp().select(selected);
    }
}
