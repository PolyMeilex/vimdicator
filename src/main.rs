/* main.rs
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

#![allow(clippy::single_match)]

mod application;
mod config;
mod input;
mod nvim;
mod widgets;

use nvim::{ExtLineGridMap, ExtPopupMenu, ExtTabline, NvimEvent, RedrawEvent};

use application::VimdicatorApplication;
use config::{GETTEXT_PACKAGE, LOCALEDIR, PKGDATADIR};
use gettextrs::{bind_textdomain_codeset, bindtextdomain, textdomain};
use gtk::{gdk, gio, glib, prelude::*};
use std::collections::HashMap;

fn main() -> glib::ExitCode {
    glib_logger::init(&glib_logger::SIMPLE);
    log::set_max_level(log::LevelFilter::Debug);

    let (gtk_tx, gtk_rx) = glib::MainContext::channel::<NvimEvent>(glib::Priority::default());
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<nvim::GtkToNvimEvent>();

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.spawn(nvim::run(rx, gtk_tx));

    // Set up gettext translations
    bindtextdomain(GETTEXT_PACKAGE, LOCALEDIR).expect("Unable to bind the text domain");
    bind_textdomain_codeset(GETTEXT_PACKAGE, "UTF-8")
        .expect("Unable to set the text domain encoding");
    textdomain(GETTEXT_PACKAGE).expect("Unable to switch to the text domain");

    // Load resources
    let resources = gio::Resource::load(PKGDATADIR.to_owned() + "/vimdicator.gresource")
        .expect("Could not load resources");
    gio::resources_register(&resources);

    let app = VimdicatorApplication::new(
        "io.github.polymeilex.vimdicator",
        &gio::ApplicationFlags::empty(),
        tx,
    );

    gtk_rx.attach(None, {
        let app = app.clone();
        let mut grid_map = ExtLineGridMap::new();
        let mut popup_menu = ExtPopupMenu::new();
        let mut tabline = ExtTabline::new();
        let mut flush_state = FlushState::default();
        let mut style = HashMap::new();

        move |event| {
            if let Some(window) = app.active_window() {
                let window: widgets::VimdicatorWindow = window.downcast().unwrap();

                match event {
                    NvimEvent::Redraw(events) => {
                        let flushed = handle_redraw_event(
                            &mut style,
                            &mut flush_state,
                            &mut grid_map,
                            &mut popup_menu,
                            &mut tabline,
                            &events,
                        );

                        if flushed {
                            let grid_widget = window.ext_line_grid();

                            if let Some(grid) = grid_map.get_default() {
                                let mut grid = grid.clone();
                                grid.style = style.clone();
                                grid_widget.set_grid(grid);
                            }

                            if flush_state.tabline_changed {
                                window.ext_tabline().update_tabs(&tabline);
                            }

                            if let Some(popup) = popup_menu.get() {
                                let list = window.ext_popup_menu();
                                list.set_items(popup.items.clone());
                                list.select(popup.selected);

                                let cell_metrics = grid_widget.cell_metrics();
                                let (x, y) = cell_metrics.pixel_coords(popup.col, popup.row);
                                let (w, h) = (cell_metrics.char_width, cell_metrics.line_height);

                                let (x, y) =
                                    grid_widget.translate_coordinates(&window, x, y).unwrap();

                                let popover = window.popover();
                                popover.set_pointing_to(Some(&gdk::Rectangle::new(
                                    x as _, y as _, w as _, h as _,
                                )));

                                popover.popup();
                                window.focus();
                            } else {
                                let popover = window.popover();
                                popover.popdown();
                            }

                            flush_state = FlushState::default();
                        }
                    }
                    _ => {}
                }
            }

            glib::Continue(true)
        }
    });

    app.run()
}

#[derive(Debug, Default)]
struct FlushState {
    popup_changed: bool,
    tabline_changed: bool,
}

fn handle_redraw_event(
    style_map: &mut HashMap<u64, nvim::Style>,
    flush_state: &mut FlushState,
    grids: &mut ExtLineGridMap,
    popup_menu: &mut ExtPopupMenu,
    tabline: &mut ExtTabline,
    events: &[RedrawEvent],
) -> bool {
    let mut flushed = false;

    for event in events {
        match event {
            RedrawEvent::GridResize {
                grid,
                width,
                height,
            } => {
                grids.grid_resize(grid, *width as usize, *height as usize);
            }

            RedrawEvent::GridClear { grid } => {
                grids.grid_clear(grid);
            }

            RedrawEvent::GridDestroy { grid } => {
                grids.grid_destroy(grid);
            }

            RedrawEvent::GridScroll {
                grid,
                top,
                bottom,
                left,
                right,
                rows,
                columns,
            } => {
                grids.grid_scroll(grid, *top, *bottom, *left, *right, *rows, *columns);
            }

            RedrawEvent::GridLine {
                grid,
                row,
                column_start,
                cells,
            } => {
                grids.grid_line(grid, *row as usize, *column_start as usize, cells);
            }

            RedrawEvent::GridCursorGoto { grid, row, column } => {
                grids.grid_cursor_goto(grid, *row as usize, *column as usize);
            }

            RedrawEvent::Flush => {
                flushed = true;
            }

            RedrawEvent::PopupmenuShow {
                items,
                selected,
                row,
                col,
                grid,
            } => {
                popup_menu.show(
                    items.clone(),
                    selected.map(|s| s as usize),
                    *row as usize,
                    *col as usize,
                    *grid,
                );
                flush_state.popup_changed = true;
            }

            RedrawEvent::TablineUpdate { current_tab, tabs } => {
                tabline.update(current_tab.clone(), tabs.clone());
                flush_state.tabline_changed = true;
            }

            RedrawEvent::PopupmenuSelect { selected } => {
                popup_menu.select(selected.map(|s| s as usize));
            }

            RedrawEvent::PopupmenuHide => {
                popup_menu.hide();
            }

            RedrawEvent::HighlightAttributesDefine { id, style } => {
                *style_map.entry(*id).or_default() = style.clone();
            }

            event => {
                dbg!(event);
            }
        }
    }

    flushed
}
