use std::str::Lines;

use eframe;
use eframe::egui::menu::{self};
use eframe::egui::{self, Align2, Button, Context, TextEdit, Ui, Widget};
use egui_toast::{Toast, ToastKind, ToastOptions, Toasts};
use log::{debug, warn};
use url::Url;

use crate::retrieval::CancelSend;
use crate::url::UrlStack;

use super::retrieval::{self, CmdSend, RespRecv};
use super::url::NexUrl;

#[derive(PartialEq)]
enum ControlFlow {
    Waiting,
    TextDoc,
}

/* struct Document {
    raw: String,
    typ: DocType
}

enum DocType {
    Nex(NexType),
}

enum NexType {
    Directory {
        links: Vec<Option<Url>>
    }
} */

pub struct Ballast {
    state: ControlFlow,
    cmd: CmdSend,
    cancel: CancelSend,
    /// Url in the address bar.
    url_string: String,
    raw: String,
    links: Vec<Option<Url>>,
    /// Current URL.
    nex_url: Option<NexUrl>,
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
            nex_url: None,
            url_stack: UrlStack::new(),
            raw: String::new(),
            links: Vec::new(),
            resp: None,
            toasts: Toasts::new()
                .anchor(Align2::RIGHT_BOTTOM, (-10.0, -10.0)) // 10 units from the bottom right corner
                .direction(egui::Direction::BottomUp),
        }
    }

    pub fn do_home_page(&mut self) {
        self.url_string = String::from("nex://nex.nightfall.city/");
        self.nex_url = Some(
            NexUrl::try_from("nex://nex.nightfall.city/")
                .expect("home page should be a valid NEX URL"),
        );
        self.start_new_url();
    }

    fn start_new_url(&mut self) {
        // debug!(target: "nex-ballast-fg", "start_new_url {:?}", url.to_string());

        self.url_stack.push(self.nex_url.as_ref().unwrap().clone());
        self.do_url();
    }

    fn get_previous_url(&mut self) {
        // debug!(target: "nex-ballast-fg", "start_new_url {:?}", url.to_string());

        /* match self.url_stack_ptr {
            None => {
                unreachable!()
            },
            Some(ref mut ptr) => {
                let len = self.url_stack.len();

                assert!(*ptr < len);
                *ptr -= 1;

                self.nex_url = Some(self.url_stack[*ptr].clone());
            }
        } */

        self.do_url();
    }

    fn do_url(&mut self) {
        let (send, recv) = oneshot::channel();

        // let url_string = url.to_string();
        // self.url_string = url_string.clone();
        self.cmd
            .send((self.nex_url.as_ref().unwrap().clone(), send));
        self.links.clear();
        /* match self.doc {
            Some(Document {
                typ: DocType::Nex(NexType::Directory { links }),
                ..
            })  => {
                links.clear();
            }
            _ => {}
        } */

        self.state = ControlFlow::Waiting;
        self.resp = Some(recv);
    }

    fn stop_url(&self) {
        if let Err(_) = self.cancel.try_send(()) {
            warn!(target: "nex-ballast-fg", "unexpected cancel request in queue");
        }
    }
}

