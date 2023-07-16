#[derive(Debug, Default, Clone)]
pub struct ExtTabline {
    current_tab: Option<super::Tabpage>,
    tabs: Vec<(String, super::Tabpage)>,
}

impl ExtTabline {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(&mut self, current_tab: super::Tabpage, tabs: Vec<(String, super::Tabpage)>) {
        self.current_tab = Some(current_tab);
        self.tabs = tabs;
    }

    pub fn current_tab(&self) -> Option<&super::Tabpage> {
        self.current_tab.as_ref()
    }

    pub fn tabs(&self) -> &[(String, super::Tabpage)] {
        &self.tabs
    }
}
