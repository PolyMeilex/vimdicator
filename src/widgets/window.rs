use std::{cell::Cell, rc::Rc};

use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gio, glib};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    nvim::{GtkToNvimEvent, NvimMouseAction, NvimMouseButton},
    widgets,
};

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

    pub fn connect(&self, nvim_tx: UnboundedSender<GtkToNvimEvent>) {
        let window = self.clone();

        window.ext_line_grid().set_nvim_tx(nvim_tx.clone());

        let tx = nvim_tx.clone();
        let key_controller = gtk::EventControllerKey::new();
        key_controller.set_name(Some("vim"));
        key_controller.set_propagation_phase(gtk::PropagationPhase::Capture);
        key_controller.connect_key_pressed(move |_, key, _, modifiers| {
            use crate::input;

            let (inhibit, input) = input::gtk_key_press_to_vim_input(key, modifiers);

            if let Some(input) = input {
                tx.send(GtkToNvimEvent::Input(input)).unwrap();
            }

            inhibit
        });
        window.add_controller(key_controller);

        let state = Rc::new(MouseState::new());

        init_motion_controller(window.clone(), nvim_tx.clone(), state.clone());
        init_scroll_controller(window.ext_line_grid(), nvim_tx.clone(), state.clone());
        init_gesture_controller(window.ext_line_grid(), nvim_tx, state);
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

    pub fn ext_popup_menu(&self) -> widgets::ExtPopupMenu {
        self.imp().ext_popup_menu.get()
    }

    pub fn ext_tabline(&self) -> widgets::ExtTabLine {
        self.imp().ext_tabline.get()
    }
}

struct MouseState {
    pos: Cell<Option<(u64, u64)>>,
    is_pressed: Cell<bool>,
}

impl MouseState {
    fn new() -> Self {
        Self {
            pos: Cell::new(None),
            is_pressed: Cell::new(false),
        }
    }
}

fn init_motion_controller(
    window: widgets::VimdicatorWindow,
    tx: UnboundedSender<GtkToNvimEvent>,
    mouse_state: Rc<MouseState>,
) {
    let motion_controller = gtk::EventControllerMotion::new();

    motion_controller.connect_motion({
        let window = window.downgrade();

        move |controller, x, y| {
            let Some(window) = window.upgrade() else { return; };
            let ext_line_grid = window.ext_line_grid();

            let state = controller.current_event_state();
            let modifier = crate::input::keyval_to_input_string("", state);

            let pos = ext_line_grid.cell_metrics().cell_cords(x, y);

            if y < 0.0 {
                window.header_bar_revealer().set_reveal_child(true);
            } else {
                window.header_bar_revealer().set_reveal_child(false);
            }

            let pos = if Some(pos) != mouse_state.pos.get() {
                mouse_state.pos.set(Some(pos));
                Some(pos)
            } else {
                None
            };

            if pos.is_some() && mouse_state.is_pressed.get() {
                tx.send(GtkToNvimEvent::InputMouse {
                    button: NvimMouseButton::Left,
                    action: NvimMouseAction::Drag,
                    modifier,
                    grid: ext_line_grid.grid_id(),
                    pos,
                })
                .unwrap();
            }
        }
    });

    window.ext_line_grid().add_controller(motion_controller);
}

fn init_scroll_controller(
    ext_line_grid: widgets::ExtLineGrid,
    tx: UnboundedSender<GtkToNvimEvent>,
    mouse_state: Rc<MouseState>,
) {
    let scroll_controller = gtk::EventControllerScroll::new(
        gtk::EventControllerScrollFlags::VERTICAL | gtk::EventControllerScrollFlags::DISCRETE,
    );

    let grid = ext_line_grid.grid_id();
    scroll_controller.connect_scroll(move |controller, _dx, dy| {
        let dy = dy.round();

        let action = match dy.total_cmp(&0.0) {
            std::cmp::Ordering::Less => NvimMouseAction::Up,
            std::cmp::Ordering::Greater => NvimMouseAction::Down,
            std::cmp::Ordering::Equal => return gtk::Inhibit(false),
        };

        let state = controller.current_event_state();
        let modifier = crate::input::keyval_to_input_string("", state);

        let dy = dy.abs() as usize;

        let pos = mouse_state.pos.get();

        for _ in 0..dy {
            tx.send(GtkToNvimEvent::InputMouse {
                button: NvimMouseButton::Wheel,
                action,
                modifier: modifier.clone(),
                grid,
                pos,
            })
            .unwrap();
        }

        gtk::Inhibit(false)
    });

    ext_line_grid.add_controller(scroll_controller);
}

fn init_gesture_controller(
    ext_line_grid: widgets::ExtLineGrid,
    tx: UnboundedSender<GtkToNvimEvent>,
    mouse_state: Rc<MouseState>,
) {
    let click_controller = gtk::GestureClick::builder().n_points(1).button(0).build();

    click_controller.connect_pressed({
        let ext_line_grid = ext_line_grid.downgrade();
        let tx = tx.clone();
        let mouse_state = mouse_state.clone();

        move |controller, _, x, y| {
            let Some(ext_line_grid) = ext_line_grid.upgrade() else { return; };

            let btn = controller.current_button();
            let state = controller.current_event_state();

            let modifier = crate::input::keyval_to_input_string("", state);

            let pos = ext_line_grid.cell_metrics().cell_cords(x, y);
            mouse_state.pos.set(Some(pos));

            match btn {
                1 => {
                    mouse_state.is_pressed.set(true);

                    tx.send(GtkToNvimEvent::InputMouse {
                        button: NvimMouseButton::Left,
                        action: NvimMouseAction::Press,
                        modifier,
                        grid: ext_line_grid.grid_id(),
                        pos: Some(pos),
                    })
                    .unwrap();
                }
                _ => {}
            }
        }
    });

    click_controller.connect_released({
        let ext_line_grid = ext_line_grid.downgrade();
        let tx = tx;
        let mouse_state = mouse_state;

        move |controller, _, x, y| {
            let Some(ext_line_grid) = ext_line_grid.upgrade() else { return; };

            let btn = controller.current_button();
            let state = controller.current_event_state();

            let modifier = crate::input::keyval_to_input_string("", state);

            let pos = ext_line_grid.cell_metrics().cell_cords(x, y);
            mouse_state.pos.set(Some(pos));

            match btn {
                1 => {
                    mouse_state.is_pressed.set(false);

                    tx.send(GtkToNvimEvent::InputMouse {
                        button: NvimMouseButton::Left,
                        action: NvimMouseAction::Release,
                        modifier,
                        grid: ext_line_grid.grid_id(),
                        pos: Some(pos),
                    })
                    .unwrap();
                }
                _ => {}
            }
        }
    });

    ext_line_grid.add_controller(click_controller);
}
