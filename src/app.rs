use std::str::Lines;

use eframe;
use eframe::egui::load::Bytes;
use eframe::egui::menu::{self};
use eframe::egui::{self, Align2, Button, Context, ImageSource, TextEdit, Ui, Widget, WidgetText};
use egui_toast::{Toast, ToastKind, ToastOptions, Toasts};
use log::{debug, warn};
use url::Url;

use crate::retrieval::CancelSend;
use crate::url::{UrlStack, UrlType};

use super::retrieval::{self, CmdSend, RespRecv};
use super::url::NexUrl;

#[derive(PartialEq)]
enum ControlFlow {
    Waiting,
    Rendering,
    Presenting
}

/// Return type from BG thread; Null and Error will never be returned from it.
pub enum Document {
    /// Replacement for Option::None.
    Null,
    Error(String),
    NexDirectory { raw: String, links: Vec<Option<Url>> },
    Jpeg { raw: Bytes },
}

pub struct Ballast {
    state: ControlFlow,
    cmd: CmdSend,
    cancel: CancelSend,
    /// Url in the address bar.
    url_string: String,
    /// In-memory repr of a document.
    doc: Document,
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
            doc: Document::Null,
            resp: None,
            toasts: Toasts::new()
                .anchor(Align2::RIGHT_BOTTOM, (-10.0, -10.0)) // 10 units from the bottom right corner
                .direction(egui::Direction::BottomUp),
        }
    }

    pub fn do_home_page(&mut self) {
        self.url_string = String::from("nex://nex.nightfall.city/");
        self.curr_url = Some(
            UrlType::Nex(NexUrl::try_from("nex://nex.nightfall.city/")
                .expect("home page should be a valid NEX URL")),
        );
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

    fn toast<T>(&mut self, text: T, kind: ToastKind) where T: Into<WidgetText> {
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
                    Err(_) => debug!(target: "nex-ballast-fg", "url didn't parse as supported... {:?}", self.url_string)
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
                        Ok(Ok(bytes)) => {
                            self.doc = match &self.curr_url {
                                Some(UrlType::Nex(url)) => {
                                    if url.selector().ends_with(".jpg") || url.selector().ends_with(".jpeg") {
                                        Document::Jpeg { raw: bytes.into() }
                                    } else {
                                        Document::NexDirectory {
                                            raw: String::from_utf8_lossy(&bytes).into_owned(),
                                            links: Vec::new()
                                        }
                                    }
                                }
                                None => unreachable!()
                            };
                            self.state = ControlFlow::Rendering;
                        },
                        Ok(Err(r)) => {
                            let err_string = format!("Error resolving {}:\n{}", self.curr_url.as_ref().unwrap().to_string(), r.to_string());
                            self.doc = Document::Error(err_string);
                            self.state = ControlFlow::Presenting;
                        }
                        _ => {}
                    }
                }
            }
            ControlFlow::Rendering => {
                ui_spinner(ui);
                match &self.doc {
                    Document::NexDirectory { raw, .. } => {
                        self.state = ControlFlow::Presenting;

                        if raw.contains('\u{fffd}') {
                            self.toast("UTF-8, replacement character detected.\nThis is probably a (unsupported) binary file.", ToastKind::Warning);
                        }
                    },
                    Document::Jpeg { .. } => {
                        ctx.forget_image("bytes://ballast-image");
                        self.state = ControlFlow::Presenting;
                    }
                    _ => unreachable!()
                }
            },
            ControlFlow::Presenting => {
                /* let do_find = false; */
                match &mut self.doc {
                    Document::NexDirectory { ref mut raw, ref mut links } => {
                        match ui_nexdir(ui, ctx, raw.lines(), links, &self.url_string, &mut self.toasts) {
                            Some(TextDocAction::StartNewUrl(url)) => {
                                match UrlType::try_from(url.as_str()) {
                                    Ok(url @ UrlType::Nex(_)) => {
                                        self.url_string = url.to_string();
                                        self.curr_url = Some(url);
                                        self.start_new_url();
                                    }
                                    Err(_) => debug!(target: "nex-ballast-fg", "url didn't parse as supported... {:?}", url.as_str())
                                }
                            },
                            None => {}
                        }
                    },
                    Document::Jpeg { ref raw} => {
                        egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
                            ui.image(ImageSource::Bytes {
                                uri: "bytes://ballast-image".into(),
                                bytes: raw.clone()
                            });
                        });
                    },
                    Document::Error(raw) => {
                        for line in raw.lines() {
                            ui.label(egui::RichText::new(line).monospace());
                        }
                    },
                    Document::Null => {},
                    _ => unimplemented!()
                }

                /* TODO: Find logic should go here, and be coupled to TextDoc variant? */
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

