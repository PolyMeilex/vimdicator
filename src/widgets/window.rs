use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gio, glib};

use crate::widgets;

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
        pub main_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub ext_line_grid: TemplateChild<widgets::ExtLineGrid>,
        #[template_child]
        pub popover: TemplateChild<gtk::Popover>,
        #[template_child]
        pub ext_popup_menu: TemplateChild<widgets::ExtPopupMenu>,
        #[template_child]
        pub ext_tabline: TemplateChild<widgets::ExtTabLine>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for VimdicatorWindow {
        const NAME: &'static str = "VimdicatorWindow";
        type Type = super::VimdicatorWindow;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            widgets::ExtTabLine::static_type();
            widgets::ExtPopupMenu::static_type();
            widgets::ExtLineGrid::static_type();
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

    pub fn ext_line_grid(&self) -> widgets::ExtLineGrid {
        self.imp().ext_line_grid.clone()
    }

    pub fn main_box(&self) -> gtk::Box {
        self.imp().main_box.clone()
    }

    pub fn popover(&self) -> gtk::Popover {
        self.imp().popover.clone()
    }

    pub fn ext_popup_menu(&self) -> widgets::ExtPopupMenu {
        self.imp().ext_popup_menu.get()
    }

    pub fn ext_tabline(&self) -> widgets::ExtTabLine {
        self.imp().ext_tabline.get()
    }
}
