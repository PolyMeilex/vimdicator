use log::error;
use nvim_rs::Value;

#[derive(Debug, Clone)]
pub enum NvimEvent {
    Redraw(Vec<RedrawEvent>),
    Gui(Vec<Value>),
    Subscription(Vec<Value>),
    Resized(Vec<Value>),
}

#[derive(Clone, Debug)]
pub enum GuiOption {
    ArabicShape(bool),
    AmbiWidth(String),
    Emoji(bool),
    GuiFont(String),
    GuiFontSet(String),
    GuiFontWide(String),
    LineSpace(i64),
    Pumblend(u64),
    ShowTabLine(u64),
    TermGuiColors(bool),
    Mousefocus(bool),

    ExtLinegrid(bool),
    ExtMultigrid(bool),
    ExtHlstate(bool),
    ExtTermcolors(bool),
    ExtCmdline(bool),
    ExtPopupmenu(bool),
    ExtTabline(bool),
    ExtWildmenu(bool),
    ExtMessages(bool),

    Unknown(String, Value),
}

impl GuiOption {
    fn parse(event: Vec<nvim_rs::Value>) -> Option<Self> {
        let mut event = event.into_iter();

        let name = event.next()?;
        let name = into_string(name)?;

        let value = event.next()?;

        let this = match name.as_str() {
            "arabicshape" => Self::ArabicShape(value.as_bool()?),
            "ambiwidth" => Self::AmbiWidth(into_string(value)?),
            "emoji" => Self::Emoji(value.as_bool()?),
            "guifont" => Self::GuiFont(into_string(value)?),
            "guifontset" => Self::GuiFontSet(into_string(value)?),
            "guifontwide" => Self::GuiFontWide(into_string(value)?),
            "linespace" => Self::LineSpace(value.as_i64()?),
            "pumblend" => Self::Pumblend(value.as_u64()?),
            "showtabline" => Self::ShowTabLine(value.as_u64()?),
            "termguicolors" => Self::TermGuiColors(value.as_bool()?),
            "mousefocus" => Self::Mousefocus(value.as_bool()?),

            "ext_linegrid" => Self::ExtLinegrid(value.as_bool()?),
            "ext_multigrid" => Self::ExtMultigrid(value.as_bool()?),
            "ext_hlstate" => Self::ExtHlstate(value.as_bool()?),
            "ext_termcolors" => Self::ExtTermcolors(value.as_bool()?),
            "ext_cmdline" => Self::ExtCmdline(value.as_bool()?),
            "ext_popupmenu" => Self::ExtPopupmenu(value.as_bool()?),
            "ext_tabline" => Self::ExtTabline(value.as_bool()?),
            "ext_wildmenu" => Self::ExtWildmenu(value.as_bool()?),
            "ext_messages" => Self::ExtMessages(value.as_bool()?),
            _ => Self::Unknown(name, value),
        };

        Some(this)
    }
}

#[derive(Clone, Debug)]
pub struct GridLineCell {
    pub text: String,
    pub highlight_id: Option<u64>,
    pub repeat: Option<u64>,
}

impl GridLineCell {
    fn parse(fields: Vec<Value>) -> Option<Self> {
        let mut fields = fields.into_iter();
        Some(Self {
            text: into_string(fields.next()?)?,
            highlight_id: fields.next().and_then(|v| v.as_u64()),
            repeat: fields.next().and_then(|v| v.as_u64()),
        })
    }
}

#[derive(Debug, Clone)]
pub struct PopupMenuItem {
    pub word: String,
    pub kind: String,
    pub menu: String,
    pub info: String,
}

impl PopupMenuItem {
    fn new(menu: Vec<String>) -> Option<Self> {
        let mut iter = menu.into_iter();
        Some(PopupMenuItem {
            word: iter.next()?,
            kind: iter.next()?,
            menu: iter.next()?,
            info: iter.next()?,
        })
    }
}

#[derive(Debug, Clone)]
pub enum RedrawEvent {
    OptionSet(GuiOption),
    ModeInfoSet,
    HighlightAttributesDefine,
    HighlightGroupSet,

    GridLine {
        grid: u64,
        row: u64,
        column_start: u64,
        cells: Vec<GridLineCell>,
    },
    GridClear {
        grid: u64,
    },
    GridDestroy {
        grid: u64,
    },
    GridCursorGoto {
        grid: u64,
        row: u64,
        column: u64,
    },
    GridScroll {
        grid: u64,
        top: u64,
        bottom: u64,
        left: u64,
        right: u64,
        rows: i64,
        columns: i64,
    },
    GridResize {
        grid: u64,
        width: u64,
        height: u64,
    },

    WindowViewport,

    ModeChange,
    MouseOn,
    MouseOff,
    Flush,

    PopupmenuShow {
        items: Vec<PopupMenuItem>,
        selected: Option<u64>,
        row: u64,
        col: u64,
        grid: u64,
    },
    PopupmenuSelect {
        selected: Option<u64>,
    },
    PopupmenuHide,
    Unknown(String, Vec<Value>),
}

