use std::cell::{Ref, RefCell, RefMut};
use std::convert::*;
use std::path::*;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::{env, thread};

use log::{debug, warn};

use adw::prelude::*;
use gio::{ApplicationCommandLine, SimpleAction};
use gtk::{Inhibit, Orientation, Paned};
use libpanel::prelude::*;

use serde::{Deserialize, Serialize};

use crate::file_browser::FileBrowserWidget;
use crate::highlight::BackgroundState;
use crate::misc::{self, BoolExt};
use crate::nvim::*;
use crate::settings::{Settings, SettingsLoader};
use crate::shell::{self, HeaderBarButtons, Shell};
use crate::shell_dlg;
use crate::subscriptions::{SubscriptionHandle, SubscriptionKey};
use crate::window::VimdicatorWindow;
use crate::Args;

macro_rules! clone {
    (@param _) => ( _ );
    (@param $x:ident) => ( $x );
    ($($n:ident),+ => move || $body:expr) => (
        {
            $( let $n = $n.clone(); )+
                move || $body
        }
    );
    ($($n:ident),+ => move |$($p:tt),+| $body:expr) => (
        {
            $( let $n = $n.clone(); )+
                move |$(clone!(@param $p),)+| $body
        }
    );
    ($($n:ident),+ => async move $body:expr) => {
        {
            $( let $n = $n.clone(); )+
                async move { $body }
        }
    };
}

const DEFAULT_WIDTH: i32 = 800;
const DEFAULT_HEIGHT: i32 = 600;
const DEFAULT_SIDEBAR_WIDTH: i32 = 200;

pub struct Ui {
    open_paths: Box<[String]>,
    initialized: bool,
    comps: Arc<UiMutex<Components>>,
    settings: Rc<RefCell<Settings>>,
    shell: Rc<RefCell<Shell>>,
    file_browser: Arc<UiMutex<FileBrowserWidget>>,
}

pub struct Components {
    pub window: Option<VimdicatorWindow>,
    window_state: ToplevelState,
    pub exit_confirmed: bool,
}

impl Components {
    fn new() -> Components {
        Components {
            window: None,
            window_state: ToplevelState::load(),
            exit_confirmed: false,
        }
    }

    pub fn close_window(&self) {
        self.window.as_ref().unwrap().close();
    }

    pub fn window(&self) -> &VimdicatorWindow {
        self.window.as_ref().unwrap()
    }

    pub fn set_title(&self, short_title: &str, long_title: &str) {
        self.window.as_ref().unwrap().set_title(Some(long_title));
        self.window
            .as_ref()
            .unwrap()
            .header_bar()
            .set_title(short_title);
    }

    pub fn saved_size(&self) -> (i32, i32) {
        (
            self.window_state.current_width,
            self.window_state.current_height,
        )
    }
}

impl Ui {
    pub fn new(options: Args, open_paths: Box<[String]>) -> Ui {
        let comps = Arc::new(UiMutex::new(Components::new()));
        let settings = Rc::new(RefCell::new(Settings::new()));
        let shell = Rc::new(RefCell::new(Shell::new(settings.clone(), options)));
        let file_browser = Arc::new(UiMutex::new(FileBrowserWidget::new(&shell.borrow().state)));
        settings.borrow_mut().set_shell(Rc::downgrade(&shell));

        Ui {
            initialized: false,
            comps,
            shell,
            settings,
            file_browser,
            open_paths,
        }
    }

