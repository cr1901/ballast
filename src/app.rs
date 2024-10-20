use std::str::Lines;

use eframe;
use eframe::egui::{self, Context, TextEdit, Ui, Widget};
use eyre::Result;
use log::debug;
use url::Url;

use super::retrieval::{self, CmdSend, RespRecv};
use super::url::NexUrl;

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
    url: Option<Url>,
    url_string: String,
    raw: String,
    links: Vec<Option<Url>>,
    nex_url: Option<NexUrl>,
    resp: Option<RespRecv>,
}

impl Ballast {
    pub fn new() -> Self {
        let cmd = retrieval::spawn();
        // let nex_string = RefCell::new(String::new());
        // let mut link_present: Vec<Option<Url>> = Vec::new();

        Self {
            state: ControlFlow::Waiting,
            cmd,
            url: None,
            url_string: String::new(),
            nex_url: None,
            raw: String::new(),
            links: Vec::new(),
            resp: None,
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
        let (send, recv) = oneshot::channel();

        // debug!(target: "nex-ballast-fg", "start_new_url {:?}", url.to_string());

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
}

impl eframe::App for Ballast {
    fn update(&mut self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame) {
        match ui_address_bar(ctx, &mut self.url_string) {
            Some(AddressBarAction::StartNewUrl) => {
                if let Ok(nex_url) = NexUrl::try_from(&*self.url_string) {
                    self.nex_url = Some(nex_url);
                    self.start_new_url();
                } else {
                    debug!(target: "nex-ballast-fg", "url didn't parse as NEX... {:?}", &self.url_string);
                }
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
                        }
                        _ => {}
                    }
                }
            }
            ControlFlow::TextDoc => {
                match ui_textdoc(ui, self.raw.lines(), &mut self.links, &self.url_string) {
                    Some(TextDocAction::StartNewUrl(url)) => {
                        if let Ok(nex_url) = NexUrl::try_from(url.as_str()) {
                            debug!(target: "nex-ballast-fg", "url parsed as NEX... {}, {:?}", url.as_str(), nex_url);
                            self.url_string = url.to_string();
                            self.nex_url = Some(nex_url);
                            self.start_new_url();
                        } else {
                            debug!(target: "nex-ballast-fg", "url didn't parse as NEX... {:?}", url.as_str());
                        }
                    }
                    None => {}
                }
            }
        });
    }
}

enum AddressBarAction {
    StartNewUrl,
}

fn ui_address_bar(ctx: &Context, addr_str: &mut String) -> Option<AddressBarAction> {
    let mut action = None;

    egui::TopBottomPanel::top("address_bar")
        .resizable(false)
        .show(ctx, |ui| {
            let response = TextEdit::singleline(addr_str)
                .desired_width(f32::INFINITY)
                .ui(ui);
            if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                action = Some(AddressBarAction::StartNewUrl);
            }
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
    lines: Lines,
    links: &mut Vec<Option<Url>>,
    addr_str: &String,
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
                                    Ok(url) => url.join(&line[3..url_end]),
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
        });

    action
}
