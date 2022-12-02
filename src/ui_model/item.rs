use std::cell::*;

use crate::color;

use gsk::{
    self,
    graphene,
};
use pango;

#[derive(Clone)]
pub struct Item {
    pub item: pango::Item,
    pub cells_count: usize,
    glyphs: RefCell<Option<pango::GlyphString>>,
    render_node: RefCell<Option<gsk::TextNode>>,
    font: pango::Font,
}

impl Item {
    pub fn new(item: pango::Item, cells_count: usize) -> Self {
        debug_assert!(cells_count > 0);

        Item {
            font: item.analysis().font(),
            item,
            cells_count,
            glyphs: RefCell::new(None),
            render_node: RefCell::new(None),
        }
    }

    pub fn glyphs(&self) -> Ref<Option<pango::GlyphString>> {
        self.glyphs.borrow()
    }

    pub fn set_glyphs(&mut self, glyphs: pango::GlyphString) {
        *self.glyphs.borrow_mut() = Some(glyphs);
        *self.render_node.borrow_mut() = None;
    }

    pub fn render_node(&self, color: &color::Color, (x, y): (f32, f32)) -> gsk::TextNode {
        let mut render_node = self.render_node.borrow_mut();
        if render_node.is_none() {
            *render_node = gsk::TextNode::new(
                &self.font,
                self.glyphs.borrow_mut().as_mut().unwrap(),
                &color.into(),
                &graphene::Point::new(x, y)
            );
        }

        render_node.as_ref().expect("Failed to create render node").clone()
    }

    pub fn new_render_node(&self, color: &color::Color, (x, y): (f32, f32)) -> gsk::TextNode {
        gsk::TextNode::new(
            &self.font,
            &mut self.glyphs().as_ref().unwrap().clone(),
            &color.into(),
            &graphene::Point::new(x, y)
        ).expect("Failed to create render node")
    }

    pub fn font(&self) -> &pango::Font {
        &self.font
    }

    pub fn analysis(&self) -> &pango::Analysis {
        self.item.analysis()
    }
}
