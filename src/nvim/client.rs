use std::{
    cell::RefCell,
    convert::*,
    rc::Rc,
    sync::RwLock,
};

use crate::nvim::*;

#[derive(Default)]
pub struct NeovimApiInfo {
    pub channel: i64,
}

impl NeovimApiInfo {
    pub fn new(api_info: Vec<Value>) -> Result<Self, String> {
        let mut self_ = Self::default();
        let mut api_info = api_info.into_iter();

        self_.channel = api_info
            .next()
            .ok_or("Channel is missing")?
            .as_i64()
            .ok_or("Channel is not i64")?;

        Ok(self_)
    }
}

#[derive(Clone, Copy, PartialEq)]
enum NeovimClientStatus {
    Uninitialized,
    InitInProgress,
    Initialized,
    Error,
}

struct NeovimClientState {
    status: NeovimClientStatus,
    api_info: Option<Rc<NeovimApiInfo>>,
}

pub struct NeovimClient {
    state: RefCell<NeovimClientState>,
    nvim: RwLock<Option<NvimSession>>,
}

impl NeovimClient {
    pub fn new() -> Self {
        NeovimClient {
            state: RefCell::new(NeovimClientState {
                status: NeovimClientStatus::Uninitialized,
                api_info: None,
            }),
            nvim: RwLock::new(None),
        }
    }

    pub fn clear(&self) {
        *self.nvim.write().unwrap() = None
    }

    pub fn set(&self, nvim: NvimSession) {
        self.nvim.write().unwrap().replace(nvim);
    }

    pub fn api_info(&self) -> Rc<NeovimApiInfo> {
        self.state
            .borrow()
            .api_info
            .as_ref()
            .expect("API info should be initialized by the time this is called")
            .clone()
    }

    pub fn set_initialized(&self, api_info: NeovimApiInfo) {
        let mut state = self.state.borrow_mut();

        state.status = NeovimClientStatus::Initialized;
        state.api_info = Some(Rc::new(api_info));
    }

    pub fn set_error(&self) {
        self.state.borrow_mut().status = NeovimClientStatus::Error;
    }

    pub fn set_in_progress(&self) {
        self.state.borrow_mut().status = NeovimClientStatus::InitInProgress;
    }

    pub fn is_initialized(&self) -> bool {
        self.state.borrow().status == NeovimClientStatus::Initialized
    }

    pub fn is_uninitialized(&self) -> bool {
        self.state.borrow().status == NeovimClientStatus::Uninitialized
    }

    pub fn is_initializing(&self) -> bool {
        self.state.borrow().status == NeovimClientStatus::InitInProgress
    }

    pub fn nvim(&self) -> Option<NvimSession> {
        self.nvim.read().unwrap().clone()
    }
}
