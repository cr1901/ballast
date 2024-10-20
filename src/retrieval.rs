use log::debug;
use oneshot::{self};
use url::Url;

use std::io::Read;
use std::io::Write;
use std::net::TcpStream;
use std::sync::mpsc;
use std::thread;

use eyre::{eyre, Result};


pub type CmdSend = mpsc::Sender<(String, oneshot::Sender::<Result<String>>)>;
pub type RespRecv = oneshot::Receiver::<Result<String>>;

pub fn spawn() -> CmdSend {
    let (cmd_send, cmd_recv) = mpsc::channel();

    thread::spawn(|| bg_thread(cmd_recv));

    cmd_send
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
