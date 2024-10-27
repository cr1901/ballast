use eframe;
use eframe::egui::menu::{self};
use eframe::egui::{self, Align2, Button, Context, TextEdit, Ui, Widget, WidgetText};
use egui_toast::{Toast, ToastKind, ToastOptions, Toasts};
use log::{debug, warn};

use crate::doc::AppAction;
use crate::retrieval::CancelSend;
use crate::url::{UrlStack, UrlType};

use super::doc::{self, Document};
use super::retrieval::{self, CmdSend, RespRecv};
use super::url::NexUrl;

#[derive(PartialEq)]
enum ControlFlow {
    Waiting,
    Init,
    Presenting,
}

pub struct Ballast {
    state: ControlFlow,
    cmd: CmdSend,
    cancel: CancelSend,
    /// Url in the address bar.
    url_string: String,
    /// In-memory repr of a document.
    doc: Box<dyn Document>,
    /// Current URL.
    curr_url: Option<UrlType>,
    /// History of visited URLs.
    url_stack: UrlStack,
    resp: Option<RespRecv>,
    toasts: Toasts,
}

impl Ballast {
    pub fn new() -> Self {
        let (cmd, cancel) = retrieval::spawn();
        // let nex_string = RefCell::new(String::new());
        // let mut link_present: Vec<Option<Url>> = Vec::new();

        Self {
            state: ControlFlow::Waiting,
            cmd,
            cancel,
            url_string: String::new(),
            curr_url: None,
            url_stack: UrlStack::new(),
            doc: Box::new(doc::Null::new()),
            resp: None,
            toasts: Toasts::new()
                .anchor(Align2::RIGHT_BOTTOM, (-10.0, -10.0)) // 10 units from the bottom right corner
                .direction(egui::Direction::BottomUp),
        }
    }

    pub fn do_home_page(&mut self) {
        self.url_string = String::from("nex://nex.nightfall.city/");
        self.curr_url = Some(UrlType::Nex(
            NexUrl::try_from("nex://nex.nightfall.city/")
                .expect("home page should be a valid NEX URL"),
        ));
        self.start_new_url();
    }

    fn start_new_url(&mut self) {
        debug!(target: "nex-ballast-fg", "start_new_url {:?}", self.curr_url.as_ref().unwrap().to_string());

        self.url_stack.push(self.curr_url.as_ref().unwrap().clone());
        self.do_url();
    }

    fn get_previous_url(&mut self) {
        debug!(target: "nex-ballast-fg", "get_previous_url {:?}", self.curr_url.as_ref().unwrap().to_string());
        self.do_url();
    }

    fn do_url(&mut self) {
        let (send, recv) = oneshot::channel();

        // let url_string = url.to_string();
        // self.url_string = url_string.clone();
        let _ = self
            .cmd
            .send((self.curr_url.as_ref().unwrap().clone(), send));

        self.state = ControlFlow::Waiting;
        self.resp = Some(recv);
    }

    fn stop_url(&self) {
        if let Err(_) = self.cancel.try_send(()) {
            warn!(target: "nex-ballast-fg", "unexpected cancel request in queue");
        }
    }

    fn toast<T>(&mut self, text: T, kind: ToastKind)
    where
        T: Into<WidgetText>,
    {
        self.toasts.add(Toast {
            text: text.into(),
            kind,
            options: ToastOptions::default()
                .duration_in_seconds(5.0)
                .show_progress(true),
            ..Default::default()
        });
    }
}

impl eframe::App for Ballast {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        match ui_address_bar(self, ctx) {
            Some(AddressBarAction::StartNewUrlBar) => {
                match UrlType::try_from(self.url_string.as_str()) {
                    Ok(url @ UrlType::Nex(_)) => {
                        self.url_string = url.to_string();
                        self.curr_url = Some(url);
                        self.start_new_url();
                    }
                    Err(_) => {
                        debug!(target: "nex-ballast-fg", "url didn't parse as supported... {:?}", self.url_string)
                    }
                }
            }
            Some(AddressBarAction::Unsupported(msg)) => {
                self.toast(format!("Unsupported feature: {}", msg), ToastKind::Info);
            }
            Some(AddressBarAction::StartNewUrlBackFwd(u)) => {
                self.url_string = u.to_string();
                self.curr_url = Some(u);
                self.get_previous_url();
            }
            Some(AddressBarAction::CancelLoad) => {
                self.stop_url();
            }
            Some(AddressBarAction::StartHomePage) => {
                if self.state == ControlFlow::Waiting {
                    self.stop_url();
                }
                self.do_home_page();
            }
            None => {}
        }

