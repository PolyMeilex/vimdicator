use lazy_static::lazy_static;

use gio::{
    self,
    prelude::*,
    subclass::prelude::*,
};
use glib;

use std::{
    cell::RefCell,
    convert::*,
    rc::Rc,
    ops::Deref,
};

use crate::nvim::CompleteItem;

glib::wrapper! {
    pub struct CompletionModel(ObjectSubclass<CompletionModelObject>)
        @implements gio::ListModel;
}

impl CompletionModel {
    pub fn new(items: &Rc<Vec<CompleteItem>>) -> Self {
        glib::Object::new(&[("items", &glib::BoxedAnyObject::new(items.clone()))])
            .expect("Failed to create NvimCompletionModel")
    }
}

#[derive(Default)]
pub struct CompletionModelObject(RefCell<Rc<Vec<CompleteItem>>>);

#[glib::object_subclass]
impl ObjectSubclass for CompletionModelObject {
    const NAME: &'static str = "NvimCompletionModel";
    type Type = CompletionModel;
    type ParentType = glib::Object;
    type Interfaces = (gio::ListModel,);
}

impl ObjectImpl for CompletionModelObject {
    fn properties() -> &'static [glib::ParamSpec] {
        lazy_static! {
            static ref PROPERTIES: Vec<glib::ParamSpec> = vec![
                glib::ParamSpecObject::new(
                    "items",
                    "Completion items",
                    "A reference to the list of completion items",
                    glib::BoxedAnyObject::static_type(),
                    glib::ParamFlags::WRITABLE,
                )
            ];
        }

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _obj: &Self::Type, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "items" =>
                *self.0.borrow_mut() = value
                    .get::<glib::BoxedAnyObject>()
                    .unwrap()
                    .borrow::<Rc<Vec<CompleteItem>>>()
                    .clone(),
            _ => unreachable!(),
        }
    }
}

impl ListModelImpl for CompletionModelObject {
    fn item(&self, _list_model: &Self::Type, position: u32) -> Option<glib::Object> {
        let items = self.0.borrow();
        CompleteItemRef::new(&items, position as usize).map(|c| {
            glib::BoxedAnyObject::new(c).upcast()
        })
    }

    fn n_items(&self, _list_model: &Self::Type) -> u32 {
        self.0.borrow().len().try_into().unwrap()
    }

    fn item_type(&self, _list_model: &Self::Type) -> glib::Type {
        glib::BoxedAnyObject::static_type()
    }
}

#[derive(Clone, Default)]
pub struct CompleteItemRef {
    array: Rc<Vec<CompleteItem>>,
    pos: usize,
}

impl CompleteItemRef {
    pub fn new(array: &Rc<Vec<CompleteItem>>, pos: usize) -> Option<Self> {
        array.get(pos).map(|_| Self { array: array.clone(), pos })
    }
}

impl Deref for CompleteItemRef {
    type Target = CompleteItem;

    fn deref(&self) -> &Self::Target {
        // SAFETY: pos is checked at creation time
        unsafe { self.array.get_unchecked(self.pos) }
    }
}
