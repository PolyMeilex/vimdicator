use gio::{prelude::*, subclass::prelude::*};
use gtk::{gio, glib};

use std::{cell::RefCell, convert::*};

use crate::nvim::event::PopupMenuItem;

glib::wrapper! {
    pub struct ExtPopupMenuModel(ObjectSubclass<imp::ExtPopupMenuModel>)
        @implements gio::ListModel;
}

impl ExtPopupMenuModel {
    pub fn new(items: Vec<PopupMenuItem>) -> Self {
        let this: Self = glib::Object::builder::<Self>().build();
        this.set_items(items);
        this
    }

    pub fn items(&self) -> std::cell::Ref<Vec<PopupMenuItem>> {
        self.imp().items.borrow()
    }

    pub fn set_items(&self, items: Vec<PopupMenuItem>) {
        let old_len = self.imp().items.borrow().len();
        let new_len = items.len();

        *self.imp().items.borrow_mut() = items;
        self.items_changed(0, old_len as u32, new_len as u32);
    }
}

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct ExtPopupMenuModel {
        pub items: RefCell<Vec<PopupMenuItem>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ExtPopupMenuModel {
        const NAME: &'static str = "NvimPopupMenuModel";
        type Type = super::ExtPopupMenuModel;
        type Interfaces = (gio::ListModel,);
    }

    impl ObjectImpl for ExtPopupMenuModel {}

    impl ListModelImpl for ExtPopupMenuModel {
        fn item(&self, position: u32) -> Option<glib::Object> {
            self.items
                .borrow()
                .get(position as usize)
                .cloned()
                .map(|item| glib::BoxedAnyObject::new(item).upcast())
        }

        fn n_items(&self) -> u32 {
            self.items.borrow().len().try_into().unwrap()
        }

        fn item_type(&self) -> glib::Type {
            glib::BoxedAnyObject::static_type()
        }
    }
}