fn into_array(value: Value) -> Option<Vec<Value>> {
    match value {
        Value::Array(arr) => Some(arr),
        _ => None,
    }
}

fn into_string(value: Value) -> Option<String> {
    match value {
        Value::String(arr) => arr.into_str(),
        _ => None,
    }
}

impl RedrawEvent {
    fn parse(args: Vec<nvim_rs::Value>) -> Option<Vec<Self>> {
        let mut args_iter = args.into_iter();

        let Some(name) = args_iter.next() else {
            error!(
                "No name provided with redraw event, args: {:?}",
                args_iter.as_slice()
            );
            return None;
        };

        let Some(name) = name.as_str() else {
            error!(
                "Expected event name to be str, instead got {:?}. Args: {:?}",
                name,
                args_iter.as_slice()
            );
            return None;
        };

        let events = args_iter
            .filter_map(into_array)
            .filter_map(|event| {
                let event = match name {
                    "option_set" => RedrawEvent::OptionSet(GuiOption::parse(event)?),
                    "mode_info_set" => RedrawEvent::ModeInfoSet,
                    "hl_attr_define" => RedrawEvent::HighlightAttributesDefine,
                    "hl_group_set" => RedrawEvent::HighlightGroupSet,

                    "grid_line" => {
                        let mut event = event.into_iter();

                        let grid = event.next()?.as_u64()?;
                        let row = event.next()?.as_u64()?;
                        let column_start = event.next()?.as_u64()?;

                        let cells = into_array(event.next()?)?;

                        RedrawEvent::GridLine {
                            grid,
                            row,
                            column_start,
                            cells: cells
                                .into_iter()
                                .filter_map(into_array)
                                .filter_map(GridLineCell::parse)
                                .collect(),
                        }
                    }
                    "grid_clear" => RedrawEvent::GridClear {
                        grid: event.first()?.as_u64()?,
                    },
                    "grid_destroy" => RedrawEvent::GridDestroy {
                        grid: event.first()?.as_u64()?,
                    },
                    "grid_cursor_goto" => {
                        let mut event = event.into_iter();
                        RedrawEvent::GridCursorGoto {
                            grid: event.next()?.as_u64()?,
                            row: event.next()?.as_u64()?,
                            column: event.next()?.as_u64()?,
                        }
                    }
                    "grid_scroll" => {
                        let mut event = event.into_iter();

                        RedrawEvent::GridScroll {
                            grid: event.next()?.as_u64()?,
                            top: event.next()?.as_u64()?,
                            bottom: event.next()?.as_u64()?,
                            left: event.next()?.as_u64()?,
                            right: event.next()?.as_u64()?,
                            rows: event.next()?.as_i64()?,
                            columns: event.next()?.as_i64()?,
                        }
                    }
                    "grid_resize" => {
                        let mut event = event.into_iter();
                        RedrawEvent::GridResize {
                            grid: event.next()?.as_u64()?,
                            width: event.next()?.as_u64()?,
                            height: event.next()?.as_u64()?,
                        }
                    }

                    "win_viewport" => RedrawEvent::WindowViewport,
                    "mode_change" => RedrawEvent::ModeChange,
                    "mouse_on" => RedrawEvent::MouseOn,
                    "mouse_off" => RedrawEvent::MouseOff,
                    "flush" => RedrawEvent::Flush,

                    "popupmenu_show" => {
                        let mut event = event.into_iter();

                        let items = into_array(event.next()?)?
                            .into_iter()
                            .filter_map(into_array)
                            .map(|array| {
                                array
                                    .into_iter()
                                    .flat_map(into_string)
                                    .collect::<Vec<String>>()
                            })
                            .filter_map(PopupMenuItem::new)
                            .collect();

                        RedrawEvent::PopupmenuShow {
                            items,
                            selected: u64::try_from(event.next()?.as_i64()?).ok(),
                            row: event.next()?.as_u64()?,
                            col: event.next()?.as_u64()?,
                            grid: event.next()?.as_u64()?,
                        }
                    }
                    "popupmenu_select" => {
                        let selected = u64::try_from(event.first()?.as_i64()?).ok();
                        RedrawEvent::PopupmenuSelect { selected }
                    }
                    "popupmenu_hide" => RedrawEvent::PopupmenuHide,

                    name => RedrawEvent::Unknown(name.to_string(), event),
                };

                Some(event)
            })
            .collect::<Vec<_>>();

        Some(events)
    }
}

impl NvimEvent {
    pub fn parse(name: String, args: Vec<nvim_rs::Value>) -> Option<Self> {
        let event = match name.as_ref() {
            "redraw" => {
                let args = args
                    .into_iter()
                    .filter_map(into_array)
                    .filter_map(RedrawEvent::parse)
                    .flatten();

                NvimEvent::Redraw(args.collect())
            }
            "Gui" => NvimEvent::Gui(args),
            "subscription" => NvimEvent::Subscription(args),
            "resized" => NvimEvent::Resized(args),
            _ => {
                error!("Notification {}({:?})", name, args);
                return None;
            }
        };

        Some(event)
    }
}