impl eframe::App for Ballast {
    fn update(&mut self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame) {
        match ui_address_bar(self, ctx) {
            Some(AddressBarAction::StartNewUrlBar) => {
                if let Ok(nex_url) = NexUrl::try_from(&*self.url_string) {
                    self.nex_url = Some(nex_url);
                    self.start_new_url();
                } else {
                    debug!(target: "nex-ballast-fg", "url didn't parse as NEX... {:?}", &self.url_string);
                }
            }
            Some(AddressBarAction::Unsupported(msg)) => {
                self.toasts.add(Toast {
                    text: format!("Unsupported feature: {}", msg).into(),
                    kind: ToastKind::Info,
                    options: ToastOptions::default()
                        .duration_in_seconds(5.0)
                        .show_progress(true),
                    ..Default::default()
                });
            }
            Some(AddressBarAction::StartNewUrlBackFwd(u)) => {
                self.url_string = u.to_string();
                self.nex_url = Some(u);
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

        egui::CentralPanel::default().show(ctx, |ui| match self.state {
            ControlFlow::Waiting => {
                ui_spinner(ui);
                if let Some(ref mut recv) = self.resp {
                    match recv.try_recv() {
                        Ok(Ok(recv)) => {
                            self.raw = recv;
                            self.links.clear();
                            self.state = ControlFlow::TextDoc;

                            if self.raw.contains('\u{fffd}') {
                                self.toasts.add(Toast {
                                    text: "UTF-8, replacement character detected.\nThis is probably a (unsupported) binary file.".into(),
                                    kind: ToastKind::Warning,
                                    options: ToastOptions::default()
                                        .duration_in_seconds(5.0)
                                        .show_progress(true),
                                    ..Default::default()
                                });
                            }
                        }
                        Ok(Err(r)) => {
                            self.raw = format!("Error resolving {}:\n{}", self.nex_url.as_ref().unwrap().to_string(), r.to_string());
                            self.links.clear();
                            self.state = ControlFlow::TextDoc;
                        }
                        _ => {}
                    }
                }

                self.toasts.show(ctx);
            }
            ControlFlow::TextDoc => {
                match ui_textdoc(ui, ctx, self.raw.lines(), &mut self.links, &self.url_string, &mut self.toasts) {
                    Some(TextDocAction::StartNewUrl(url)) => {
                        if let Ok(nex_url) = NexUrl::try_from(url.as_str()) {
                            debug!(target: "nex-ballast-fg", "url parsed as NEX... {}, {:?}", url.as_str(), nex_url);
                            self.url_string = url.to_string();
                            self.nex_url = Some(nex_url);
                            self.start_new_url();
                        } else {
                            debug!(target: "nex-ballast-fg", "url didn't parse as NEX... {:?}", url.as_str());
                        }
                    },
                    None => {}
                }
            }
        });
    }
}

enum AddressBarAction {
    StartNewUrlBar,
    StartNewUrlBackFwd(NexUrl),
    Unsupported(&'static str),
    CancelLoad,
    StartHomePage
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
                                    .ui(ui).clicked() {
                                        clicked = Some((i, u.clone()));
                                    }
                            } else {
                                if Button::new(format!("{}", u.to_string()))
                                    .wrap_mode(egui::TextWrapMode::Extend)
                                    .ui(ui).clicked() {
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

enum TextDocAction {
    StartNewUrl(String),
}

fn ui_textdoc(
    ui: &mut Ui,
    ctx: &eframe::egui::Context,
    lines: Lines,
    links: &mut Vec<Option<Url>>,
    addr_str: &String,
    toasts: &mut Toasts,
) -> Option<TextDocAction> {
    let mut action = None;

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for (i, line) in lines.enumerate() {
                match links.get(i) {
                    // FIXME: Need to handle links which don't end with extension
                    // or '/'... they are currently relative to parent.
                    Some(Some(url)) if line.starts_with("=> ") => {
                        let mut start_new = false;

                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing.x = 0.0;
                            let url_end = line[3..].find(' ').unwrap_or(line.len() - 3) + 3;

                            ui.label(egui::RichText::new("=> ").monospace());
                            if ui.link(&line[3..url_end]).clicked() {
                                start_new = true;
                            }

                            if url_end < line.len() {
                                ui.label(egui::RichText::new(&line[url_end..]).monospace());
                            }
                        });

                        if start_new {
                            action = Some(TextDocAction::StartNewUrl(url.to_string()));
                            return;
                        }
                    }
                    Some(Some(_)) => {
                        unreachable!()
                    }
                    Some(None) => {
                        ui.label(egui::RichText::new(line).monospace());
                    }
                    None if line.starts_with("=> ") => {
                        assert!(links.len() == i);
                        let url_end = line[3..].find(' ').unwrap_or(line.len() - 3) + 3;

                        match Url::parse(&line[3..url_end]) {
                            Ok(url) => {
                                let mut start_new = false;

                                ui.horizontal_wrapped(|ui| {
                                    ui.spacing_mut().item_spacing.x = 0.0;

                                    ui.label(egui::RichText::new("=> ").monospace());
                                    if ui.link(&line[3..url_end]).clicked() {
                                        start_new = true;
                                    }

                                    if url_end < line.len() {
                                        ui.label(egui::RichText::new(&line[url_end..]).monospace());
                                    }
                                });

                                if start_new {
                                    action = Some(TextDocAction::StartNewUrl(url.to_string()));
                                    return;
                                }

                                links.push(Some(url.clone()));
                            }
                            Err(_) => {
                                let abs_url = match Url::parse(addr_str) {
                                    Ok(url) => {
                                        if !url.path().ends_with('/') && !url.path().contains('.') {
                                            let new_url = url.join(&format!("{}/{}", url.path(), &line[3..url_end]));
                                            debug!(target: "nex-ballast-fg", "fixing up nex directory without trailing slash {:?} => {:?}", url, new_url);
                                            new_url
                                        } else {
                                            url.join(&line[3..url_end])
                                        }
                                    }
                                    Err(_) => {
                                        links.push(None);
                                        ui.label(egui::RichText::new(line).monospace());
                                        continue;
                                    }
                                };
                                // FIXME: Render relative links and start new url here too?
                                debug!(target: "nex-ballast-fg", "url didn't parse... treating as relative {:?}", &abs_url);
                                match abs_url
                                {
                                    Ok(url) => {
                                        links.push(Some(url.clone()));
                                    }
                                    Err(_) => {
                                        links.push(None);
                                        ui.label(egui::RichText::new(line).monospace());
                                        continue;
                                    }
                                }
                            }
                        }
                    }
                    None => {
                        links.push(None);
                        ui.label(egui::RichText::new(format!("{}\n", line)).monospace());
                    }
                }
            }

            toasts.show(ctx);
        });

    action
}
