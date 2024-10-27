use eframe::egui::{Context, ImageSource, Ui, WidgetText};
use eframe::{self, egui};
use egui_toast::ToastKind;
use eyre::Report;

use super::url::UrlType as OurUrl;

mod nex;

/* pub enum Document {
    /// Replacement for Option::None.
    Null,
    Error(String),
    NexDirectory { raw: String, links: Vec<Option<Url>> },
    Jpeg { raw: Bytes },
} */

impl From<(Result<Vec<u8>, Report>, &Option<OurUrl>)> for Box<dyn Document> {
    fn from((recv, url): (Result<Vec<u8>, Report>, &Option<OurUrl>)) -> Self {
        match (recv, url) {
            (Ok(bytes), Some(url)) => match url {
                OurUrl::Nex(url) => {
                    if url.selector().ends_with(".jpg") || url.selector().ends_with(".jpeg") {
                        Box::new(Jpeg::new(bytes))
                    } else {
                        Box::new(nex::Directory::new(bytes, url))
                    }
                }
                _ => unreachable!(),
            },
            (Err(r), Some(url)) => {
                let err_string = format!("Error resolving {}:\n{}", url.to_string(), r.to_string());
                Box::new(Error::new(err_string))
            }
            (_, None) => unreachable!(),
        }
    }
}

pub enum AppAction {
    None,
    Toast { kind: ToastKind, text: WidgetText },
    StartNewUrl(String),
}

pub trait Document {
    fn render(&mut self, ui: &mut Ui, ctx: &Context) -> AppAction;
    fn present(&mut self, ui: &mut Ui, ctx: &Context) -> AppAction;
}

pub struct Jpeg {
    raw: Vec<u8>,
}

impl Jpeg {
    pub fn new<B>(bytes: B) -> Self
    where
        B: ToOwned<Owned = Vec<u8>>,
    {
        Self {
            raw: bytes.to_owned(),
        }
    }
}

impl Document for Jpeg {
    fn render(&mut self, ui: &mut Ui, ctx: &Context) -> AppAction {
        ctx.forget_image("bytes://ballast-image");
        AppAction::None
    }

    fn present(&mut self, ui: &mut Ui, ctx: &Context) -> AppAction {
        ui.image(ImageSource::Bytes {
            uri: "bytes://ballast-image".into(),
            bytes: self.raw.clone().into(),
        });

        AppAction::None
    }
}

pub struct Error {
    raw: String,
}

impl Error {
    pub fn new(msg: String) -> Self {
        Self { raw: msg }
    }
}

impl Document for Error {
    fn render(&mut self, ui: &mut Ui, ctx: &Context) -> AppAction {
        AppAction::None
    }

    fn present(&mut self, ui: &mut Ui, ctx: &Context) -> AppAction {
        for line in self.raw.lines() {
            ui.label(egui::RichText::new(line).monospace());
        }

        AppAction::None
    }
}

pub struct Null(());

impl Null {
    pub fn new() -> Self {
        Self(())
    }
}

impl Document for Null {
    fn render(&mut self, ui: &mut Ui, ctx: &Context) -> AppAction {
        AppAction::None
    }

    fn present(&mut self, ui: &mut Ui, ctx: &Context) -> AppAction {
        AppAction::None
    }
}
