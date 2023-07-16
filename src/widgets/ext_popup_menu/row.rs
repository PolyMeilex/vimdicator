use crate::nvim::event::PopupMenuItem;

use gtk::{glib, pango};
use gtk::{prelude::*, subclass::prelude::*};

glib::wrapper! {
    pub struct PopupMenuListRow(ObjectSubclass<imp::PopupMenuListRow>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible;
}

impl Default for PopupMenuListRow {
    fn default() -> Self {
        Self::new()
    }
}

impl PopupMenuListRow {
    pub fn new() -> Self {
        glib::Object::builder::<Self>().build()
    }

    pub fn set_row(&self, row: Option<&PopupMenuItem>) {
        self.set_property("row", row.cloned().map(glib::BoxedAnyObject::new));
    }
}

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct PopupMenuListRow {
        word_label: glib::WeakRef<gtk::Label>,
        kind_label: glib::WeakRef<gtk::Label>,
        menu_label: glib::WeakRef<gtk::Label>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PopupMenuListRow {
        const NAME: &'static str = "NvimPopupMenuListRow";
        type Type = super::PopupMenuListRow;
        type ParentType = gtk::Box;
    }

    impl ObjectImpl for PopupMenuListRow {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();

            obj.set_widget_name("nvim_popup_menu_list_row");

            let word_label = gtk::Label::builder()
                .single_line_mode(true)
                .ellipsize(pango::EllipsizeMode::Middle)
                .xalign(0.0)
                .build();
            self.word_label.set(Some(&word_label));
            obj.append(&word_label);

            let kind_label = gtk::Label::builder()
                .visible(false)
                .single_line_mode(true)
                .ellipsize(pango::EllipsizeMode::End)
                .xalign(0.0)
                .build();
            self.kind_label.set(Some(&kind_label));
            obj.append(&kind_label);

            let menu_label = gtk::Label::builder()
                .visible(false)
                .single_line_mode(true)
                .ellipsize(pango::EllipsizeMode::Middle)
                .xalign(0.0)
                .build();
            self.menu_label.set(Some(&menu_label));
            obj.append(&menu_label);
        }

        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: std::sync::OnceLock<Vec<glib::ParamSpec>> =
                std::sync::OnceLock::new();

            PROPERTIES.get_or_init(|| {
                vec![
                    glib::ParamSpecObject::builder::<glib::BoxedAnyObject>("row")
                        .write_only()
                        .build(),
                ]
            })
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            match pspec.name() {
                "row" => {
                    let row = value.get_owned::<Option<glib::BoxedAnyObject>>().unwrap();

                    if let Some(row) = row {
                        let row = row.borrow::<PopupMenuItem>();
                        let word_label = self.word_label.upgrade().unwrap();
                        word_label.set_label(&row.word);

                        let kind_label = self.kind_label.upgrade().unwrap();
                        kind_label.set_visible(false);
                        kind_label.set_label(&row.kind);

                        let menu_label = self.menu_label.upgrade().unwrap();
                        menu_label.set_visible(false);
                        menu_label.set_label(&row.menu);
                    }
                }
                _ => unreachable!(),
            }
        }
    }

    impl WidgetImpl for PopupMenuListRow {}
    impl BoxImpl for PopupMenuListRow {}
}
