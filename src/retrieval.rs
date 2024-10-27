
use log::debug;
use log::warn;
use oneshot;

use std::error;
use std::fmt;
use std::io::ErrorKind;
use std::sync::mpsc;
use std::thread;

use async_channel;
use eyre::Result;


use super::url::{NexUrl, UrlType};

type Raw = Vec<u8>;

pub type CmdSend = mpsc::Sender<(UrlType, oneshot::Sender<Result<Raw>>)>;
pub type RespRecv = oneshot::Receiver<Result<Raw>>;
pub type CancelSend = async_channel::Sender<()>;

type CmdRecv = mpsc::Receiver<(UrlType, RespSend)>;
type CancelRecv = async_channel::Receiver<()>;
type RespSend = oneshot::Sender<Result<Raw>>;

pub fn spawn() -> (CmdSend, CancelSend) {
    let (cmd_send, cmd_recv) = mpsc::channel();
    let (cancel_send, cancel_recv) = async_channel::bounded(1);

    thread::spawn(|| bg_thread(cmd_recv, cancel_recv));

    (cmd_send, cancel_send)
}

fn bg_thread(cmd_recv: CmdRecv, cancel_recv: CancelRecv) {
    loop {
        let (url, send) = if let Ok(cmd) = cmd_recv.recv() {
            cmd
        } else {
            debug!(target: "nex-ballast-bg", "Sender disconnected");
            break;
        };

        debug!(target: "nex-ballast-bg", "Connecting to {:?}", url);

        if cancel_recv.is_full() {
            warn!(target: "nex-ballast-bg", "spurious request to cancel, ignoring");
            let _ = cancel_recv.recv_blocking();
        }

        match url {
            UrlType::Nex(url) => nex::handle(url, send, &cancel_recv),
        }
    }
}

#[derive(Debug)]
struct ConnCancelled {}

impl fmt::Display for ConnCancelled {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "connection was manually cancelled")
    }
}

impl error::Error for ConnCancelled {}

/* FIXME: Damnit, I did not want to bring in an async executor, but in Rust,
   you're expected to instead of doing select loops. I hope one day I can
   rewrite this so there's a chance it'll run on [Windows 95](https://seri.tools/blog/announcing-rust9x/)
   for the real retro goodness.

   At the very least, mspc channels rely on futexes/wait-for-address-to-change
   functionality: https://github.com/rust-lang/rust/blob/93742bd782e7142899d782f663448ab51a3eec9b/library/std/src/sys/sync/thread_parking/futex.rs#L55
   Does Windows 95 have this?
*/
mod nex {
    use super::{NexUrl, RespSend, CancelRecv, ConnCancelled, debug, ErrorKind};

    use async_net::AsyncToSocketAddrs;
    use async_net::TcpStream;
    use futures_lite::AsyncWriteExt;
    use futures_lite::{future::block_on, AsyncReadExt, FutureExt};

    use std::io;

    
    fn connect<A>(url: A, cancel_recv: &CancelRecv) -> Result<TcpStream, io::Error>
    where
        A: AsyncToSocketAddrs,
    {
        block_on(TcpStream::connect(url).or(async {
            let _ = cancel_recv.recv().await;
            Err(io::Error::other(ConnCancelled {}))
        }))
    }

    fn write(conn: &mut TcpStream, buf: &str) -> Result<usize, io::Error> {
        block_on(conn.write(format!("{}\n", buf).as_bytes()))
    }

    fn read(
        mut conn: TcpStream,
        buf: &mut Vec<u8>,
        cancel_recv: &CancelRecv,
    ) -> Result<usize, io::Error> {
        block_on(conn.read_to_end(buf).or(async {
            let _ = cancel_recv.recv().await;
            Err(io::Error::other(ConnCancelled {}))
        }))
    }

    pub(super) fn handle(url: NexUrl, send: RespSend, cancel_recv: &CancelRecv) {
        match connect((url.host(), url.port()), cancel_recv) {
            Ok(mut conn) => {
                if let Err(e) = write(&mut conn, url.selector()) {
                    let _ = send.send(Err(e.into()));
                    return;
                }
    
                let mut bytes = Vec::new();
                if let Err(e) = read(conn, &mut bytes, cancel_recv) {
                    if e.kind() == ErrorKind::Other {
                        let e = match e.downcast::<ConnCancelled>() {
                            Ok(_cc) => {
                                debug!(target: "nex-ballast-bg", "conection cancelled");
                                let _ = send.send(Err(ConnCancelled {}.into()));
                                return;
                            }
                            Err(e) => e,
                        };
    
                        debug!(target: "nex-ballast-bg", "unexpected ErrorKind::Other: {}", e);
                        let _ = send.send(Err(e.into()));
                        return;
                    } else {
                        debug!(target: "nex-ballast-bg", "unexpected error: {}", e);
                        let _ = send.send(Err(e.into()));
                        return;
                    }
                }
                // debug!(target: "nex-ballast-bg", "{}", nex_string);
    
                send.send(Ok(bytes));
            }
            Err(e) => {
                debug!(target: "nex-ballast-bg", "connect error {}", e);
                let _ = send.send(Err(e.into()));
            }
        }
    }
}
