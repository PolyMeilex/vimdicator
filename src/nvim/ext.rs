use std::error::Error;

use nvim_rs::error::CallError;

use crate::nvim::SessionError;

pub trait CallErrorExt {
    fn print(&self);
}
impl CallErrorExt for CallError {
    fn print(&self) {
        error!("Error in last Neovim request: {}", self);
        error!("Caused by: {:?}", self.source());
    }
}

pub trait ErrorReport<T> {
    fn report_err(&self);

    fn ok_and_report(self) -> Option<T>;
}

impl<T> ErrorReport<T> for Result<T, SessionError> {
    fn report_err(&self) {
        if let Err(ref err) = self {
            match *err {
                SessionError::CallError(ref e) => e.print(),
                SessionError::TimeoutError(ref e) => {
                    panic!("Neovim request {:?} timed out", e.source());
                }
            }
        }
    }

    fn ok_and_report(self) -> Option<T> {
        self.report_err();
        Some(self.unwrap())
    }
}
