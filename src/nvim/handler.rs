use std::{
    result,
    sync::{mpsc, Arc},
};

use log::{debug, error};

use nvim_rs::{compat::tokio::Compat, Handler, Value};

use async_trait::async_trait;

use crate::shell;
use crate::{
    nvim::{Neovim, NvimWriter},
    ui::UiMutex,
};

use super::{
    redraw_handler::{self, PendingPopupMenu, RedrawMode},
    NvimEvent, NvimHandlerEvent, NvimRequest,
};

pub struct NvimHandler {
    tx: glib::Sender<NvimHandlerEvent>,
}

impl NvimHandler {
    pub fn new(tx: glib::Sender<NvimHandlerEvent>) -> Self {
        NvimHandler { tx }
    }

    async fn nvim_cb(&self, method: String, params: Vec<Value>) {
        let event = match method.as_ref() {
            "redraw" => NvimEvent::Redraw(params),
            "Gui" => NvimEvent::Gui(params),
            "subscription" => NvimEvent::Subscription(params),
            "resized" => NvimEvent::Resized(params),
            _ => {
                error!("Notification {}({:?})", method, params);
                return;
            }
        };

        self.tx.send(NvimHandlerEvent::Event(event)).unwrap();
    }

    fn nvim_cb_req(&self, method: String, params: Vec<Value>) -> result::Result<Value, Value> {
        match method.as_ref() {
            "Gui" => {
                if !params.is_empty() {
                    let mut params_iter = params.into_iter();
                    if let Some(req_name) = params_iter.next() {
                        if let Value::String(req_name) = req_name {
                            let args = params_iter.collect();
                            let (sender, receiver) = mpsc::channel::<Result<Value, Value>>();

                            self.tx
                                .send(NvimHandlerEvent::Request(NvimRequest::Gui {
                                    req_name: req_name
                                        .into_str()
                                        .ok_or("Event name does not exists")?,
                                    args,
                                    response: sender,
                                }))
                                .unwrap();

                            Ok(receiver.recv().unwrap()?)
                        } else {
                            error!("Unsupported request");
                            Err(Value::Nil)
                        }
                    } else {
                        error!("Request name does not exist");
                        Err(Value::Nil)
                    }
                } else {
                    error!("Unsupported request {:?}", params);
                    Err(Value::Nil)
                }
            }
            _ => {
                error!("Request {}({:?})", method, params);
                Err(Value::Nil)
            }
        }
    }
}

pub fn nvim_cb(
    shell: Arc<UiMutex<shell::State>>,
    resize_status: Arc<shell::ResizeState>,
    event: NvimEvent,
) {
    match event {
        NvimEvent::Redraw(params) => {
            wrap(|| call_redraw_handler(params, &shell));
        }
        NvimEvent::Gui(params) => {
            if !params.is_empty() {
                let mut params_iter = params.into_iter();
                if let Some(ev_name) = params_iter.next() {
                    if let Value::String(ev_name) = ev_name {
                        let args = params_iter.collect();

                        wrap(|| {
                            let ui = &mut shell.borrow_mut();
                            redraw_handler::call_gui_event(
                                ui,
                                ev_name.as_str().ok_or("Event name does not exists")?,
                                args,
                            )?;
                            ui.queue_draw(RedrawMode::All);
                            Ok(())
                        });
                    } else {
                        error!("Unsupported event");
                    }
                } else {
                    error!("Event name does not exists");
                }
            } else {
                error!("Unsupported event {:?}", params);
            }
        }
        NvimEvent::Subscription(params) => {
            wrap(|| shell.borrow().notify(params));
        }
        NvimEvent::Resized(_) => {
            debug!("Received resized notification");
            resize_status.notify_finished();
        }
    }
}

pub fn nvim_req(shell: Arc<UiMutex<shell::State>>, request: NvimRequest) {
    match request {
        NvimRequest::Gui {
            args,
            req_name,
            response,
        } => {
            response
                .send(redraw_handler::call_gui_request(
                    &shell.clone(),
                    &req_name,
                    &args,
                ))
                .unwrap();

            let ui = &mut shell.borrow_mut();
            ui.queue_draw(RedrawMode::All);
        }
    }
}

fn call_redraw_handler(
    params: Vec<Value>,
    ui: &Arc<UiMutex<shell::State>>,
) -> result::Result<(), String> {
    let mut repaint_mode = RedrawMode::Nothing;
    let mut pending_popupmenu = PendingPopupMenu::None;

    let mut ui_ref = ui.borrow_mut();
    for ev in params {
        let ev_args = match ev {
            Value::Array(args) => args,
            _ => {
                error!("Unsupported event type: {:?}", ev);
                continue;
            }
        };
        let mut args_iter = ev_args.into_iter();
        let ev_name = match args_iter.next() {
            Some(ev_name) => ev_name,
            None => {
                error!(
                    "No name provided with redraw event, args: {:?}",
                    args_iter.as_slice()
                );
                continue;
            }
        };
        let ev_name = match ev_name.as_str() {
            Some(ev_name) => ev_name,
            None => {
                error!(
                    "Expected event name to be str, instead got {:?}. Args: {:?}",
                    ev_name,
                    args_iter.as_slice()
                );
                continue;
            }
        };

        for local_args in args_iter {
            let args = match local_args {
                Value::Array(ar) => ar,
                _ => vec![],
            };

            let (call_repaint_mode, call_popupmenu) =
                match redraw_handler::call(&mut ui_ref, ev_name, args) {
                    Ok(mode) => mode,
                    Err(desc) => return Err(format!("Event {ev_name}\n{desc}")),
                };
            repaint_mode = repaint_mode.max(call_repaint_mode);
            pending_popupmenu.update(call_popupmenu);
        }
    }

    ui_ref.queue_draw(repaint_mode);
    drop(ui_ref);
    ui.borrow().popupmenu_flush(pending_popupmenu);
    Ok(())
}

fn wrap<F>(cb: F)
where
    F: FnOnce() -> result::Result<(), String>,
{
    if let Err(msg) = cb() {
        error!("Error call function: {}", msg);
    }
}

impl Clone for NvimHandler {
    fn clone(&self) -> Self {
        NvimHandler {
            tx: self.tx.clone(),
        }
    }
}

#[async_trait]
impl Handler for NvimHandler {
    type Writer = Compat<NvimWriter>;

    async fn handle_notify(&self, name: String, args: Vec<Value>, _: Neovim) {
        self.nvim_cb(name, args).await;
    }

    async fn handle_request(
        &self,
        name: String,
        args: Vec<Value>,
        _: Neovim,
    ) -> result::Result<Value, Value> {
        self.nvim_cb_req(name, args)
    }
}
