mod viewport;

use std::cell::RefCell;
use std::cmp::{max, min};
use std::iter;
use std::rc::Rc;
use std::sync::Arc;

use glib::clone;
use log::error;

use gtk::prelude::*;

use unicode_segmentation::UnicodeSegmentation;

use crate::cursor;
use crate::highlight::HighlightMap;
use crate::mode;
use crate::nvim::{self, *};
use crate::nvim_viewport::NvimViewport;
use crate::popup_menu;
use crate::render::{self, CellMetrics};
use crate::shell;
use crate::ui::UiMutex;
use crate::ui_model::{HighlightedLine, HighlightedRange, ModelLayout};

use viewport::CmdlineViewport;

pub struct Level {
    model_layout: ModelLayout,
    prompt_offset: usize,
    preferred_width: i32,
    preferred_height: i32,
}

impl Level {
    pub fn insert(&mut self, c: String, shift: bool, render_state: &shell::RenderState) {
        self.model_layout
            .insert_char(c, shift, render_state.hl.default_hl());
        self.update_preferred_size(render_state);
    }

    pub fn replace_from_ctx(&mut self, ctx: &CmdLineContext, render_state: &shell::RenderState) {
        let content = ctx.get_lines(&render_state.hl);
        self.replace_line(content.lines, false);
        self.prompt_offset = content.prompt_offset;
        self.model_layout
            .set_cursor(self.prompt_offset + ctx.pos as usize);
        self.update_preferred_size(render_state);
    }

    pub fn from_ctx(ctx: &CmdLineContext, render_state: &shell::RenderState) -> Self {
        let content = ctx.get_lines(&render_state.hl);
        let mut level = Level::from_lines(content.lines, ctx.max_width, render_state);

        level.prompt_offset = content.prompt_offset;
        level
            .model_layout
            .set_cursor(level.prompt_offset + ctx.pos as usize);
        level.update_preferred_size(render_state);

        level
    }

    fn replace_line(&mut self, lines: Vec<HighlightedLine>, append: bool) {
        if append {
            self.model_layout.layout_append(lines);
        } else {
            self.model_layout.layout(lines);
        }
    }

    fn update_preferred_size(&mut self, render_state: &shell::RenderState) {
        let &CellMetrics {
            line_height,
            char_width,
            ..
        } = render_state.font_ctx.cell_metrics();

        let (columns, rows) = self.model_layout.size();
        let columns = max(columns, 5);

        self.preferred_width = (char_width * columns as f64) as i32;
        self.preferred_height = (line_height * rows as f64) as i32;
    }

    pub fn from_multiline_content(
        content: &Vec<Vec<(u64, String)>>,
        max_width: i32,
        render_state: &shell::RenderState,
    ) -> Self {
        let lines = content.to_attributed_content(&render_state.hl);
        Level::from_lines(lines, max_width, render_state)
    }

    pub fn from_lines(
        lines: Vec<HighlightedLine>,
        max_width: i32,
        render_state: &shell::RenderState,
    ) -> Self {
        let &CellMetrics { char_width, .. } = render_state.font_ctx.cell_metrics();

        let max_width_chars = (max_width as f64 / char_width) as u64;

        let mut model_layout = ModelLayout::new(max_width_chars);
        model_layout.layout(lines);

        let mut level = Level {
            model_layout,
            preferred_width: -1,
            preferred_height: -1,
            prompt_offset: 0,
        };

        level.update_preferred_size(render_state);
        level
    }

    fn update_cache(&mut self, render_state: &shell::RenderState) {
        render::shape_dirty(
            &render_state.font_ctx,
            &mut self.model_layout.model,
            &render_state.hl,
        );
    }

    fn set_cursor(&mut self, render_state: &shell::RenderState, pos: usize) {
        self.model_layout.set_cursor(self.prompt_offset + pos);
        self.update_preferred_size(render_state);
    }
}

fn prompt_lines(
    firstc: &str,
    prompt: &str,
    indent: u64,
    hl: &HighlightMap,
) -> (usize, Vec<HighlightedRange>) {
    let prompt: Vec<HighlightedRange> = if !firstc.is_empty() {
        if firstc.len() >= indent as usize {
            vec![HighlightedRange {
                highlight: hl.default_hl(),
                graphemes: vec![firstc.to_owned()],
            }]
        } else {
            vec![HighlightedRange::new(
                hl.default_hl(),
                iter::once(firstc.to_owned())
                    .chain((firstc.len()..indent as usize).map(|_| " ".to_owned()))
                    .collect(),
            )]
        }
    } else if !prompt.is_empty() {
        prompt
            .lines()
            .map(|l| HighlightedRange {
                highlight: hl.default_hl(),
                graphemes: l.graphemes(true).map(|g| g.to_owned()).collect(),
            })
            .collect()
    } else {
        vec![]
    };

    let prompt_offset = prompt
        .last()
        .map(|range| range.graphemes.len())
        .unwrap_or(0);

    (prompt_offset, prompt)
}