    pub fn init(
        &mut self,
        app: &gtk::Application,
        args: &crate::Args,
        app_cmdline: Arc<Mutex<Option<ApplicationCommandLine>>>,
    ) {
        if self.initialized {
            return;
        }
        self.initialized = true;

        let mut settings = self.settings.borrow_mut();
        settings.init();

        let window = VimdicatorWindow::new(app);

        let main = Paned::builder()
            .orientation(Orientation::Horizontal)
            .focusable(false)
            .build();

        let comps_ref = &self.comps;
        let shell_ref = &self.shell;
        let file_browser_ref = &self.file_browser;

        {
            self.shell.borrow_mut().init(app_cmdline, comps_ref);

            // initialize window from comps
            // borrowing of comps must be leaved
            // for event processing
            let mut comps = comps_ref.borrow_mut();

            comps.window = Some(window.clone());

            let sidebar_width = if !args.disable_win_restore {
                if comps.window_state.is_maximized {
                    window.maximize();
                }

                window.set_default_size(
                    comps.window_state.current_width,
                    comps.window_state.current_height,
                );

                comps.window_state.sidebar_width
            } else {
                window.set_default_size(DEFAULT_WIDTH, DEFAULT_HEIGHT);
                DEFAULT_SIDEBAR_WIDTH
            };

            main.set_position(if args.hide_sidebar { 0 } else { sidebar_width });
        }

        // Override default shortcuts which are easy to press accidentally
        if let Some(app) = window.application() {
            app.set_accels_for_action("app.preferences", &[]);
            app.set_accels_for_action("gtkinternal.hide", &[]);
            app.set_accels_for_action("gtkinternal.hide-others", &[]);
            app.set_accels_for_action("app.quit", &[]);

            app.set_accels_for_action("app.show-sidebar", &["<Ctrl>e"]);
        }

        let (update_subtitle, header_bar) = self.create_header_bar(app);

        let show_sidebar_action = SimpleAction::new("show-sidebar", None);
        let sidebar_list_view = self.file_browser.borrow().file_tree_view().list_view();
        show_sidebar_action.connect_activate(
            glib::clone!(@strong file_browser_ref, @weak comps_ref => move |_, _| {
                let comps_ref = &mut *comps_ref.borrow_mut();
                if let Some(window) = comps_ref.window.as_ref(){
                    let is_visible = window.dock().reveals_start();
                    let is_focused = is_visible && (window.start_panel().is_focus() || window.start_panel().focus_child().is_some());


                    let new;

                    if is_visible  {
                        if is_focused {
                            new = false;
                        } else {
                            sidebar_list_view.grab_focus();
                            new = true;
                        }
                    } else {
                        sidebar_list_view.grab_focus();
                        new = true;
                    }

                    window.dock().set_reveal_start(new);
                    comps_ref.window_state.show_sidebar = new;
                }
            }),
        );
        app.add_action(&show_sidebar_action);

        window.connect_default_width_notify(glib::clone!(
            @strong main, @weak comps_ref => move |window| {
                gtk_window_resize(
                    window,
                    &mut comps_ref.borrow_mut(),
                    &main,
                    gtk::Orientation::Horizontal,
                );
            }
        ));
        window.connect_default_height_notify(glib::clone!(
            @strong main, @weak comps_ref => move |window| {
                gtk_window_resize(
                    window,
                    &mut comps_ref.borrow_mut(),
                    &main,
                    gtk::Orientation::Vertical,
                );
            }
        ));

        window.connect_maximized_notify(glib::clone!(@weak comps_ref => move |window| {
            comps_ref.borrow_mut().window_state.is_maximized = window.is_maximized();
        }));

        window.connect_destroy(glib::clone!(@weak comps_ref => move |_| {
            comps_ref.borrow().window_state.save();
        }));

        let shell = self.shell.borrow();
        let file_browser = self.file_browser.borrow();

        window.start_panel().append(&**file_browser);
        window.main_panel().append(&**shell);
        window.present();

        if !args.disable_win_restore {
            // Hide sidebar, if it wasn't shown last time.
            // Has to be done after show_all(), so it won't be shown again.
            let show_sidebar = comps_ref.borrow().window_state.show_sidebar;

            if let Some(window) = comps_ref.borrow().window.as_ref() {
                window.dock().set_reveal_start(show_sidebar);
            }
        }

        let state_ref = shell_ref.borrow().state.clone();
        let state = state_ref.borrow();
        state.subscribe(
            SubscriptionKey::from("VimLeave"),
            &["v:exiting ? v:exiting : 0"],
            glib::clone!(@weak shell_ref => move |args| set_exit_status(&shell_ref, args)),
        );

        // Autocmds we want to run when starting
        let mut autocmds = vec![
            state.subscribe(
                SubscriptionKey::from("BufEnter,BufFilePost,BufModifiedSet,DirChanged"),
                &[
                    "expand('%:p')",
                    "getcwd()",
                    "argidx()",
                    "argc()",
                    "&modified",
                    "&modifiable",
                    "win_gettype()",
                    "&buftype",
                ],
                glib::clone!(@weak comps_ref => move |args| update_window_title(&comps_ref, args)),
            ),
            state.subscribe(
                SubscriptionKey::with_pattern("OptionSet", "completeopt"),
                &["&completeopt"],
                glib::clone!(@weak shell_ref => move |args| set_completeopts(&shell_ref, args)),
            ),
            state.subscribe(
                SubscriptionKey::with_pattern("OptionSet", "background"),
                &["&background"],
                glib::clone!(@weak shell_ref => move |args| set_background(&shell_ref, args)),
            ),
        ];
        autocmds.push(update_subtitle);

        window.connect_close_request(glib::clone!(
            @weak shell_ref, @weak comps_ref => @default-return gtk::Inhibit(false),
            move |_| gtk_close_request(&comps_ref, &shell_ref)
        ));

        shell.grab_focus();

        let (post_config_cmds, diff_mode) = {
            let state_ref = state_ref.borrow();
            let mut options = state_ref.options.borrow_mut();

            (options.post_config_cmds(), options.diff_mode)
        };

        state.set_action_widgets(header_bar, file_browser_ref.borrow().clone());

        drop(state);
        shell.set_detach_cb(Some(glib::clone!(@strong comps_ref => move || {
            glib::idle_add_once(glib::clone!(
                @strong comps_ref => move || comps_ref.borrow().close_window()
            ));
        })));

        shell.set_nvim_started_cb(Some(glib::clone!(
            @strong file_browser_ref,
            @strong self.open_paths as files_list => move || {
            Ui::nvim_started(
                &state_ref.borrow(),
                &file_browser_ref,
                &files_list,
                &autocmds,
                post_config_cmds.as_ref(),
                diff_mode,
            );
        })));

        let comps_ref = comps_ref.clone();
        shell.set_nvim_command_cb(Some(
            move |shell: &mut shell::State, command: NvimCommand| {
                Ui::nvim_command(shell, command, &comps_ref);
            },
        ));
    }

