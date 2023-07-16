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
use std::cell::OnceCell;
use tokio::sync::mpsc::UnboundedSender;

use crate::config::VERSION;
use crate::{nvim::GtkToNvimEvent, widgets};

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
                let window = widgets::VimdicatorWindow::new(&*application);
                window.connect(self.nvim_tx.get().unwrap().clone());
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
