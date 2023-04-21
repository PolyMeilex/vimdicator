use std::path::PathBuf;

use crate::dirs;

#[derive(Clone)]
pub struct NvimConfig {}

impl NvimConfig {
    const CONFIG_PATH: &'static str = "settings.vim";

    pub fn config_path() -> Option<PathBuf> {
        let mut path = dirs::app_config_dir().to_path_buf();
        path.push(NvimConfig::CONFIG_PATH);
        if path.is_file() {
            return Some(path);
        }

        None
    }
}