enum TextDocAction {
    StartNewUrl(String),
}

fn ui_nexdir(
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
                    Some(Some(url)) if line.starts_with("=> ") => {
                        if let Some(a) = ui_hyperlink(ui, &line, &url) {
                            action = Some(a);
                            return;
                        }
                    }
                    Some(Some(_)) => {
                        unreachable!(
                            "links vector should have an entry for line {}, but doesn't",
                            i
                        );
                    }
                    Some(None) => {
                        ui.label(egui::RichText::new(line).monospace());
                    }
                    None if line.starts_with("=> ") => {
                        assert!(links.len() == i);

                        // let (url_port, _) = split_directory(line);
                        match resolve_line(&line, &addr_str) {
                            Some(url) => {
                                if let Some(a) = ui_hyperlink(ui, &line, &url) {
                                    action = Some(a);
                                    return;
                                }

                                links.push(Some(url.clone()));
                            }
                            None => {
                                ui.label(egui::RichText::new(line).monospace());
                                links.push(None);
                            }
                        }
                    }
                    None => {
                        ui.label(egui::RichText::new(line).monospace());
                        links.push(None);
                    }
                }
            }

            toasts.show(ctx);
        });

    action
}

fn ui_hyperlink(ui: &mut Ui, line: &str, url: &Url) -> Option<TextDocAction> {
    match url.scheme() {
        "http" | "https" => ui_http(ui, &line),
        "nex" => {
            if ui_nex(ui, &line) {
                return Some(TextDocAction::StartNewUrl(url.to_string()));
            }
        }
        _ => {
            if ui_generic_link(ui, &line) {
                return Some(TextDocAction::StartNewUrl(url.to_string()));
            }
        }
    }

    None
}

fn ui_http(ui: &mut Ui, line: &str) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let (_, url_port, rest) = split_directory(line);

        ui.label(egui::RichText::new("=> ").monospace());
        ui.hyperlink(url_port);

        ui.label(egui::RichText::new(rest).monospace());
    });
}

fn ui_nex(ui: &mut Ui, line: &str) -> bool {
    ui_generic_link(ui, line)
}

fn ui_generic_link(ui: &mut Ui, line: &str) -> bool {
    let mut start_new = false;

    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let (_, url_port, rest) = split_directory(line);

        ui.label(egui::RichText::new("=> ").monospace());
        if ui.link(url_port).clicked() {
            start_new = true;
        }

        ui.label(egui::RichText::new(rest).monospace());
    });

    start_new
}

fn split_directory(line: &str) -> (&str, &str, &str) {
    let url_end = line[3..].find(' ').unwrap_or(line.len() - 3) + 3;
    (&line[..3], &line[3..url_end], &line[url_end..])
}

// FIXME: nex-specific right now... needs refactor.
fn resolve_line(line: &str, addr_str: &str) -> Option<Url> {
    let (_, url_port, _) = split_directory(line);

    match Url::parse(url_port) {
        Ok(url) => Some(url),
        Err(_) => resolve_relative(addr_str, url_port),
    }
}

// FIXME: nex-specific right now... needs refactor.
fn resolve_relative(addr_str: &str, path: &str) -> Option<Url> {
    let abs_url = match Url::parse(addr_str) {
        Ok(url) => {
            if !url.path().ends_with('/') && !url.path().contains('.') {
                let new_url = url.join(&format!("{}/{}", url.path(), path));
                debug!(target: "nex-ballast-fg", "fixing up nex directory without trailing slash {:?} => {:?}", url, new_url);
                new_url
            } else {
                url.join(path)
            }
        }
        Err(_) => {
            return None;
        }
    };
    debug!(target: "nex-ballast-fg", "url didn't parse... treating as relative {:?}", &abs_url);
    match abs_url {
        Ok(url) => Some(url),
        Err(_) => None,
    }
}
