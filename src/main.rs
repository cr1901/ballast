#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use env_logger;
use log::debug;
use oneshot::{self};
use std::cell::RefCell;
use std::io::Read;
use std::io::Write;
use std::net::TcpStream;
use std::thread;
use std::sync::mpsc;

use eyre::Result;

use eframe::egui;

enum ControlFlow {
    Waiting,
    Idle,
}

fn bg_thread(recv: mpsc::Receiver<(String, oneshot::Sender<Result<String>>)>) {
    loop {
        debug!(target: "nex-ballast-bg", "Here");
        let (url, send) = if let Ok(cmd) = recv.recv() {
            cmd
        } else {
            debug!(target: "nex-ballast-bg", "Sender disconnected");
            break;
        };

        match TcpStream::connect(url) {
            Ok(mut conn) => {
                let mut bytes = Vec::new();
                conn.write(b"/\n");
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
    let (send, recv_) = oneshot::channel();
    let mut state = ControlFlow::Waiting;

    thread::spawn(|| bg_thread(cmd_recv));

    let _ = cmd_send.send((String::from("nex.nightfall.city:1900"), send));
    let nex_string = RefCell::new(String::new());

    eframe::run_simple_native("swmon", options, move |ctx, _frame| {
        egui::CentralPanel::default().show(ctx, |ui| {
            debug!(target: "nex-ballast-fg", "{:?}", ui.max_rect());
            match &mut state {
                ControlFlow::Waiting => match recv_.try_recv() {
                    Ok(Ok(recv)) => {
                        *nex_string.borrow_mut() = recv;
                        state = ControlFlow::Idle;
                    }
                    _ => {}
                },
                ControlFlow::Idle => {
                    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                        ui.label(egui::RichText::new(&*nex_string.borrow()).monospace());
                    });
                }
            }
        });
    })
}
