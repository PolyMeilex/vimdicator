use gdk::subclass::prelude::ObjectSubclassIsExt;

mod imp {
    use adw::subclass::prelude::*;
    use gtk::{glib, CompositeTemplate};

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(file = "window.ui")]
    pub struct VimdicatorWindow {
        #[template_child]
        pub headerbar: TemplateChild<adw::HeaderBar>,

        #[template_child]
        pub dock: TemplateChild<libpanel::Dock>,

        #[template_child]
        pub main_panel: TemplateChild<libpanel::Paned>,
        #[template_child]
        pub start_panel: TemplateChild<libpanel::Paned>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for VimdicatorWindow {
        const NAME: &'static str = "VimdicatorWindow";
        type Type = super::VimdicatorWindow;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
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

    impl AdwWindowImpl for VimdicatorWindow {}
    impl AdwApplicationWindowImpl for VimdicatorWindow {}
}

glib::wrapper! {
    pub struct VimdicatorWindow(ObjectSubclass<imp::VimdicatorWindow>)
        @extends gtk::Widget, gtk::Window, gtk::ApplicationWindow,
        @implements gio::ActionMap, gio::ActionGroup;
}

impl VimdicatorWindow {
    pub fn new<P: glib::IsA<gtk::Application>>(app: &P) -> Self {
        glib::Object::builder().property("application", app).build()
    }

    pub fn header_bar(&self) -> adw::HeaderBar {
        self.imp().headerbar.get()
    }

    pub fn dock(&self) -> libpanel::Dock {
        self.imp().dock.get()
    }

    pub fn main_panel(&self) -> libpanel::Paned {
        self.imp().main_panel.get()
    }

    pub fn start_panel(&self) -> libpanel::Paned {
        self.imp().start_panel.get()
    }
}
