use eframe;
use eframe::egui::{self, Context, Ui};
use egui_toast::ToastKind;
use log::debug;
use url::Url;

use super::{AppAction, Document};

pub(super) struct Directory {
    raw: String,
    addr: String,
    links: Vec<Option<Url>>,
}

impl Directory {
    pub fn new<B, U>(bytes: B, url: &U) -> Self where B: AsRef<[u8]>, U: ToString {
        Self {
            raw: String::from_utf8_lossy(bytes.as_ref()).into_owned(),
            addr: url.to_string(),
            links: Vec::new()
        }
    }
}

impl Document for Directory {
    fn render(&mut self, ui: &mut Ui, ctx: &Context) -> AppAction {
        if self.raw.contains('\u{fffd}') {
            return AppAction::Toast {
                text: "UTF-8, replacement character detected.\nThis is probably a (unsupported) binary file.".into(),
                kind: ToastKind::Warning
            }
        }

        AppAction::None
    }

    fn present(&mut self, ui: &mut Ui, ctx: &Context) -> AppAction {
        let mut action = AppAction::None;

        egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for (i, line) in self.raw.lines().enumerate() {
                match self.links.get(i) {
                    Some(Some(url)) if line.starts_with("=> ") => {
                        if let Some(u) = ui_hyperlink(ui, &line, &url) {
                            action = AppAction::StartNewUrl(u);
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
                        assert!(self.links.len() == i);

                        // let (url_port, _) = split_directory(line);
                        match resolve_line(&line, &self.addr) {
                            Some(url) => {
                                if let Some(u) = ui_hyperlink(ui, &line, &url) {
                                    action = AppAction::StartNewUrl(u);
                                    return;
                                }

                                self.links.push(Some(url.clone()));
                            }
                            None => {
                                ui.label(egui::RichText::new(line).monospace());
                                self.links.push(None);
                            }
                        }
                    }
                    None => {
                        ui.label(egui::RichText::new(line).monospace());
                        self.links.push(None);
                    }
                }
            }
        });

        action
    }
}

fn ui_hyperlink(ui: &mut Ui, line: &str, url: &Url) -> Option<String> {
    match url.scheme() {
        "http" | "https" => ui_http(ui, &line),
        "nex" => {
            if ui_nex(ui, &line) {
                return Some(url.to_string());
            }
        }
        _ => {
            if ui_generic_link(ui, &line) {
                return Some(url.to_string());
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
