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
mod window;

use std::collections::HashMap;

use application::VimdicatorApplication;
use nvim::{ExtLineGridMap, ExtPopupMenu, NeovimApiInfo, NvimEvent, RedrawEvent};
use window::VimdicatorWindow;

use config::{GETTEXT_PACKAGE, LOCALEDIR, PKGDATADIR};
use gettextrs::{bind_textdomain_codeset, bindtextdomain, textdomain};
use gtk::{gdk, prelude::*};
use gtk::{gio, glib};
use nvim_rs::UiAttachOptions;
use tokio::net::tcp::OwnedWriteHalf;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio_util::compat::{Compat, TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

type Neovim = nvim_rs::Neovim<Compat<OwnedWriteHalf>>;

#[derive(Debug, Clone, Copy)]
pub enum NvimMouseButton {
    Left,
    Right,
    Wheel,
}

impl NvimMouseButton {
    fn as_str(&self) -> &str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
            Self::Wheel => "wheel",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum NvimMouseAction {
    Press,
    Release,
    Up,
    Down,
    Drag,
}

impl NvimMouseAction {
    fn as_str(&self) -> &str {
        match self {
            Self::Press => "press",
            Self::Release => "release",
            Self::Up => "up",
            Self::Down => "down",
            Self::Drag => "drag",
        }
    }
}

pub enum GtkToNvimEvent {
    Input(String),
    InputMouse {
        button: NvimMouseButton,
        action: NvimMouseAction,
        modifier: String,
        grid: Option<u64>,
        /// (col, row)
        pos: Option<(u64, u64)>,
    },
    Resized {
        width: u64,
        height: u64,
    },
}

async fn run(mut rx: UnboundedReceiver<GtkToNvimEvent>, gtk_tx: glib::Sender<NvimEvent>) {
    let stream = tokio::net::TcpStream::connect("127.0.0.1:8080")
        .await
        .unwrap();

    let handler = nvim::NvimHadler::new(gtk_tx);

    let (reader, writer) = stream.into_split();
    let (nvim, io_future) = Neovim::new(reader.compat(), writer.compat_write(), handler);

    let join = tokio::spawn(async move {
        // add callback on session end
        if let Err(e) = io_future.await {
            if !e.is_reader_error() {
                println!("{}", e);
            }
        }
    });

    {
        let mut version_info: Vec<(nvim_rs::Value, nvim_rs::Value)> = vec![
            ("major".into(), env!("CARGO_PKG_VERSION_MAJOR").into()),
            ("minor".into(), env!("CARGO_PKG_VERSION_MINOR").into()),
            ("patch".into(), env!("CARGO_PKG_VERSION_PATCH").into()),
        ];
        if let Some(git_commit) = option_env!("GIT_COMMIT") {
            version_info.push(("commit".into(), git_commit.into()));
        }

        nvim.set_client_info(
            env!("CARGO_PKG_NAME"),
            version_info,
            "ui",
            vec![],
            vec![("license".into(), env!("CARGO_PKG_LICENSE").into())],
        )
        .await
        .unwrap();

        let api_info = nvim.get_api_info().await.unwrap();
        let api_info = NeovimApiInfo::new(api_info).unwrap();
        dbg!(api_info);

        nvim.ui_attach(
            200,
            30,
            UiAttachOptions::new()
                .set_rgb(true)
                .set_popupmenu_external(true)
                // .set_cmdline_external(true)
                .set_linegrid_external(true)
                .set_tabline_external(false)
                .set_hlstate_external(true)
                .set_termcolors_external(false)
                .set_wildmenu_external(false),
        )
        .await
        .unwrap();
    }

    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            match event {
                GtkToNvimEvent::Input(input) => {
                    nvim.input(&input).await.unwrap();
                }
                GtkToNvimEvent::InputMouse {
                    button,
                    action,
                    modifier,
                    grid,
                    pos,
                } => {
                    let grid = grid.map(|g| g as i64).unwrap_or(-1);
                    let (col, row) = pos.map(|(c, r)| (c as i64, r as i64)).unwrap_or((-1, -1));

                    nvim.input_mouse(button.as_str(), action.as_str(), &modifier, grid, row, col)
                        .await
                        .unwrap();
                }
                GtkToNvimEvent::Resized { width, height } => {
                    nvim.ui_try_resize(width as i64, height as i64)
                        .await
                        .unwrap();
                }
            }
        }
    });

    join.await.unwrap();
}

fn main() -> glib::ExitCode {
    glib_logger::init(&glib_logger::SIMPLE);

    let (gtk_tx, gtk_rx) = glib::MainContext::channel::<NvimEvent>(glib::Priority::default());
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<GtkToNvimEvent>();

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.spawn(run(rx, gtk_tx));

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
        let mut flush_state = FlushState::default();
        let mut style = HashMap::new();

        move |event| {
            if let Some(window) = app.active_window() {
                let window: VimdicatorWindow = window.downcast().unwrap();

                match event {
                    NvimEvent::Redraw(events) => {
                        let flushed = handle_redraw_event(
                            &mut style,
                            &mut flush_state,
                            &mut grid_map,
                            &mut popup_menu,
                            &events,
                        );

                        if flushed {
                            let grid_widget = window.ext_line_grid();

                            if let Some(grid) = grid_map.get_default() {
                                grid_widget.set_grid(grid.clone());
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

    // Run the application. This function will block until the application
    // exits. Upon return, we have our exit code to return to the shell. (This
    // is the code you see when you do `echo $?` after running a command in a
    // terminal.
    app.run()
}

#[derive(Debug, Default)]
struct FlushState {
    popup_changed: bool,
}

fn handle_redraw_event(
    style_map: &mut HashMap<u64, nvim::Style>,
    flush_state: &mut FlushState,
    grids: &mut ExtLineGridMap,
    popup_menu: &mut ExtPopupMenu,
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
                grids.get_default_mut().unwrap().style = style_map.clone();
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
