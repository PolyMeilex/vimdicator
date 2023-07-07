#[derive(Debug, Default)]
pub struct NeovimApiInfo {
    pub channel: i64,

    pub ext_cmdline: bool,
    pub ext_wildmenu: bool,
    pub ext_hlstate: bool,
    pub ext_linegrid: bool,
    pub ext_popupmenu: bool,
    pub ext_tabline: bool,
    pub ext_termcolors: bool,

    pub ui_pum_set_height: bool,
    pub ui_pum_set_bounds: bool,
}

impl NeovimApiInfo {
    pub fn new(api_info: Vec<nvim_rs::Value>) -> Result<Self, String> {
        let mut self_ = Self::default();
        let mut api_info = api_info.into_iter();

        self_.channel = api_info
            .next()
            .ok_or("Channel is missing")?
            .as_i64()
            .ok_or("Channel is not i64")?;

        let metadata = match api_info.next().ok_or("Metadata is missing")? {
            nvim_rs::Value::Map(pairs) => Ok(pairs),
            v => Err(format!("Metadata is wrong type, got {v:?}")),
        }?;

        for (key, value) in metadata.into_iter() {
            match key
                .as_str()
                .ok_or(format!("Metadata key {key:?} isn't string"))?
            {
                "ui_options" => self_.parse_ui_options(value)?,
                "functions" => self_.parse_functions(value)?,
                _ => (),
            }
        }
        Ok(self_)
    }

    #[inline]
    fn parse_ui_options(&mut self, extensions: nvim_rs::Value) -> Result<(), String> {
        for extension in extensions
            .as_array()
            .ok_or(format!("UI option list is invalid: {extensions:?}"))?
        {
            match extension
                .as_str()
                .ok_or(format!("UI option isn't string: {extensions:?}"))?
            {
                "ext_cmdline" => self.ext_cmdline = true,
                "ext_wildmenu" => self.ext_wildmenu = true,
                "ext_hlstate" => self.ext_hlstate = true,
                "ext_linegrid" => self.ext_linegrid = true,
                "ext_popupmenu" => self.ext_popupmenu = true,
                "ext_tabline" => self.ext_tabline = true,
                "ext_termcolors" => self.ext_termcolors = true,
                _ => (),
            };
        }
        Ok(())
    }

    #[inline]
    fn parse_functions(&mut self, functions: nvim_rs::Value) -> Result<(), String> {
        for function in functions
            .as_array()
            .ok_or_else(|| format!("Function list is not a list: {functions:?}"))?
        {
            match function
                .as_map()
                .ok_or_else(|| format!("Function info is not a map: {function:?}"))?
                .iter()
                .find_map(|(key, value)| {
                    key.as_str()
                        .filter(|k| *k == "name")
                        .and_then(|_| value.as_str())
                })
                .ok_or_else(|| format!("Function info is missing name: {functions:?}"))?
            {
                "nvim_ui_pum_set_height" => self.ui_pum_set_height = true,
                "nvim_ui_pum_set_bounds" => self.ui_pum_set_bounds = true,
                _ => (),
            }
        }
        Ok(())
    }
}