        egui::CentralPanel::default().show(ctx, |ui| match &self.state {
            ControlFlow::Waiting => {
                ui_spinner(ui);
                if let Some(ref mut recv) = self.resp {
                    match recv.try_recv() {
                        Ok(r) => {
                            self.doc = Box::from((r, &self.curr_url));
                            self.state = ControlFlow::Init;
                        }
                        _ => {}
                    }
                }
            }
            ControlFlow::Init => {
                ui_spinner(ui);
                match self.doc.init(ui, ctx) {
                    AppAction::StartNewUrl(_) => {
                        unreachable!()
                        /* match UrlType::try_from(url.as_str()) {
                            Ok(url @ UrlType::Nex(_)) => {
                                self.url_string = url.to_string();
                                self.curr_url = Some(url);
                                self.start_new_url();
                            }
                            Err(_) => debug!(target: "nex-ballast-fg", "url didn't parse as supported... {:?}", url.as_str())
                        } */
                    }
                    AppAction::Toast { kind, text } => { self.toast(text, kind) }
                    AppAction::None => {}
                }

                self.toasts.show(ctx);
                self.state = ControlFlow::Presenting;
            },
            ControlFlow::Presenting => {
                /* let do_find = false; */

                match self.doc.present(ui, ctx) {
                    AppAction::StartNewUrl(url) => {
                        match UrlType::try_from(url.as_str()) {
                            Ok(url @ UrlType::Nex(_)) => {
                                self.url_string = url.to_string();
                                self.curr_url = Some(url);
                                self.start_new_url();
                            }
                            Err(_) => debug!(target: "nex-ballast-fg", "url didn't parse as supported... {:?}", url.as_str())
                        }
                    }
                    AppAction::Toast { kind, text } => { self.toast(text, kind) }
                    AppAction::None => {}
                }

                self.toasts.show(ctx);
                /* TODO: Find logic should go here- should be trait method for Document? */
            },
        });
    }
}

enum AddressBarAction {
    StartNewUrlBar,
    StartNewUrlBackFwd(UrlType),
    Unsupported(&'static str),
    CancelLoad,
    StartHomePage,
}

fn ui_address_bar(ballast: &mut Ballast, ctx: &Context) -> Option<AddressBarAction> {
    let mut action = None;

    egui::TopBottomPanel::top("address_bar")
        .resizable(false)
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x *= 0.5;
                let max_rect = ui.max_rect();
                let width = max_rect.width();

                // FIXME: How do I set this based on menu size?
                // I want to right-justify menu and set address bar as a function
                // of menu bar size. Right now, everything _barely_ fits into 640 px.
                let response = TextEdit::singleline(&mut ballast.url_string)
                    .desired_width(width * 0.80)
                    .ui(ui);
                if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    action = Some(AddressBarAction::StartNewUrlBar);
                }

                menu::bar(ui, |ui| {
                    if ui.button("\u{1f5d9}").clicked() && ballast.state == ControlFlow::Waiting {
                        action = Some(AddressBarAction::CancelLoad);
                    }

                    if ui.button("\u{1f3e0}").clicked() {
                        action = Some(AddressBarAction::StartHomePage);
                    }

                    // TODO: Figure out how menus can overflow their container.
                    // egui "does the right thing" here, but it's still rather
                    // magic to me...
                    // Menus have shadows, so they're a different egui Layer?
                    let mut clicked = None;
                    ui.menu_button("\u{21a9}\u{21aa}", |ui| {
                        for (i, u) in ballast.url_stack.iter() {
                            ui.set_max_width(200.0);
                            if Some(i.into()) == ballast.url_stack.ptr() {
                                if Button::new(format!("\u{2705} {}", u.to_string()))
                                    .wrap_mode(egui::TextWrapMode::Extend)
                                    .ui(ui)
                                    .clicked()
                                {
                                    clicked = Some((i, u.clone()));
                                }
                            } else {
                                if Button::new(format!("{}", u.to_string()))
                                    .wrap_mode(egui::TextWrapMode::Extend)
                                    .ui(ui)
                                    .clicked()
                                {
                                    clicked = Some((i, u.clone()));
                                }
                            }
                        }
                    });

                    if let Some((i, u)) = clicked {
                        ballast.url_stack.set_ptr(i);
                        action = Some(AddressBarAction::StartNewUrlBackFwd(u));
                        debug!(target: "nex-ballast-fg", "url clicked... {:?}", i);
                    }

                    if ui.button("\u{1f4be}").clicked() {
                        action = Some(AddressBarAction::Unsupported("Download"));
                    }

                    if ui.button("\u{1f50d}").clicked() {
                        action = Some(AddressBarAction::Unsupported("Find"));
                    }
                });
            })
        });

    action
}

fn ui_spinner(ui: &mut Ui) {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.centered_and_justified(|ui| {
                let height = ui.max_rect().height();
                ui.add(egui::widgets::Spinner::new().size(height));
            })
        });
}
