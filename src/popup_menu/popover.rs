// Literally just a PopupMenu but we expose bounds_change. Yes, we know what this does and yes we do
// actually need it - we use it in order to report the full bounds of the popupmenu

use std::convert::*;

use lazy_static::lazy_static;

use gdk;
use glib::{self, prelude::*, SignalHandlerId};
use gtk::graphene::*;
use gtk::{self, prelude::*, subclass::prelude::*};

glib::wrapper! {
    pub struct PopupMenuPopover(ObjectSubclass<PopupMenuPopoverObject>)
        @extends gtk::Popover, gtk::Native, gtk::Widget;
}

impl PopupMenuPopover {
    pub fn new() -> Self {
        glib::Object::new::<Self>(&[])
    }

    pub fn connect_bounds_changed<F>(&self, cb: F) -> SignalHandlerId
    where
        F: Fn(&Self, i32, i32, i32, i32) + 'static,
    {
        self.connect_local("bounds-change", true, move |values| {
            cb(
                values[0].get().as_ref().unwrap(),
                values[1].get().unwrap(),
                values[2].get().unwrap(),
                values[3].get().unwrap(),
                values[4].get().unwrap(),
            );
            None
        })
    }
}

#[derive(Default)]
pub struct PopupMenuPopoverObject(());

#[glib::object_subclass]
impl ObjectSubclass for PopupMenuPopoverObject {
    const NAME: &'static str = "NvimPopupMenuPopover";
    type Type = PopupMenuPopover;
    type ParentType = gtk::Popover;
}

impl ObjectImpl for PopupMenuPopoverObject {
    fn signals() -> &'static [glib::subclass::Signal] {
        lazy_static! {
            static ref SIGNALS: Vec<glib::subclass::Signal> = vec![
                glib::subclass::Signal::builder("bounds-change")
                    .param_types(vec![
                        glib::Type::I32, // x
                        glib::Type::I32, // y
                        glib::Type::I32, // w
                        glib::Type::I32, // h
                    ])
                    .build(),
            ];
        }

        SIGNALS.as_ref()
    }
}
impl PopoverImpl for PopupMenuPopoverObject {}

impl WidgetImpl for PopupMenuPopoverObject {
    fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
        self.parent_size_allocate(width, height, baseline);

        let obj = self.obj();
        let gdk_popup = obj.surface().downcast::<gdk::Popup>().unwrap();

        obj.emit_by_name::<()>(
            "bounds-change",
            &[
                &gdk_popup.position_x(),
                &gdk_popup.position_y(),
                &width,
                &height,
            ],
        );
    }
}
