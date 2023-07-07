use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{
    gdk, glib,
    graphene::{self},
    gsk, pango,
};
use std::cell::{OnceCell, RefCell};
use tokio::sync::mpsc::UnboundedSender;

use crate::GtkToNvimEvent;

#[derive(Debug)]
pub struct CellMetrics {
    pub line_height: f64,
    pub char_width: f64,
    pub ascent: f64,
    pub descent: f64,
    pub underline_position: f64,
    pub underline_thickness: f64,
    pub strikethrough_position: f64,
    pub strikethrough_thickness: f64,
    pub pango_ascent: i32,
    pub pango_descent: i32,
    pub pango_char_width: i32,
}

impl CellMetrics {
    fn new(font_metrics: &pango::FontMetrics, line_space: i32) -> Self {
        let ascent = (f64::from(font_metrics.ascent()) / f64::from(pango::SCALE)).ceil();
        let descent = (f64::from(font_metrics.descent()) / f64::from(pango::SCALE)).ceil();

        // distance above top of underline, will typically be negative
        let pango_underline_position = f64::from(font_metrics.underline_position());
        let underline_position = (pango_underline_position / f64::from(pango::SCALE))
            .abs()
            .ceil()
            .copysign(pango_underline_position);

        let underline_thickness =
            (f64::from(font_metrics.underline_thickness()) / f64::from(pango::SCALE)).ceil();

        let strikethrough_position =
            (f64::from(font_metrics.strikethrough_position()) / f64::from(pango::SCALE)).ceil();
        let strikethrough_thickness =
            (f64::from(font_metrics.strikethrough_thickness()) / f64::from(pango::SCALE)).ceil();

        CellMetrics {
            pango_ascent: font_metrics.ascent(),
            pango_descent: font_metrics.descent(),
            pango_char_width: font_metrics.approximate_char_width(),
            ascent,
            descent,
            line_height: ascent + descent + f64::from(line_space),
            char_width: f64::from(font_metrics.approximate_char_width()) / f64::from(pango::SCALE),
            underline_position: ascent - underline_position + underline_thickness / 2.0,
            underline_thickness,
            strikethrough_position: ascent - strikethrough_position + strikethrough_thickness / 2.0,
            strikethrough_thickness,
        }
    }

    // Translate the given grid coordinates into their actual pixel coordinates
    pub fn pixel_coords(&self, (col, row): (usize, usize)) -> (f64, f64) {
        (self.char_width * col as f64, self.line_height * row as f64)
    }
}

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub struct ExtLineGrid {
        pub grid: RefCell<Option<crate::nvim::ExtLineGrid>>,
        pub nvim_tx: OnceCell<UnboundedSender<GtkToNvimEvent>>,
        pub context: OnceCell<pango::Context>,
        pub cell_metrics: OnceCell<CellMetrics>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ExtLineGrid {
        const NAME: &'static str = "ExtLineGrid";
        type Type = super::ExtLineGrid;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for ExtLineGrid {
        fn constructed(&self) {
            self.obj().set_widget_name("ext_line_grid");

            // let desc = pango::FontDescription::from_string("Sans Mono 12");
            let desc = pango::FontDescription::from_string("Source Code Pro 11");

            let context = self.obj().create_pango_context();
            context.set_font_description(Some(&desc));

            let font_metrics = context.metrics(Some(&desc), None);

            self.cell_metrics
                .set(CellMetrics::new(&font_metrics, 0))
                .unwrap();
            self.context.set(context).unwrap();
        }
    }

    impl WidgetImpl for ExtLineGrid {
        fn snapshot(&self, snapshot_in: &gtk::Snapshot) {
            let width = self.obj().width();
            let height = self.obj().height();

            let context = self.context.get().unwrap();
            let cell_metrics = self.cell_metrics.get().unwrap();

            let grid = self.grid.borrow();

            if let Some(tx) = self.nvim_tx.get() {
                let width = width as f64;
                let height = height as f64;

                let width = width / cell_metrics.char_width;
                let height = height / cell_metrics.line_height;

                let width = width.trunc() as u64;
                let height = height.trunc() as u64;

                if let Some(grid) = grid.as_ref() {
                    if width as usize != grid.columns() || height as usize != grid.rows() {
                        tx.send(GtkToNvimEvent::Resized { width, height }).unwrap();
                    }
                } else {
                    tx.send(GtkToNvimEvent::Resized { width, height }).unwrap();
                }
            }

            let Some(grid) = grid.as_ref() else { return; };

            let mut y = 0.0;
            for line in grid.buffer().iter() {
                let line: String = line.columns().iter().collect();

                let s = &line;
                let items =
                    pango::itemize(context, s, 0, s.len() as i32, &pango::AttrList::new(), None);
                let mut glyphs = pango::GlyphString::new();

                for item in items {
                    let analysis = item.analysis();
                    let font = analysis.font();
                    let offset = item.offset() as usize;
                    let length = item.length() as usize;

                    if let Some(line_str) = s.get(offset..offset + length) {
                        pango::shape(line_str, analysis, &mut glyphs);
                    }

                    let line_height = cell_metrics.line_height;
                    let ascent = cell_metrics.ascent;

                    let render_node = gsk::TextNode::new(
                        &font,
                        &glyphs,
                        &gdk::RGBA::new(53.0 / 255.0, 132.0 / 255.0, 228.0 / 255.0, 1.0),
                        &graphene::Point::new(0.0, y + ascent as f32),
                    );

                    y += line_height as f32;

                    if let Some(render_node) = render_node {
                        snapshot_in.append_node(&render_node);
                    }
                }
            }

            let pos = grid.cursor_position();

            let (x, y) = self
                .cell_metrics
                .get()
                .unwrap()
                .pixel_coords((pos.column, pos.row));

            snapshot_in.append_color(
                &gdk::RGBA::new(1.0, 1.0, 1.0, 0.1),
                &graphene::Rect::new(
                    x as f32,
                    y as f32,
                    cell_metrics.char_width as f32,
                    cell_metrics.line_height as f32,
                ),
            );
        }
    }
    impl BinImpl for ExtLineGrid {}
}

glib::wrapper! {
    pub struct ExtLineGrid(ObjectSubclass<imp::ExtLineGrid>)
        @extends gtk::Widget;
}

// impl Default for ExtLineGrid {
//     fn default() -> Self {
//         Self::new()
//     }
// }

impl ExtLineGrid {
    // pub fn new() -> Self {
    //     let this: Self = glib::Object::builder().build();
    //     this
    // }

    pub fn set_nvim_tx(&self, tx: UnboundedSender<GtkToNvimEvent>) {
        self.imp().nvim_tx.set(tx).unwrap();
    }

    pub fn set_grid(&self, grid: crate::nvim::ExtLineGrid) {
        *self.imp().grid.borrow_mut() = Some(grid);
        self.queue_draw();
    }

    pub fn cell_metrics(&self) -> &CellMetrics {
        self.imp().cell_metrics.get().unwrap()
    }
}
