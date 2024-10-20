use eframe;
use eframe::egui::{self, TextEdit, Widget};
use log::debug;
use url::Url;

use super::retrieval::{self, CmdSend, RespRecv};

enum ControlFlow {
    Waiting,
    Idle,
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
    // doc: Option<Document>
    url: Option<Url>, 
    url_string: String,
    raw: String,
    links: Vec<Option<Url>>,
    resp: Option<RespRecv>
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
            raw: String::new(),
            links: Vec::new(),
            resp: None
        }
    }

    pub fn do_home_page(&mut self) {
        self.url_string = String::from("nex://nex.nightfall.city/");
        self.start_new_url();
    }

    fn start_new_url(&mut self) {
        let (send, recv) = oneshot::channel();
    
        // debug!(target: "nex-ballast-fg", "start_new_url {:?}", url.to_string());
    
        // let url_string = url.to_string();
        // self.url_string = url_string.clone();
        self.cmd.send((self.url_string.clone(), send)).unwrap();
    
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
        egui::TopBottomPanel::top("address_bar")
                .resizable(false)
                .show(ctx, |ui| {
                    let response = TextEdit::singleline(&mut self.url_string).desired_width(f32::INFINITY).ui(ui);
                    if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        self.start_new_url();
                    }
                });

            egui::CentralPanel::default().show(ctx, |ui| match &mut self.state {
                ControlFlow::Waiting => {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.centered_and_justified(|ui| {
                                let height = ui.max_rect().height();
                                ui.add(egui::widgets::Spinner::new().size(height));
                            })
                        });
                    if let Some(ref mut recv) = self.resp {
                        match recv.try_recv() {
                            Ok(Ok(recv)) => {
                                self.raw = recv;
                                self.links.clear();
                                self.state = ControlFlow::Idle;
                            }
                            _ => {}
                        }
                    }
                }
                ControlFlow::Idle => {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            for (i, line) in self.raw.lines().enumerate() {
                                match self.links.get(i) {
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
                                            self.url_string = url.to_string();
                                            self.start_new_url();
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
                                        assert!(self.links.len() == i);
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
                                                    self.url_string = url.to_string();
                                                    self.start_new_url();
                                                    return;
                                                }

                                                self.links.push(Some(url.clone()));
                                            }
                                            Err(_) => {
                                                let abs_url = match Url::parse(&self.url_string) {
                                                    Ok(url) => url.join(&line[3..url_end]),
                                                    Err(_) => {
                                                        self.links.push(None);
                                                        ui.label(egui::RichText::new(line).monospace());
                                                        continue;
                                                    }
                                                };
                                                debug!(target: "nex-ballast-fg", "url didn't parse... treating as relative {:?}", &abs_url);
                                                match abs_url
                                                {
                                                    Ok(url) => {
                                                        self.links.push(Some(url.clone()));
                                                    }
                                                    Err(_) => {
                                                        self.links.push(None);
                                                        ui.label(egui::RichText::new(line).monospace());
                                                        continue;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    None => {
                                        self.links.push(None);
                                        ui.label(egui::RichText::new(format!("{}\n", line)).monospace());
                                    }
                                }
                            }
                        });
                }
            });
        }
    }