pub struct State {
    nvim: Option<Rc<nvim::NeovimClient>>,
    levels: Vec<Level>,
    block: Option<Level>,
    render_state: Rc<RefCell<shell::RenderState>>,
    viewport: CmdlineViewport,
    cursor: Option<cursor::Cursor<State>>,
}

impl State {
    fn new(viewport: CmdlineViewport, render_state: Rc<RefCell<shell::RenderState>>) -> Self {
        State {
            nvim: None,
            levels: Vec::new(),
            block: None,
            render_state,
            viewport,
            cursor: None,
        }
    }

    fn request_area_size(&self) {
        let block = self.block.as_ref();
        let level = self.levels.last();

        let (block_width, block_height) = block
            .map(|b| (b.preferred_width, b.preferred_height))
            .unwrap_or((0, 0));
        let (level_width, level_height) = level
            .map(|l| (l.preferred_width, l.preferred_height))
            .unwrap_or((0, 0));

        self.viewport.set_size_request(
            max(level_width, block_width),
            max(block_height + level_height, 40),
        );
        self.viewport.clear_snapshot_cache();
    }

    fn preferred_height(&self) -> i32 {
        let level = self.levels.last();
        level.map(|l| l.preferred_height).unwrap_or(0)
            + self.block.as_ref().map(|b| b.preferred_height).unwrap_or(0)
    }

    fn set_cursor(&mut self, render_state: &shell::RenderState, pos: usize, level: usize) {
        debug_assert!(level > 0);

        if let Some(l) = self.levels.get_mut(level - 1) {
            l.set_cursor(render_state, pos)
        }

        self.viewport.queue_draw();
    }
}

impl cursor::CursorRedrawCb for State {
    fn queue_redraw_cursor(&mut self) {
        self.viewport.queue_draw();
    }
}

pub struct CmdLine {
    popover: gtk::Popover,
    wild_tree: gtk::TreeView,
    wild_scroll: gtk::ScrolledWindow,
    wild_css_provider: gtk::CssProvider,
    wild_renderer: gtk::CellRendererText,
    wild_column: gtk::TreeViewColumn,
    displayed: bool,
    state: Arc<UiMutex<State>>,
}

impl CmdLine {
    pub fn new(
        nvim_viewport: &NvimViewport,
        render_state: Rc<RefCell<shell::RenderState>>,
    ) -> Self {
        let popover = gtk::Popover::new();
        popover.set_autohide(false);
        popover.set_position(gtk::PositionType::Right);
        nvim_viewport.set_ext_cmdline(&popover);
        popover.add_css_class("nvim-popover");

        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);

        let viewport = CmdlineViewport::new();
        content.append(&viewport);

        let state = Arc::new(UiMutex::new(State::new(viewport.clone(), render_state)));
        let weak_cb = Arc::downgrade(&state);
        let cursor = cursor::Cursor::new(weak_cb);
        state.borrow_mut().cursor = Some(cursor);

        viewport.set_state(&state);

        let (wild_scroll, wild_tree, wild_css_provider, wild_renderer, wild_column) =
            CmdLine::create_wildmenu(&state);
        content.append(&wild_scroll);
        popover.set_child(Some(&content));

