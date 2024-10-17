#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use eframe::egui::Response;
use env_logger;
use log::debug;
use oneshot::{self};
use url::{ParseError, Url};

use std::cell::RefCell;
use std::io::Read;
use std::io::Write;
use std::net::TcpStream;
use std::sync::mpsc;
use std::thread;

use eyre::{eyre, Result};

use eframe::egui;

enum ControlFlow {
    Waiting,
    Idle,
}

fn bg_thread(recv: mpsc::Receiver<(String, oneshot::Sender<Result<String>>)>) {
    loop {
        let (url, send) = if let Ok(cmd) = recv.recv() {
            cmd
        } else {
            debug!(target: "nex-ballast-bg", "Sender disconnected");
            break;
        };

        let (domain, port, link) = match Url::parse(&url) {
            Ok(u) if u.has_host() => {
                let host = u.host().unwrap().to_owned();
                (host, u.port().unwrap_or(1900), u.path().to_owned())
            }
            Ok(u) if !u.has_host() => {
                debug!(target: "nex-ballast-bg", "not a domain: {}", u);
                let _ = send.send(Err(eyre!("not a domain: {}", u)));
                continue;
            }
            Ok(_) => {
                unreachable!()
            }
            Err(e) => {
                debug!(target: "nex-ballast-bg", "{}", e);
                let _ = send.send(Err(e.into()));
                continue;
            }
        };

        debug!(target: "nex-ballast-bg", "Connecting to {}, {}, {}", domain, port, link);

        match TcpStream::connect((domain.to_string(), port)) {
            Ok(mut conn) => {
                let mut bytes = Vec::new();
                conn.write(format!("{}\n", link).as_bytes());
                conn.read_to_end(&mut bytes);

                let nex_string = String::from_utf8_lossy(&mut bytes).into_owned();
                // debug!(target: "nex-ballast-bg", "{}", nex_string);
                let _ = send.send(Ok(nex_string));
            }
            Err(e) => {
                debug!(target: "nex-ballast-bg", "{}", e);
                let _ = send.send(Err(e.into()));
            }
        }
    }
}

fn main() -> Result<(), eframe::Error> {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([640.0, 480.0]),
        ..Default::default()
    };

    // let mut error_text = VecDeque::new();
    let (cmd_send, cmd_recv) = mpsc::channel();
    let (send, mut recv_) = oneshot::channel();
    let mut state = ControlFlow::Waiting;

    thread::spawn(|| bg_thread(cmd_recv));

    let mut url_string = String::from("nex://nex.nightfall.city/");
    let _ = cmd_send.send((url_string.clone(), send));
    let nex_string = RefCell::new(String::new());

    eframe::run_simple_native("swmon", options, move |ctx, _frame| {
        // Render top to bottom
        egui::TopBottomPanel::top("address_bar")
            .resizable(false)
            .show(ctx, |ui| {
                let response = ui.text_edit_singleline(&mut url_string);
                if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    let (send, recv_tmp_) = oneshot::channel();
                    recv_ = recv_tmp_;

                    let _ = cmd_send.send((url_string.clone(), send));
                    state = ControlFlow::Waiting;
                }
            });

        egui::CentralPanel::default().show(ctx, |ui| match &mut state {
            ControlFlow::Waiting => {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.add(egui::widgets::Spinner::new());
                    });
                match recv_.try_recv() {
                    Ok(Ok(recv)) => {
                        *nex_string.borrow_mut() = recv;
                        state = ControlFlow::Idle;
                    }
                    _ => {}
                }
            }
            ControlFlow::Idle => {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new(&*nex_string.borrow()).monospace());
                    });
            }
        });
    })
}
