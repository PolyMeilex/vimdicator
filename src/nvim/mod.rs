pub mod api_info;
pub use api_info::NeovimApiInfo;

pub mod handler;
pub use handler::NvimHadler;

pub mod event;
pub use event::{NvimEvent, RedrawEvent, Style};

pub mod ext_line_grid;
pub use ext_line_grid::{ExtLineGrid, ExtLineGridMap};

pub mod ext_popup_menu;
pub use ext_popup_menu::{ExtPopupMenu, ExtPopupMenuState};

pub mod ext_tabline;
pub use ext_tabline::ExtTabline;

use gtk::glib;
use tokio::{net::tcp::OwnedWriteHalf, sync::mpsc::UnboundedReceiver};
use tokio_util::compat::{Compat, TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

type Neovim = nvim_rs::Neovim<Compat<OwnedWriteHalf>>;

#[derive(Clone, PartialEq)]
pub struct Tabpage {
    ext: (i8, Vec<u8>),
    inner: nvim_rs::Tabpage<Compat<OwnedWriteHalf>>,
}

impl std::cmp::Eq for Tabpage {}

impl Tabpage {
    pub fn new(code_data: nvim_rs::Value, nvim: Neovim) -> Self {
        let inner = nvim_rs::Tabpage::new(code_data.clone(), nvim);

        Self {
            ext: if let nvim_rs::Value::Ext(a, b) = code_data {
                (a, b)
            } else {
                panic!("Expected Value::Ext in Tabpage")
            },
            inner,
        }
    }
}

impl std::hash::Hash for Tabpage {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.ext.hash(state);
    }
}

impl std::fmt::Debug for Tabpage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tabpage")
            .field("id", &self.ext)
            .finish_non_exhaustive()
    }
}

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
    ExecLua(String),
}

pub async fn run(mut rx: UnboundedReceiver<GtkToNvimEvent>, gtk_tx: glib::Sender<NvimEvent>) {
    let stream = tokio::net::TcpStream::connect("127.0.0.1:8080")
        .await
        .unwrap();

    let handler = NvimHadler::new(gtk_tx);

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
            nvim_rs::UiAttachOptions::new()
                .set_rgb(true)
                .set_popupmenu_external(true)
                // .set_cmdline_external(true)
                .set_linegrid_external(true)
                .set_tabline_external(true)
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
                GtkToNvimEvent::ExecLua(code) => {
                    nvim.exec_lua(&code, vec![]).await.unwrap();
                }
            }
        }
    });

    join.await.unwrap();
}