        CmdLine {
            popover,
            state,
            displayed: false,
            wild_scroll,
            wild_tree,
            wild_css_provider,
            wild_renderer,
            wild_column,
        }
    }

    fn create_wildmenu(
        state: &Arc<UiMutex<State>>,
    ) -> (
        gtk::ScrolledWindow,
        gtk::TreeView,
        gtk::CssProvider,
        gtk::CellRendererText,
        gtk::TreeViewColumn,
    ) {
        let css_provider = gtk::CssProvider::new();

        let tree = gtk::TreeView::new();
        let style_context = tree.style_context();
        style_context.add_provider(&css_provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);

        tree.selection().set_mode(gtk::SelectionMode::Single);
        tree.set_headers_visible(false);
        tree.set_focusable(false);

        let renderer = gtk::CellRendererText::new();
        renderer.set_ellipsize(pango::EllipsizeMode::End);

        let column = gtk::TreeViewColumn::new();
        column.pack_start(&renderer, true);
        column.add_attribute(&renderer, "text", 0);
        tree.append_column(&column);

        let scroll = gtk::ScrolledWindow::new();
        scroll.set_propagate_natural_height(true);
        scroll.set_propagate_natural_width(true);
        scroll.set_visible(false);

        scroll.set_child(Some(&tree));

        let controller = gtk::GestureClick::builder().button(1).build();
        controller.connect_pressed(clone!(@strong state => move |controller, _, x, y| {
            let state = state.borrow_mut();
            let nvim = state.nvim.as_ref().unwrap().nvim();
            let tree = controller.widget().downcast().unwrap();
            if let Some(nvim) = nvim {
                tree_button_press(&tree, x, y, &nvim, "");
            }
        }));
        tree.add_controller(controller);

        (scroll, tree, css_provider, renderer, column)
    }

    pub fn show_level(&mut self, ctx: &CmdLineContext) {
        let mut state = self.state.borrow_mut();
        if state.nvim.is_none() {
            state.nvim = Some(ctx.nvim.clone());
        }
        let render_state = state.render_state.clone();
        let render_state = render_state.borrow();

        if ctx.level_idx as usize == state.levels.len() {
            let level = state.levels.last_mut().unwrap();
            level.replace_from_ctx(ctx, &render_state);
            level.update_cache(&render_state);
        } else {
            let mut level = Level::from_ctx(ctx, &render_state);
            level.update_cache(&render_state);
            state.levels.push(level);
        }

        state.request_area_size();

        if !self.displayed {
            self.displayed = true;
            self.popover.set_pointing_to(Some(&gdk::Rectangle::new(
                ctx.x, ctx.y, ctx.width, ctx.height,
            )));
            self.popover.popup();
            state.cursor.as_mut().unwrap().start();
        } else {
            state.viewport.queue_draw()
        }
    }

    pub fn special_char(
        &self,
        render_state: &shell::RenderState,
        c: String,
        shift: bool,
        level: u64,
    ) {
        let mut state = self.state.borrow_mut();

        if let Some(level) = state.levels.get_mut((level - 1) as usize) {
            level.insert(c, shift, render_state);
            level.update_cache(render_state);
        } else {
            error!("Level {} does not exists", level);
        }

        state.request_area_size();
        state.viewport.queue_draw()
    }

    pub fn hide_level(&mut self, level_idx: u64) {
        let mut state = self.state.borrow_mut();

        if level_idx as usize == state.levels.len() {
            state.levels.pop();
        }

        if state.levels.is_empty() {
            self.popover.hide();
            self.displayed = false;
        }
    }

    pub fn show_block(&mut self, content: &Vec<Vec<(u64, String)>>, max_width: i32) {
        let mut state = self.state.borrow_mut();
        let mut block =
            Level::from_multiline_content(content, max_width, &state.render_state.borrow());
        block.update_cache(&state.render_state.borrow());
        state.block = Some(block);
        state.request_area_size();
    }

    pub fn block_append(&mut self, content: &Vec<(u64, String)>) {
        let mut state = self.state.borrow_mut();
        let render_state = state.render_state.clone();
        {
            let attr_content = content.to_attributed_content(&render_state.borrow().hl);

            let block = state.block.as_mut().unwrap();
            block.replace_line(attr_content, true);
            block.update_preferred_size(&render_state.borrow());
            block.update_cache(&render_state.borrow());
        }
        state.request_area_size();
    }

    pub fn block_hide(&self) {
        self.state.borrow_mut().block = None;
    }

    pub fn pos(&self, render_state: &shell::RenderState, pos: u64, level: u64) {
        self.state
            .borrow_mut()
            .set_cursor(render_state, pos as usize, level as usize);
    }

    pub fn set_mode_info(&self, mode_info: Option<mode::ModeInfo>) {
        self.state
            .borrow_mut()
            .cursor
            .as_mut()
            .unwrap()
            .set_mode_info(mode_info);
    }

    pub fn show_wildmenu(
        &self,
        items: Vec<String>,
        render_state: &shell::RenderState,
        max_width: i32,
    ) {
        // update font/color
        self.wild_renderer.set_font(Some(
            render_state
                .font_ctx
                .font_description()
                .to_string()
                .as_str(),
        ));

        self.wild_renderer
            .set_foreground_rgba(Some(&render_state.hl.pmenu_fg().into()));

        update_css(&self.wild_css_provider, &render_state.hl);

        // set width
        // this calculation produce width more then needed, but this is looks ok :)
        let max_item_width = (items.iter().map(|item| item.len()).max().unwrap() as f64
            * render_state.font_ctx.cell_metrics().char_width) as i32
            + self.state.borrow().levels.last().unwrap().preferred_width;
        self.wild_column
            .set_fixed_width(min(max_item_width, max_width));
        self.wild_scroll.set_max_content_width(max_width);

        // set height
        let treeview_height =
            calc_treeview_height(&self.wild_tree, &self.wild_renderer, items.len());

        // load data
        let list_store = gtk::ListStore::new(&[glib::Type::STRING; 1]);
        for item in items {
            list_store.insert_with_values(None, &[(0, &item)]);
        }
        self.wild_tree.set_model(Some(&list_store));

        self.wild_scroll.set_max_content_height(treeview_height);

        self.wild_scroll.show();
    }

    pub fn hide_wildmenu(&self) {
        self.wild_scroll.hide();
    }

    pub fn wildmenu_select(&self, selected: i64) {
        if selected >= 0 {
            let wild_tree = self.wild_tree.clone();
            glib::idle_add_local_once(move || {
                let selected_path = gtk::TreePath::from_string(&format!("{selected}")).unwrap();
                wild_tree.selection().select_path(&selected_path);
                wild_tree.scroll_to_cell(
                    Some(&selected_path),
                    Option::<&gtk::TreeViewColumn>::None,
                    false,
                    0.0,
                    0.0,
                );
            });
        } else {
            self.wild_tree.selection().unselect_all();
        }
    }
}

