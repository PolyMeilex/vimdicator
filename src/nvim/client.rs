use std::{
    cell::Cell,
    sync::RwLock,
};

use crate::nvim::NvimSession;

#[derive(Clone, Copy, PartialEq)]
enum NeovimClientState {
    Uninitialized,
    InitInProgress,
    Initialized,
    Error,
}

pub struct NeovimClient {
    state: Cell<NeovimClientState>,
    nvim: RwLock<Option<NvimSession>>,
}

impl NeovimClient {
    pub fn new() -> Self {
        NeovimClient {
            state: Cell::new(NeovimClientState::Uninitialized),
            nvim: RwLock::new(None),
        }
    }

    pub fn clear(&self) {
        *self.nvim.write().unwrap() = None
    }

    pub fn set(&self, nvim: NvimSession) {
        self.nvim.write().unwrap().replace(nvim);
    }

    pub fn set_initialized(&self) {
        self.state.set(NeovimClientState::Initialized);
    }

    pub fn set_error(&self) {
        self.state.set(NeovimClientState::Error);
    }

    pub fn set_in_progress(&self) {
        self.state.set(NeovimClientState::InitInProgress);
    }

    pub fn is_initialized(&self) -> bool {
        self.state.get() == NeovimClientState::Initialized
    }

    pub fn is_uninitialized(&self) -> bool {
        self.state.get() == NeovimClientState::Uninitialized
    }

    pub fn is_initializing(&self) -> bool {
        self.state.get() == NeovimClientState::InitInProgress
    }

    pub fn nvim(&self) -> Option<NvimSession> {
        self.nvim.read().unwrap().clone()
    }
}
