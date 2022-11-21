use super::completion_model::CompleteItemRef;

use std::{
    cell::RefCell,
    convert::*,
    rc::*,
};

use lazy_static::lazy_static;

use gtk::{
    self,
    prelude::*,
    subclass::prelude::*,
};
use glib;

pub const PADDING: i32 = 2;

glib::wrapper! {
    pub struct CompletionListRow(ObjectSubclass<CompletionListRowObject>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible;
}

impl CompletionListRow {
    pub fn new(state: &Rc<RefCell<CompletionListRowState>>) -> Self {
        glib::Object::new(&[("state", &glib::BoxedAnyObject::new(state.clone()))])
            .expect("Failed to create CompletionListRow")
    }

    pub fn set_row(&self, row: Option<&CompleteItemRef>) {
        self.set_property("row", row.cloned().map(|r| glib::BoxedAnyObject::new(r)));
    }
}

/// For types that need inner mutability
#[derive(Default)]
struct CompletionListRowInner {
    row: Option<CompleteItemRef>,
    state: Rc<RefCell<CompletionListRowState>>,
}

#[derive(Default)]
pub struct CompletionListRowObject {
    inner: RefCell<CompletionListRowInner>,
    word_label: glib::WeakRef<gtk::Label>,
    kind_label: glib::WeakRef<gtk::Label>,
    menu_label: glib::WeakRef<gtk::Label>,
}

#[glib::object_subclass]
impl ObjectSubclass for CompletionListRowObject {
    const NAME: &'static str = "NvimCompletionListRow";
    type Type = CompletionListRow;
    type ParentType = gtk::Box;
}

impl ObjectImpl for CompletionListRowObject {
    fn constructed(&self, obj: &Self::Type) {
        self.parent_constructed(obj);

        let word_label = gtk::Label::builder()
            .single_line_mode(true)
            .ellipsize(pango::EllipsizeMode::Middle)
            .hexpand(true)
            .xalign(0.0)
            .build();
        self.word_label.set(Some(&word_label));
        obj.append(&word_label);

        let kind_label = gtk::Label::builder()
            .visible(false)
            .single_line_mode(true)
            .ellipsize(pango::EllipsizeMode::End)
            .hexpand(true)
            .xalign(0.0)
            .build();
        self.kind_label.set(Some(&kind_label));
        obj.append(&kind_label);

        let menu_label = gtk::Label::builder()
            .visible(false)
            .single_line_mode(true)
            .ellipsize(pango::EllipsizeMode::Middle)
            .hexpand(true)
            .xalign(0.0)
            .build();
        self.menu_label.set(Some(&menu_label));
        obj.append(&menu_label);
    }

    fn properties() -> &'static [glib::ParamSpec] {
        lazy_static! {
            static ref PROPERTIES: Vec<glib::ParamSpec> = vec![
                glib::ParamSpecObject::new(
                    "state",
                    "Completion list row state",
                    "A reference to the shared state structure for all CompletionListRow widgets",
                    glib::BoxedAnyObject::static_type(),
                    glib::ParamFlags::WRITABLE,
                ),
                glib::ParamSpecObject::new(
                    "row",
                    "Row",
                    "A reference to the current row we're displaying",
                    glib::BoxedAnyObject::static_type(),
                    glib::ParamFlags::READWRITE,
                ),
            ];
        }

        PROPERTIES.as_ref()
    }

    fn set_property(
        &self,
        _obj: &Self::Type,
        _id: usize,
        value: &glib::Value,
        pspec: &glib::ParamSpec
    ) {
        match pspec.name() {
            "row" => {
                let row = value
                    .get_owned::<Option<glib::BoxedAnyObject>>()
                    .unwrap()
                    .map(|o| o.borrow::<CompleteItemRef>().clone());

                if let Some(ref row) = row {
                    let inner = self.inner.borrow();
                    let state = inner.state.borrow();
                    let word_label = self.word_label.upgrade().unwrap();
                    word_label.set_label(&row.word);
                    word_label.set_width_request(state.word_col_width);

                    let kind_label = self.kind_label.upgrade().unwrap();
                    kind_label.set_visible(state.kind_col_width.is_some());
                    kind_label.set_label(&row.kind);
                    if let Some(width) = state.kind_col_width {
                        kind_label.set_width_request(width);
                    }

                    let menu_label = self.menu_label.upgrade().unwrap();
                    menu_label.set_visible(state.menu_col_width.is_some());
                    menu_label.set_label(&row.menu);
                    if let Some(width) = state.menu_col_width {
                        menu_label.set_width_request(width);
                    }
                }

                self.inner.borrow_mut().row = row;
            },
            "state" => {
                self.inner.borrow_mut().state = value
                    .get_owned::<glib::BoxedAnyObject>()
                    .unwrap()
                    .borrow::<Rc<RefCell<CompletionListRowState>>>()
                    .clone();
            },
            _ => unreachable!(),
        }
    }

    fn property(&self, _obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "row" => {
                self
                    .inner
                    .borrow()
                    .row
                    .clone()
                    .map(|r| glib::BoxedAnyObject::new(r))
                    .to_value()
            }
            _ => unreachable!(),
        }
    }
}

impl WidgetImpl for CompletionListRowObject {}
impl BoxImpl for CompletionListRowObject {}

/// A state struct that is shared across all CompletionListRow widgets. It is provided at
/// construction
#[derive(Default)]
pub struct CompletionListRowState {
    pub word_col_width: i32,
    pub kind_col_width: Option<i32>,
    pub menu_col_width: Option<i32>,
}
