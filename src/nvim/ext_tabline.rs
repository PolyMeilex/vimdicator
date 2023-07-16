#[derive(Debug, Default, Clone)]
pub struct ExtTabline {
    current_tab: Option<crate::Tabpage>,
    tabs: Vec<(String, crate::Tabpage)>,
}

impl ExtTabline {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(&mut self, current_tab: crate::Tabpage, tabs: Vec<(String, crate::Tabpage)>) {
        self.current_tab = Some(current_tab);
        self.tabs = tabs;
    }

    pub fn current_tab(&self) -> Option<&crate::Tabpage> {
        self.current_tab.as_ref()
    }

    pub fn tabs(&self) -> &[(String, crate::Tabpage)] {
        &self.tabs
    }
}
