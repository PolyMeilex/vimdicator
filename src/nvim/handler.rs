use std::sync::{Arc, Mutex};

use nvim_rs::{Neovim, Value};

use async_trait::async_trait;
use tokio::net::tcp::OwnedWriteHalf;
use tokio_util::compat::Compat;

use super::event::NvimEvent;
use gtk::glib;

#[derive(Debug)]
struct InnerData {
    gtk_tx: glib::Sender<NvimEvent>,
}

#[derive(Debug, Clone)]
pub struct NvimHadler {
    data: Arc<Mutex<InnerData>>,
}

impl NvimHadler {
    pub fn new(gtk_tx: glib::Sender<NvimEvent>) -> Self {
        Self {
            data: Arc::new(Mutex::new(InnerData { gtk_tx })),
        }
    }
}

#[async_trait]
impl nvim_rs::Handler for NvimHadler {
    type Writer = Compat<OwnedWriteHalf>;

    async fn handle_notify(&self, name: String, args: Vec<Value>, nvim: Neovim<Self::Writer>) {
        let event = NvimEvent::parse(name, args, nvim).unwrap();
        self.data.lock().unwrap().gtk_tx.send(event).unwrap();
    }

    async fn handle_request(
        &self,
        name: String,
        _args: Vec<Value>,
        _: Neovim<Self::Writer>,
    ) -> Result<Value, Value> {
        dbg!(name);

        Ok(nvim_rs::Value::Nil)
    }
}
