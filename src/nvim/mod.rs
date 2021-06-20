mod client;
mod handler;
mod redraw_handler;
mod repaint_mode;
mod ext;

pub use self::redraw_handler::{CompleteItem, NvimCommand};
pub use self::repaint_mode::RepaintMode;
pub use self::client::NeovimClient;
pub use self::ext::*;
pub use self::handler::NvimHandler;

use std::{
    error, fmt, env, result,
    time::Duration,
    pin::Pin,
    process::Stdio,
    task::{Context, Poll},
    ops::{Deref, DerefMut},
    future::Future,
    sync::Arc,
};

use tokio::{
    io::{self, AsyncWrite},
    process::{Command, ChildStdin},
    time::{timeout, error::Elapsed},
    task::JoinHandle,
    runtime::Runtime,
};
use tokio_util::compat::*;

use futures::future::{
    FutureExt, BoxFuture,
};

use nvim_rs::{
    self,
    UiAttachOptions,
    error::{LoopError, CallError, DecodeError},
    compat::tokio::Compat,
};

use crate::nvim_config::NvimConfig;

#[derive(Debug)]
pub struct NvimInitError {
    source: Box<dyn error::Error>,
    cmd: Option<String>,
}

impl NvimInitError {
    pub fn new_post_init<E>(error: E) -> NvimInitError
    where
        E: Into<Box<dyn error::Error>>,
    {
        NvimInitError {
            cmd: None,
            source: error.into(),
        }
    }

    pub fn new<E>(cmd: &Command, error: E) -> NvimInitError
    where
        E: Into<Box<dyn error::Error>>,
    {
        NvimInitError {
            cmd: Some(format!("{:?}", cmd)),
            source: error.into(),
        }
    }

    pub fn source(&self) -> String {
        format!("{}", self.source)
    }

    pub fn cmd(&self) -> Option<&String> {
        self.cmd.as_ref()
    }
}

impl fmt::Display for NvimInitError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.source)
    }
}

impl error::Error for NvimInitError {
    fn description(&self) -> &str {
        "Can't start nvim instance"
    }

    fn cause(&self) -> Option<&dyn error::Error> {
        Some(&*self.source)
    }
}

#[derive(Debug)]
pub enum SessionError {
    CallError(Box<CallError>),
    TimeoutError(Elapsed),
}

impl error::Error for SessionError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::CallError(e) => Some(e),
            Self::TimeoutError(e) => Some(e),
        }
    }
}

impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::CallError(e) => write!(f, "{:?}", e),
            Self::TimeoutError(e) => write!(f, "{:?}", e),
        }
    }
}

impl From<Box<CallError>> for SessionError {
    fn from(err: Box<CallError>) -> Self {
        SessionError::CallError(err)
    }
}

impl From<Elapsed> for SessionError {
    fn from(err: Elapsed) -> Self {
        SessionError::TimeoutError(err)
    }
}

#[cfg(target_os = "windows")]
fn set_windows_creation_flags(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
}

pub enum NvimWriter {
    ChildProcess(ChildStdin),
}

impl From<ChildStdin> for NvimWriter {
    fn from(stdin: ChildStdin) -> Self {
        Self::ChildProcess(stdin)
    }
}

impl AsyncWrite for NvimWriter {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8]
    ) -> Poll<io::Result<usize>> {
        match *self {
            Self::ChildProcess(ref mut stdin) => Pin::new(stdin).poll_write(cx, buf),
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match *self {
            Self::ChildProcess(ref mut stdin) => Pin::new(stdin).poll_flush(cx),
        }
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match *self {
            Self::ChildProcess(ref mut stdin) => Pin::new(stdin).poll_shutdown(cx),
        }
    }
}

pub type Neovim = nvim_rs::Neovim<Compat<NvimWriter>>;
pub type Tabpage = nvim_rs::Tabpage<Compat<NvimWriter>>;

/// Our main wrapper for `Neovim`, which also provides access to the timeout duration for this
/// session
#[derive(Clone)]
pub struct NvimSession {
    nvim: Neovim,
    timeout: Duration,
    runtime: Arc<Runtime>,
}

