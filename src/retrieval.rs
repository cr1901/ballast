use log::debug;
use oneshot::{self};

use std::io::Read;
use std::io::Write;
use std::net::TcpStream;
use std::sync::mpsc;
use std::thread;

use eyre::Result;

use super::url::NexUrl;

pub type CmdSend = mpsc::Sender<(NexUrl, oneshot::Sender<Result<String>>)>;
pub type RespRecv = oneshot::Receiver<Result<String>>;

pub fn spawn() -> CmdSend {
    let (cmd_send, cmd_recv) = mpsc::channel();

    thread::spawn(|| bg_thread(cmd_recv));

    cmd_send
}

fn bg_thread(recv: mpsc::Receiver<(NexUrl, oneshot::Sender<Result<String>>)>) {
    loop {
        let (url, send) = if let Ok(cmd) = recv.recv() {
            cmd
        } else {
            debug!(target: "nex-ballast-bg", "Sender disconnected");
            break;
        };

        debug!(target: "nex-ballast-bg", "Connecting to {:?}", url);

        match TcpStream::connect(&url) {
            Ok(mut conn) => {
                let mut bytes = Vec::new();
                conn.write(format!("{}\n", url.selector()).as_bytes());
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
