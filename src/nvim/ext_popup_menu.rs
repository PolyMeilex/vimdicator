use super::event::PopupMenuItem;

#[derive(Debug)]
pub struct ExtPopupMenuState {
    pub items: Vec<PopupMenuItem>,
    pub selected: Option<usize>,
    pub row: usize,
    pub col: usize,
    pub grid: u64,
}

#[derive(Debug, Default)]
pub struct ExtPopupMenu {
    state: Option<ExtPopupMenuState>,
}

impl ExtPopupMenu {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self) -> Option<&ExtPopupMenuState> {
        self.state.as_ref()
    }

    pub fn show(
        &mut self,
        items: Vec<PopupMenuItem>,
        selected: Option<usize>,
        row: usize,
        col: usize,
        grid: u64,
    ) {
        self.state = Some(ExtPopupMenuState {
            items,
            selected,
            row,
            col,
            grid,
        })
    }

    pub fn hide(&mut self) {
        self.state = None;
    }

    pub fn select(&mut self, selected: Option<usize>) {
        if let Some(state) = self.state.as_mut() {
            state.selected = selected;
        }
    }
}