    fn nvim_started(
        shell: &shell::State,
        file_browser: &UiMutex<FileBrowserWidget>,
        files_list: &[String],
        subscriptions: &[SubscriptionHandle],
        post_config_cmds: &[String],
        diff_mode: bool,
    ) {
        file_browser.borrow_mut().init();
        shell.set_autocmds();
        for subscription in subscriptions.iter() {
            shell.run_now(subscription);
        }

        let mut commands = Vec::<String>::new();
        if !files_list.is_empty() {
            if diff_mode {
                commands.reserve(files_list.len() + post_config_cmds.len());
                commands.push(format!(
                    r"try|e {}|cat /^Vim(\a\+):E325:/|endt|difft",
                    misc::escape_filename(&files_list[0])
                ));
                for file in &files_list[1..] {
                    commands.push(format!(
                        r"try|vs {}|cat /^Vim(\a\+):E325:/|endt|difft",
                        misc::escape_filename(file)
                    ));
                }
            } else {
                commands.reserve(1 + post_config_cmds.len());
                commands.push(format!(
                    r"try|ar {}|cat /^Vim(\a\+):E325:/|endt",
                    files_list
                        .iter()
                        .map(|f| misc::escape_filename(f))
                        .collect::<Box<_>>()
                        .join(" ")
                ));
            }
        }

        commands.extend(
            post_config_cmds
                .iter()
                .map(|cmd| format!(r#"exec "{}""#, misc::viml_escape(cmd))),
        );
        debug!("{:?}", commands);

        // open files as last command
        // because it can generate user query
        let action_widgets = shell.action_widgets();
        if commands.is_empty() {
            action_widgets.borrow().as_ref().unwrap().set_enabled(true);
            return;
        }

        let commands = commands.join("|");
        let nvim_client = shell.nvim_clone();
        let nvim = nvim_client.nvim().unwrap();
        let channel_id = nvim_client
            .api_info()
            .expect("API info should be initialized by the time this is called")
            .channel;
        nvim.clone().spawn(async move {
            let res = nvim.command(&commands).await;

            glib::idle_add_once(move || {
                action_widgets.borrow().as_ref().unwrap().set_enabled(true)
            });

            if let Err(e) = res {
                if let Ok(e) = NormalError::try_from(&*e) {
                    if e == NormalError::KeyboardInterrupt {
                        nvim.shutdown(channel_id).await;
                    } else if !e.has_code(325) {
                        // Filter out errors we get if the user is presented with a prompt
                        e.print(&nvim).await;
                    }
                } else {
                    e.print();
                }
            }
        });
    }

    fn nvim_command(shell: &mut shell::State, command: NvimCommand, comps: &UiMutex<Components>) {
        match command {
            NvimCommand::ShowProjectView => {
                // TODO:
                // glib::idle_add_once(clone!(projects => move || projects.borrow_mut().show()));
            }
            NvimCommand::ShowGtkInspector => {
                comps
                    .borrow()
                    .window
                    .as_ref()
                    .unwrap()
                    .emit_enable_debugging(false);
            }
            NvimCommand::ToggleSidebar => {
                let comps = comps.borrow();
                if let Some(window) = comps.window.as_ref() {
                    let is = window.dock().reveals_start();
                    window.dock().set_reveal_start(!is);
                }
            }
            NvimCommand::Transparency(background_alpha, filled_alpha) => {
                let comps = comps.borrow();
                let window = comps.window.as_ref().unwrap();

                let display = gtk::prelude::WidgetExt::display(window);
                if display.is_composited() {
                    shell.set_transparency(background_alpha, filled_alpha);
                } else {
                    warn!("Screen is not composited");
                }
            }
            NvimCommand::PreferDarkTheme(prefer_dark_theme) => {
                let comps = comps.borrow();
                let window = comps.window.as_ref().unwrap();

                window
                    .settings()
                    .set_property("gtk-application-prefer-dark-theme", prefer_dark_theme);
            }
        }
    }

    fn create_header_bar(
        &self,
        app: &gtk::Application,
    ) -> (SubscriptionHandle, Box<HeaderBarButtons>) {
        let comps = self.comps.borrow_mut();

        let window = comps.window.as_ref().unwrap();

        self.create_actions(app, window);

        let shell = self.shell.borrow();

        let header_bar = window.header_bar().clone();
        let update_subtitle = shell.state.borrow().subscribe(
            SubscriptionKey::from("DirChanged"),
            &["getcwd()"],
            move |args| {
                header_bar.set_subtitle(shorten_file_path(&args[0]));
            },
        );

        (update_subtitle, Box::new(HeaderBarButtons::new()))
    }

    fn create_actions(&self, app: &gtk::Application, window: &VimdicatorWindow) {
        let edit_paste_action = SimpleAction::new("edit-paste", None);
        let shell = self.shell.clone();
        edit_paste_action.connect_activate(move |_, _| shell.borrow().edit_paste());
        edit_paste_action.set_enabled(true);
        app.add_action(&edit_paste_action);

        let new_tab_action = SimpleAction::new("new-tab", None);
        let shell = self.shell.clone();
        new_tab_action.connect_activate(move |_, _| shell.borrow().new_tab());
        new_tab_action.set_enabled(true);
        app.add_action(&new_tab_action);

        let save_all_action = SimpleAction::new("save-all", None);
        let shell = self.shell.clone();
        save_all_action.connect_activate(move |_, _| shell.borrow().edit_save_all());
        save_all_action.set_enabled(true);
        app.add_action(&save_all_action);

        let about_action = SimpleAction::new("HelpAbout", None);
        about_action.connect_activate(clone!(window => move |_, _| on_help_about(&window)));
        about_action.set_enabled(true);

        app.add_action(&about_action);

        // let shell = &self.shell;
        // btn.connect_realize(clone!(shell => move |btn| {
        //     let drawing_area = shell.borrow().state.borrow().nvim_viewport.clone();

        //     btn
        //         .popover()
        //         .unwrap()
        //         .downcast_ref::<gtk::Popover>()
        //         .unwrap()
        //         .connect_closed(move |_| {
        //             drawing_area.grab_focus();
        //         });
        //     }
        // ));
    }
}

fn on_help_about(window: &VimdicatorWindow) {
    adw::AboutWindow::builder()
        .transient_for(window)
        .application_name("Vimdicator")
        .version(crate::GIT_BUILD_VERSION.unwrap_or(env!("CARGO_PKG_VERSION")))
        .application_icon("nvim")
        .developers(
            env!("CARGO_PKG_AUTHORS")
                .split(':')
                .collect::<Vec<_>>()
                .as_slice(),
        )
        .comments(misc::about_comments().as_str())
        .modal(true)
        .build()
        .show();
}

fn gtk_close_request(comps: &Arc<UiMutex<Components>>, shell: &Rc<RefCell<Shell>>) -> Inhibit {
    let shell_ref = shell.borrow();
    if !shell_ref.is_nvim_initialized() {
        return Inhibit(false);
    }

    let nvim = shell_ref.state.borrow().nvim_clone();
    Inhibit(if shell_dlg::can_close_window(comps, shell, &nvim) {
        let comps = comps.borrow();
        comps.close_window();
        shell_ref.detach_ui();
        false
    } else {
        true
    })
}

fn gtk_window_resize(
    app_window: &VimdicatorWindow,
    comps: &mut Components,
    main: &gtk::Paned,
    orientation: gtk::Orientation,
) {
    if !app_window.is_maximized() {
        match orientation {
            gtk::Orientation::Horizontal => {
                comps.window_state.current_width = app_window.size(gtk::Orientation::Horizontal)
            }
            gtk::Orientation::Vertical => {
                comps.window_state.current_height = app_window.size(gtk::Orientation::Vertical)
            }
            _ => unreachable!(),
        }
    }
    if comps.window_state.show_sidebar {
        comps.window_state.sidebar_width = main.position();
    }
}

fn set_completeopts(shell: &RefCell<Shell>, args: Vec<String>) {
    let options = &args[0];

    shell.borrow().set_completeopts(options);
}

fn set_background(shell: &RefCell<Shell>, args: Vec<String>) {
    let background = match args[0].as_str() {
        "light" => BackgroundState::Light,
        "dark" => BackgroundState::Dark,
        val => panic!("Unexpected 'background' value received: {}", val),
    };

    let state = &shell.borrow().state;
    state.borrow().set_background(background);

    // Neovim won't send us a redraw to update the default colors on the screen, so do it ourselves
    glib::idle_add_once(
        clone!(state => move || state.borrow_mut().queue_draw(RedrawMode::ClearCache)),
    );
}

fn shorten_file_path(path: impl AsRef<Path>) -> String {
    let path = path.as_ref();
    if let Ok(path) = path.canonicalize() {
        if let Ok(path) = path.strip_prefix(glib::home_dir()) {
            return format!("~{MAIN_SEPARATOR}{}", path.to_string_lossy());
        }
    }

    path.to_string_lossy().to_string()
}

fn format_window_title(
    file_path: &str,
    dir: &Path,
    argidx: u32,
    argc: u32,
    modified: bool,
    modifiable: bool,
    long: bool,
) -> String {
    let mut parts = Vec::with_capacity(5);

    let filename = if file_path.is_empty() {
        "[No Name]"
    } else if let Some(rel_path) = Path::new(&file_path)
        .strip_prefix(dir)
        .ok()
        .and_then(|p| p.to_str())
    {
        rel_path
    } else {
        file_path
    };
    parts.push(filename);

    if modifiable {
        if modified {
            parts.push("+");
        }
    } else {
        parts.push("-");
    }

    let dir_str;
    if long {
        dir_str = format!("({})", shorten_file_path(dir));
        parts.push(&dir_str);
    }

    let arg_cnt;
    if argc > 1 {
        arg_cnt = format!("({argidx} of {argc})");
        parts.push(&arg_cnt);
    }

    if long {
        parts.push("- neovim-gtk");
    }

    parts.join(" ")
}

fn update_window_title(comps: &Arc<UiMutex<Components>>, args: Vec<String>) {
    let file_path = &args[0];
    let dir = Path::new(&args[1]);
    let argidx = args[2].parse::<u32>().unwrap() + 1;
    let argc = args[3].parse::<u32>().unwrap();
    let modified = bool::from_int_str(&args[4]).unwrap();
    let modifiable = bool::from_int_str(&args[5]).unwrap();

    // Ignore certain window types that will never have a title (GH #26)
    let win_type = &args[6];
    let buf_type = &args[7];
    if !win_type.is_empty() || !matches!(buf_type.as_str(), "" | "terminal") {
        return;
    }

    comps.borrow().set_title(
        &format_window_title(file_path, dir, argidx, argc, modified, modifiable, false),
        &format_window_title(file_path, dir, argidx, argc, modified, modifiable, true),
    );
}

fn set_exit_status(shell: &RefCell<Shell>, args: Vec<String>) {
    let status = args[0].parse().unwrap();
    shell.borrow().set_exit_status(status);
}

#[derive(Serialize, Deserialize)]
struct ToplevelState {
    current_width: i32,
    current_height: i32,
    is_maximized: bool,
    show_sidebar: bool,
    sidebar_width: i32,
}

impl Default for ToplevelState {
    fn default() -> Self {
        ToplevelState {
            current_width: DEFAULT_WIDTH,
            current_height: DEFAULT_HEIGHT,
            is_maximized: false,
            show_sidebar: false,
            sidebar_width: DEFAULT_SIDEBAR_WIDTH,
        }
    }
}

impl SettingsLoader for ToplevelState {
    const SETTINGS_FILE: &'static str = "window.toml";

    fn from_str(s: &str) -> Result<Self, String> {
        toml::from_str(s).map_err(|e| format!("{e}"))
    }
}

/// Our big thread-safety guard. This guard relies on the following assertions to remain true in
/// order to provide safety:
///
/// 1. T may never be accessed, except from within the same thread the UiMutex was originally
///    created on
/// 2. The thread T was created on is destroyed only after all other possible threads with
///    references to T have been finished execution
///
/// Both of these assumptions are verified at runtime, just in case.
#[derive(Debug)]
pub struct UiMutex<T: ?Sized> {
    thread: thread::ThreadId,
    location: RefCell<Option<String>>,
    data: RefCell<T>,
}

unsafe impl<T: ?Sized> Send for UiMutex<T> {}
unsafe impl<T: ?Sized> Sync for UiMutex<T> {}

impl<T: ?Sized> Drop for UiMutex<T> {
    fn drop(&mut self) {
        assert_eq!(
            self.thread,
            thread::current().id(),
            "Value dropped on a different thread than where it was created, this likely means our \
            async runtime outlived GTK+. That's not good!"
        );
    }
}

impl<T> UiMutex<T> {
    pub fn new(t: T) -> UiMutex<T> {
        UiMutex {
            thread: thread::current().id(),
            location: Default::default(),
            data: RefCell::new(t),
        }
    }

    pub fn replace(&self, t: T) -> T {
        self.assert_ui_thread();
        self.data.replace(t)
    }
}

impl<T: ?Sized> UiMutex<T> {
    #[track_caller]
    pub fn borrow(&self) -> Ref<T> {
        self.assert_ui_thread();

        let res = self.data.try_borrow();

        if res.is_err() {
            dbg!(&self.location);
        } else {
            let loc = std::panic::Location::caller();
            *self.location.borrow_mut() = Some(format!("{loc:?}"));
        }

        res.unwrap()
    }

    pub fn try_borrow_mut(&self) -> Option<RefMut<T>> {
        self.data.try_borrow_mut().ok()
    }

    #[track_caller]
    pub fn borrow_mut(&self) -> RefMut<T> {
        self.assert_ui_thread();

        let res = self.data.try_borrow_mut();

        if res.is_err() {
            dbg!(&self.location);
        } else {
            let loc = std::panic::Location::caller();
            *self.location.borrow_mut() = Some(format!("{loc:?}"));
        }

        res.unwrap()
    }

    #[inline]
    fn assert_ui_thread(&self) {
        if thread::current().id() != self.thread {
            panic!("Can access to UI only from main thread");
        }
    }
}
