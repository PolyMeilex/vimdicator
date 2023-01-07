use std::cell::RefCell;
use std::ops::Deref;
use std::rc::Rc;

use gtk::prelude::*;

use crate::{
    nvim::{self, ErrorReport, Tabpage},
    spawn_timeout_user_err,
};

struct State {
    data: Vec<Tabpage>,
    selected: Option<Tabpage>,
    nvim: Option<Rc<nvim::NeovimClient>>,
}

impl State {
    pub fn new() -> Self {
        State {
            data: Vec::new(),
            selected: None,
            nvim: None,
        }
    }

    fn switch_page(&self, idx: u32) {
        let target = &self.data[idx as usize];
        if Some(target) != self.selected.as_ref() {
            if let Some(nvim) = self.nvim.as_ref().unwrap().nvim() {
                nvim.block_timeout(nvim.set_current_tabpage(target))
                    .report_err();
            }
        }
    }

    fn close_tab(&self, idx: u32) {
        if let Some(nvim) = self.nvim.as_ref().unwrap().nvim() {
            spawn_timeout_user_err!(nvim.command(&format!("tabc {}", idx + 1)));
        }
    }
}

pub struct Tabline {
    tabs: gtk::Notebook,
    state: Rc<RefCell<State>>,
    switch_handler_id: glib::SignalHandlerId,
}

impl Tabline {
    pub fn new() -> Self {
        let tabs = gtk::Notebook::builder()
            .can_focus(false)
            .scrollable(true)
            .show_border(false)
            .hexpand(true)
            .sensitive(false)
            .visible(false)
            .build();

        let state = Rc::new(RefCell::new(State::new()));

        let switch_handler_id =
            tabs.connect_switch_page(glib::clone!(@strong state => move |_, _, idx| {
                let state = state.borrow();
                state.switch_page(idx)
            }));

        Tabline {
            tabs,
            state,
            switch_handler_id,
        }
    }

    fn update_state(
        &self,
        nvim: &Rc<nvim::NeovimClient>,
        selected: &Tabpage,
        tabs: &[(Tabpage, Option<String>)],
    ) {
        let mut state = self.state.borrow_mut();

        if state.nvim.is_none() {
            state.nvim = Some(nvim.clone());
        }

        state.selected = Some(selected.clone());

        state.data = tabs.iter().map(|item| item.0.clone()).collect();
    }

    pub fn update_tabs(
        &self,
        nvim: &Rc<nvim::NeovimClient>,
        selected: &Tabpage,
        tabs: &[(Tabpage, Option<String>)],
    ) {
        if tabs.len() <= 1 {
            self.tabs.hide();
            return;
        } else {
            self.tabs.show();
        }

        self.update_state(nvim, selected, tabs);

        self.tabs.block_signal(&self.switch_handler_id);

        let count = self.tabs.n_pages() as usize;
        if count < tabs.len() {
            for _ in count..tabs.len() {
                let empty = gtk::Box::new(gtk::Orientation::Vertical, 0);
                let title = gtk::Label::builder()
                    .ellipsize(pango::EllipsizeMode::Middle)
                    .width_chars(25)
                    .hexpand(true)
                    .build();

                let close_btn = gtk::Button::from_icon_name("window-close-symbolic");
                close_btn.set_has_frame(false);
                close_btn.set_focus_on_click(false);

                let label_box = gtk::Box::builder()
                    .orientation(gtk::Orientation::Horizontal)
                    .hexpand(true)
                    .build();
                label_box.append(&title);
                label_box.append(&close_btn);

                self.tabs.append_page(&empty, Some(&label_box));

                let tabs = self.tabs.clone();
                let state_ref = Rc::clone(&self.state);
                close_btn.connect_clicked(move |btn| {
                    let current_label = btn.parent().unwrap();
                    for i in 0..tabs.n_pages() {
                        let page = tabs.nth_page(Some(i)).unwrap();
                        let label = tabs.tab_label(&page).unwrap();
                        if label == current_label {
                            state_ref.borrow().close_tab(i);
                        }
                    }
                });
            }
        } else if count > tabs.len() {
            for _ in tabs.len()..count {
                self.tabs.remove_page(None);
            }
        }

        for (idx, tab) in tabs.iter().enumerate() {
            let tab_child = self.tabs.nth_page(Some(idx as u32));
            let tab_label = self
                .tabs
                .tab_label(&tab_child.unwrap())
                .unwrap()
                .first_child()
                .unwrap()
                .downcast::<gtk::Label>()
                .unwrap();
            tab_label.set_text(tab.1.as_ref().unwrap_or(&"??".to_owned()));

            if *selected == tab.0 {
                self.tabs.set_current_page(Some(idx as u32));
            }
        }

        self.tabs.unblock_signal(&self.switch_handler_id);
    }
}

impl Deref for Tabline {
    type Target = gtk::Notebook;

    fn deref(&self) -> &gtk::Notebook {
        &self.tabs
    }
}
