use gdk::subclass::prelude::ObjectSubclassIsExt;

mod imp {
    use adw::subclass::prelude::*;

    use glib::Properties;
    use glib::{ParamSpec, Value};
    use gtk::{glib, prelude::*, CompositeTemplate};
    use std::cell::RefCell;

    #[derive(Debug, Default, CompositeTemplate, Properties)]
    #[template(file = "headerbar.ui")]
    #[properties(wrapper_type = super::VimdicatorHeaderBar)]
    pub struct VimdicatorHeaderBar {
        #[template_child]
        pub headerbar: TemplateChild<adw::HeaderBar>,
        #[template_child]
        pub panel_toggle_button: TemplateChild<libpanel::ToggleButton>,

        #[property(get, set)]
        pub dock: RefCell<Option<libpanel::Dock>>,

        #[property(get, set)]
        pub title: RefCell<String>,
        #[property(get, set)]
        pub subtitle: RefCell<String>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for VimdicatorHeaderBar {
        const NAME: &'static str = "VimdicatorHeaderBar";
        type Type = super::VimdicatorHeaderBar;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for VimdicatorHeaderBar {
        fn properties() -> &'static [glib::ParamSpec] {
            Self::derived_properties()
        }

        fn set_property(&self, id: usize, value: &Value, pspec: &ParamSpec) {
            Self::derived_set_property(self, id, value, pspec)
        }

        fn property(&self, id: usize, pspec: &ParamSpec) -> Value {
            Self::derived_property(self, id, pspec)
        }
    }
    impl WidgetImpl for VimdicatorHeaderBar {}
    impl BinImpl for VimdicatorHeaderBar {}
}

glib::wrapper! {
    pub struct VimdicatorHeaderBar(ObjectSubclass<imp::VimdicatorHeaderBar>)
        @extends gtk::Widget;
}

impl VimdicatorHeaderBar {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    pub fn header_bar(&self) -> adw::HeaderBar {
        self.imp().headerbar.get()
    }
}