impl NvimSession {
    pub fn new_child<'a>(
        mut cmd: Command,
        handler: NvimHandler,
        timeout: Duration,
    ) -> Result<(NvimSession, BoxFuture<'a, Result<(), Box<LoopError>>>), NvimInitError> {
        let runtime = Arc::new(
            Runtime::new().map_err(|e| NvimInitError::new(&cmd, e))?
        );
        let mut child = runtime.block_on(async move {
            cmd.spawn().map_err(|e| NvimInitError::new(&cmd, e))
        })?;

        let (nvim, io_future) = Neovim::new(
            child.stdout.take().unwrap().compat(),
            NvimWriter::from(child.stdin.take().unwrap()).compat_write(),
            handler
        );

        Ok((Self { nvim, timeout, runtime }, io_future.boxed()))
    }

    /// Wrap a future from an RPC call to neovim within a timeout
    pub async fn timeout<F, T>(&self, f: F) -> Result<T, SessionError>
    where
        F: Future<Output = Result<T, Box<CallError>>>,
    {
        match timeout(self.timeout, f).await {
            Ok(f) => match f {
                Ok(f) => Ok(f),
                Err(e) => Err(e.into()),
            },
            Err(e) => Err(e.into()),
        }
    }

    /// Execute a future on the current thread using this session's tokio runtime
    #[inline]
    pub fn block_on<T>(&self, f: impl Future<Output = T>) -> T {
        self.runtime.block_on(f)
    }

    /// Spawn a future on this session's tokio runtime
    #[inline]
    pub fn spawn(&self, f: impl Future<Output = ()> + Send + 'static) -> JoinHandle<()> {
        self.runtime.spawn(f)
    }

    /// Wrap a future from an RPC call to neovim inside a timeout, and execute it on the current
    /// thread using this session's tokio runtime
    pub fn block_timeout<F, T>(&self, f: F) -> Result<T, SessionError>
    where
        F: Future<Output = Result<T, Box<CallError>>>,
    {
        self.block_on(self.timeout(f))
    }

    #[doc(hidden)]
    pub fn spawn_timeout<F, T>(&self, f: F) -> JoinHandle<()>
    where
        F: Future<Output = Result<T, Box<CallError>>> + Send + 'static
    {
        let nvim = self.clone();

        self.spawn(async move {
            nvim.timeout(f).await.report_err()
        })
    }

    /// Shutdown this neovim session by executing the relevant autocommands, and then closing our
    /// RPC channel with the Neovim instance.
    pub async fn shutdown(&self) {
        self.timeout(self.command("doau VimLeavePre|doau VimLeave")).await.report_err();

        let chan = self
            .timeout(self.get_api_info())
            .await
            .ok_and_report()
            .and_then(|v| v[0].as_i64())
            .expect("Couldn't retrieve current channel for closing");

        let res = self.timeout(self.command(&format!("cal chanclose({})", chan))).await;
        if let Err(ref e) = res {
            if let SessionError::CallError(ref e) = *e {
                if let CallError::DecodeError(ref e, _) = **e {
                    if let DecodeError::ReaderError(_) = **e {
                        // It's expected that we'll fail to read the response to this
                        return;
                    }
                }
            }
            res.report_err();
        }
    }
}

impl Deref for NvimSession {
    type Target = Neovim;

    fn deref(&self) -> &Self::Target {
        &self.nvim
    }
}

impl DerefMut for NvimSession {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.nvim
    }
}

/// Wrap a future with a timeout, and spawn it on this session's tokio runtime, then report any
/// resulting errors to the console.
#[macro_export] macro_rules! spawn_timeout {
    ($nvim:ident.$fn:ident($( $a:expr ),*)) => {
        let nvim = $nvim.clone();
        $nvim.spawn_timeout(async move { nvim.$fn($( $a ),*).await })
    };
}

pub fn start<'a>(
    handler: NvimHandler,
    nvim_bin_path: Option<String>,
    timeout: Option<Duration>,
    args_for_neovim: Vec<String>,
) -> result::Result<(NvimSession, BoxFuture<'a, Result<(), Box<LoopError>>>), NvimInitError> {
    let mut cmd = if let Some(path) = nvim_bin_path {
        Command::new(path)
    } else {
        Command::new("nvim")
    };

    cmd.arg("--embed")
        .arg("--cmd")
        .arg("set termguicolors")
        .arg("--cmd")
        .arg("let g:GtkGuiLoaded = 1")
        .arg("--cmd")
        .arg(&format!("let &rtp.=',{}'",
                      env::var("NVIM_GTK_RUNTIME_PATH").unwrap_or(env!("RUNTIME_PATH").into())))
        .stderr(Stdio::inherit())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped());

    #[cfg(target_os = "windows")]
    set_windows_creation_flags(&mut cmd);

    if let Some(nvim_config) = NvimConfig::config_path() {
        if let Some(path) = nvim_config.to_str() {
            cmd.arg("--cmd").arg(format!("source {}", path));
        }
    }

    for arg in args_for_neovim {
        cmd.arg(arg);
    }

    NvimSession::new_child(cmd, handler, timeout.unwrap_or(Duration::from_secs(10)))
}

pub async fn post_start_init(
    nvim: NvimSession,
    cols: i64,
    rows: i64,
    input_data: Option<String>,
) -> Result<(), NvimInitError> {
    nvim.timeout(nvim.ui_attach(
        cols,
        rows,
        UiAttachOptions::new()
        .set_popupmenu_external(true)
        .set_tabline_external(true)
        .set_linegrid_external(true)
        .set_hlstate_external(true)
        ))
        .await.map_err(NvimInitError::new_post_init)?;

    nvim.timeout(nvim.command("runtime! ginit.vim")).await.map_err(NvimInitError::new_post_init)?;

    if let Some(input_data) = input_data {
        let buf = nvim.timeout(nvim.get_current_buf()).await.ok_and_report();

        if let Some(buf) = buf {
            nvim.timeout(buf.set_lines(
                0,
                0,
                true,
                input_data.lines().map(|l| l.to_owned()).collect()
            )).await.report_err();
        }
    }

    Ok(())
}