pub struct CmdLineContext<'a> {
    pub nvim: &'a Rc<NeovimClient>,
    pub content: Vec<(u64, String)>,
    pub pos: u64,
    pub firstc: String,
    pub prompt: String,
    pub indent: u64,
    pub level_idx: u64,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub max_width: i32,
}

impl<'a> CmdLineContext<'a> {
    fn get_lines(&self, hl: &HighlightMap) -> LineContent {
        let mut content_line = self.content.to_attributed_content(hl);
        let (prompt_offset, prompt_lines) =
            prompt_lines(&self.firstc, &self.prompt, self.indent, hl);

        let mut content: Vec<_> = prompt_lines.into_iter().map(|line| vec![line]).collect();

        if content.is_empty() {
            content.push(content_line.remove(0));
        } else if let Some(line) = content.last_mut() {
            line.extend(content_line.remove(0))
        }

        LineContent {
            lines: content,
            prompt_offset,
        }
    }
}

struct LineContent {
    lines: Vec<HighlightedLine>,
    prompt_offset: usize,
}

trait ToAttributedModelContent {
    fn to_attributed_content(&self, hl: &HighlightMap) -> Vec<HighlightedLine>;
}

impl ToAttributedModelContent for Vec<Vec<(u64, String)>> {
    fn to_attributed_content(&self, hl: &HighlightMap) -> Vec<HighlightedLine> {
        self.iter()
            .map(|line_chars| {
                line_chars
                    .iter()
                    .map(|c| HighlightedRange {
                        highlight: hl.get(c.0.into()),
                        graphemes: c.1.graphemes(true).map(|g| g.to_owned()).collect(),
                    })
                    .collect()
            })
            .collect()
    }
}

impl ToAttributedModelContent for Vec<(u64, String)> {
    fn to_attributed_content(&self, hl: &HighlightMap) -> Vec<HighlightedLine> {
        vec![self
            .iter()
            .map(|c| HighlightedRange {
                highlight: hl.get(c.0.into()),
                graphemes: c.1.graphemes(true).map(|g| g.to_owned()).collect(),
            })
            .collect()]
    }
}

pub fn calc_treeview_height(
    tree: &gtk::TreeView,
    renderer: &gtk::CellRendererText,
    item_count: usize,
) -> i32 {
    let (_, natural_size) = renderer.preferred_height(tree);
    let (_, ypad) = renderer.padding();

    let row_height = natural_size + ypad;

    row_height * min(item_count, popup_menu::MAX_VISIBLE_ROWS as usize) as i32
}

pub fn tree_button_press(
    tree: &gtk::TreeView,
    x: f64,
    y: f64,
    nvim: &NvimSession,
    last_command: &str,
) {
    let (paths, ..) = tree.selection().selected_rows();
    let selected_idx = if !paths.is_empty() {
        let ids = paths[0].indices();
        if !ids.is_empty() {
            ids[0]
        } else {
            -1
        }
    } else {
        -1
    };

    if let Some((Some(tree_path), ..)) = tree.path_at_pos(x as i32, y as i32) {
        let target_idx = tree_path.indices()[0];

        let scroll_count = popup_menu::find_scroll_count(selected_idx, target_idx);

        let apply_command: String = if target_idx > selected_idx {
            (0..scroll_count)
                .map(|_| "<C-n>")
                .chain(iter::once(last_command))
                .collect()
        } else {
            (0..scroll_count)
                .map(|_| "<C-p>")
                .chain(iter::once(last_command))
                .collect()
        };

        nvim.block_timeout(nvim.input(&apply_command)).report_err();
    }
}

fn update_css(css_provider: &gtk::CssProvider, hl: &HighlightMap) {
    let bg = hl.pmenu_bg_sel();
    let fg = hl.pmenu_fg_sel();

    css_provider.load_from_data(&format!(
        ".view :selected {{ color: {}; background-color: {};}}\n
                .view {{ background-color: {}; }}",
        fg.to_hex(),
        bg.to_hex(),
        hl.pmenu_bg().to_hex(),
    ));
}
