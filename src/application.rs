/* application.rs
 *
 * Copyright 2023 poly
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>.
 *
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

use adw::subclass::prelude::*;
use gtk::prelude::*;
use gtk::{gio, glib};
use std::cell::{Cell, OnceCell};
use std::rc::Rc;
use tokio::sync::mpsc::UnboundedSender;

use crate::config::VERSION;
use crate::{widgets, GtkToNvimEvent, NvimMouseAction, NvimMouseButton, VimdicatorWindow};

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
    window: VimdicatorWindow,
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

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub struct VimdicatorApplication {
        pub nvim_tx: OnceCell<UnboundedSender<GtkToNvimEvent>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for VimdicatorApplication {
        const NAME: &'static str = "VimdicatorApplication";
        type Type = super::VimdicatorApplication;
        type ParentType = adw::Application;
    }

    impl ObjectImpl for VimdicatorApplication {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            obj.setup_gactions();
            obj.set_accels_for_action("app.quit", &["<primary>q"]);
        }
    }

    impl ApplicationImpl for VimdicatorApplication {
        fn activate(&self) {
            let application = self.obj();

            let window = if let Some(window) = application.active_window() {
                window
            } else {
                let window = VimdicatorWindow::new(&*application);

                window
                    .ext_line_grid()
                    .set_nvim_tx(self.nvim_tx.get().unwrap().clone());

                let tx = self.nvim_tx.get().unwrap().clone();
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
                let nvim_tx = self.nvim_tx.get().unwrap();

                init_motion_controller(window.clone(), nvim_tx.clone(), state.clone());
                init_scroll_controller(window.ext_line_grid(), nvim_tx.clone(), state.clone());
                init_gesture_controller(window.ext_line_grid(), nvim_tx.clone(), state);

                window.upcast()
            };

            window.present();
        }
    }

    impl GtkApplicationImpl for VimdicatorApplication {}
    impl AdwApplicationImpl for VimdicatorApplication {}
}

glib::wrapper! {
    pub struct VimdicatorApplication(ObjectSubclass<imp::VimdicatorApplication>)
        @extends gio::Application, gtk::Application, adw::Application,
        @implements gio::ActionGroup, gio::ActionMap;
}

impl VimdicatorApplication {
    pub fn new(
        application_id: &str,
        flags: &gio::ApplicationFlags,
        input_tx: UnboundedSender<GtkToNvimEvent>,
    ) -> Self {
        let this: Self = glib::Object::builder()
            .property("application-id", application_id)
            .property("flags", flags)
            .build();

        this.imp().nvim_tx.set(input_tx).unwrap();

        this
    }

    fn setup_gactions(&self) {
        let sidebar_action = gio::ActionEntry::builder("toggle_sidebar")
            .activate(move |app: &Self, _, _| {
                app.imp()
                    .nvim_tx
                    .get()
                    .unwrap()
                    .send(GtkToNvimEvent::ExecLua(
                        r#"require("nvim-tree.api").tree.toggle()"#.to_string(),
                    ))
                    .unwrap();
            })
            .build();
        let quit_action = gio::ActionEntry::builder("quit")
            .activate(move |app: &Self, _, _| app.quit())
            .build();
        let about_action = gio::ActionEntry::builder("about")
            .activate(move |app: &Self, _, _| app.show_about())
            .build();
        self.add_action_entries([quit_action, about_action, sidebar_action]);
    }

    fn show_about(&self) {
        let window = self.active_window().unwrap();
        let about = adw::AboutWindow::builder()
            .transient_for(&window)
            .application_name("vimdicator")
            .application_icon("io.github.polymeilex.vimdicator")
            .developer_name("poly")
            .version(VERSION)
            .developers(vec!["poly"])
            .copyright("© 2023 poly")
            .build();

        about.present();
    }
}
