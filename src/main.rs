#![windows_subsystem = "windows"]
#![allow(clippy::new_without_default)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::comparison_chain)]
#![allow(clippy::await_holding_refcell_ref)]

mod color;
mod dirs;
mod mode;
mod nvim_config;
mod ui_model;
mod value;
#[macro_use]
mod ui;
mod cmd_line;
mod cursor;
mod error;
mod file_browser;
mod grid;
mod highlight;
mod input;
mod misc;
mod nvim;
mod nvim_viewport;
mod plug_manager;
mod popup_menu;
mod project;
mod render;
mod settings;
mod shell;
mod shell_dlg;
mod subscriptions;
mod tabline;

use log::error;

use gio::prelude::*;
use gio::ApplicationCommandLine;
use std::cell::RefCell;
use std::io::Read;
use std::sync::{Arc, Mutex};

#[cfg(unix)]
use fork::{daemon, Fork};

use crate::shell::ShellOptions;
use crate::ui::Ui;

use clap::{value_parser, Arg, ArgAction, ArgMatches, Command};

use is_terminal::IsTerminal;

include!(concat!(env!("OUT_DIR"), "/version.rs"));

fn main() {
    env_logger::init();

    let mut command = Command::new("NeovimGtk")
        .version(GIT_BUILD_VERSION.unwrap_or(env!("CARGO_PKG_VERSION")))
        .author(env!("CARGO_PKG_AUTHORS"))
        .about(misc::about_comments())
        .arg(Arg::new("no-fork")
             .long("no-fork")
             .action(ArgAction::SetTrue)
             .help("Prevent detach from console"))
        .arg(Arg::new("disable-win-restore")
             .long("disable-win-restore")
             .action(ArgAction::SetTrue)
             .help("Don't restore window size at start"))
        .arg(Arg::new("timeout")
             .long("timeout")
             .value_parser(value_parser!(u64))
             .default_value("10")
             .help("Wait timeout in seconds. If nvim does not response in given time NvimGtk stops")
             .num_args(1))
        .arg(Arg::new("cterm-colors")
             .long("cterm-colors")
             .action(ArgAction::SetTrue)
             .help("Use ctermfg/ctermbg instead of guifg/guibg"))
        .arg(Arg::new("diff-mode")
             .help("Open two or more files in diff mode")
             .short('d')
             .action(ArgAction::SetTrue))
        .arg(Arg::new("files")
             .help("Files to open")
             .num_args(1..))
        .arg(Arg::new("nvim-bin-path")
             .long("nvim-bin-path")
             .help("Path to nvim binary")
             .num_args(1))
        .arg(Arg::new("post-config-cmds")
             .help("Execute <cmd> after config and first file")
             .value_name("cmd")
             .short('c')
             .num_args(1)
             .action(ArgAction::Append))
        .arg(Arg::new("nvim-args")
             .help("Args will be passed to nvim")
             .last(true)
             .num_args(0..));

    let matches = command.get_matches_mut();

    let input_data = RefCell::new(read_piped_input());

    // Additional argument parsing
    if matches.get_flag("diff-mode") {
        if let Some(files) = matches.get_many::<String>("files") {
            if files.len() < 2 {
                command
                    .error(
                        clap::error::ErrorKind::TooFewValues,
                        "Diff mode (-d) requires 2 or more files",
                    )
                    .exit();
            }
        } else {
            command.error(
                clap::error::ErrorKind::MissingRequiredArgument,
                "Diff mode (-d) specified but no files provided. 2 or more files must be provided",
            ).exit();
        }
    }

    // fork to background by default
    #[cfg(unix)]
    if !matches.get_flag("no-fork") {
        match daemon(true, true) {
            Ok(Fork::Parent(_)) => return,
            Ok(Fork::Child) => (),
            Err(code) => panic!("Failed to fork, got {}", code),
        };
    }

    #[cfg(debug_assertions)]
    if std::env::var("NVIM_GTK_CLI_TEST_MODE") == Ok("1".to_string()) {
        println!("Testing the CLI");
        return;
    }

    gtk::init().expect("Failed to initialize GTK+");

    let app_flags = gio::ApplicationFlags::HANDLES_OPEN
        | gio::ApplicationFlags::HANDLES_COMMAND_LINE
        | gio::ApplicationFlags::NON_UNIQUE;

    glib::set_program_name(Some("NeovimGtk"));

    let app = if cfg!(debug_assertions) {
        gtk::Application::new(Some("org.daa.NeovimGtkDebug"), app_flags)
    } else {
        gtk::Application::new(Some("org.daa.NeovimGtk"), app_flags)
    };

    let app_cmdline = Arc::new(Mutex::new(None));
    let app_cmdline_copy = app_cmdline.clone();
    let matches_copy = matches.clone();
    app.connect_command_line(move |app, cmdline| {
        app_cmdline_copy.lock().unwrap().replace(cmdline.clone());
        let input_data = input_data
            .replace(None)
            .filter(|_input| !matches_copy.get_flag("files"));

        match input_data {
            Some(input_data) => activate(
                app,
                &matches_copy,
                Some(input_data),
                app_cmdline_copy.clone(),
            ),
            None => {
                let files = matches_copy
                    .get_many::<String>("files")
                    .into_iter()
                    .flat_map(|v| v.into_iter().cloned())
                    .collect::<Box<[String]>>();
                open(app, files, &matches_copy, app_cmdline_copy.clone());
            }
        }
        0
    });

    // Setup our global style provider
    let css_provider = gtk::CssProvider::new();
    css_provider.load_from_data(include_bytes!("style.css"));
    gtk::StyleContext::add_provider_for_display(
        gdk::Display::default()
            .as_ref()
            .expect("Cannot find default GDK Display"),
        &css_provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    let app_ref = app.clone();
    let matches_copy = matches;
    let app_cmdline_copy = Arc::clone(&app_cmdline);
    let new_window_action = gio::SimpleAction::new("new-window", None);
    new_window_action.connect_activate(move |_, _| {
        activate(&app_ref, &matches_copy, None, app_cmdline_copy.clone())
    });
    app.add_action(&new_window_action);

    gtk::Window::set_default_icon_name("org.daa.NeovimGtk");

    app.run();
    let lock = app_cmdline.lock().unwrap();
    std::process::exit(lock.as_ref().unwrap().exit_status());
}

fn open(
    app: &gtk::Application,
    files: Box<[String]>,
    matches: &ArgMatches,
    app_cmdline: Arc<Mutex<Option<ApplicationCommandLine>>>,
) {
    let mut ui = Ui::new(ShellOptions::new(matches, None), files);

    ui.init(app, !matches.get_flag("disable-win-restore"), app_cmdline);
}

fn activate(
    app: &gtk::Application,
    matches: &ArgMatches,
    input_data: Option<String>,
    app_cmdline: Arc<Mutex<Option<ApplicationCommandLine>>>,
) {
    let mut ui = Ui::new(ShellOptions::new(matches, input_data), Box::new([]));

    ui.init(app, !matches.get_flag("disable-win-restore"), app_cmdline);
}

fn read_piped_input() -> Option<String> {
    if !std::io::stdout().is_terminal() {
        let mut buf = String::new();
        match std::io::stdin().read_to_string(&mut buf) {
            Ok(size) if size > 0 => Some(buf),
            Ok(_) => None,
            Err(err) => {
                error!("Error read stdin {}", err);
                None
            }
        }
    } else {
        None
    }
}
