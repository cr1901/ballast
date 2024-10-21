use async_net::AsyncToSocketAddrs;
use futures_lite::AsyncWriteExt;
use log::debug;
use log::warn;
use oneshot::{self};

use std::error;
use std::fmt;
use std::io;
use std::io::ErrorKind;
use std::sync::mpsc;
use std::thread;

use async_channel;
use async_net::TcpStream;
use eyre::Result;
use futures_lite::{future::block_on, AsyncReadExt, FutureExt};

use super::url::NexUrl;

pub type CmdSend = mpsc::Sender<(NexUrl, oneshot::Sender<Result<String>>)>;
pub type RespRecv = oneshot::Receiver<Result<String>>;
pub type CancelSend = async_channel::Sender<()>;

type CmdRecv = mpsc::Receiver<(NexUrl, RespSend)>;
type CancelRecv = async_channel::Receiver<()>;
type RespSend = oneshot::Sender<Result<String>>;

pub fn spawn() -> (CmdSend, CancelSend) {
    let (cmd_send, cmd_recv) = mpsc::channel();
    let (cancel_send, cancel_recv) = async_channel::bounded(1);

    thread::spawn(|| bg_thread(cmd_recv, cancel_recv));

    (cmd_send, cancel_send)
}

/* FIXME: Damnit, I did not want to bring in an async executor, but in Rust,
   you're expected to instead of doing select loops. I hope one day I can
   rewrite this so there's a chance it'll run on [Windows 95](https://seri.tools/blog/announcing-rust9x/)
   for the real retro goodness.

   At the very least, mspc channels rely on futexes/wait-for-address-to-change
   functionality: https://github.com/rust-lang/rust/blob/93742bd782e7142899d782f663448ab51a3eec9b/library/std/src/sys/sync/thread_parking/futex.rs#L55
   Does Windows 95 have this?
*/
fn bg_thread(cmd_recv: CmdRecv, cancel_recv: CancelRecv) {
    fn tcp_connect<A>(url: A, cancel_recv: &CancelRecv) -> Result<TcpStream, io::Error>
    where
        A: AsyncToSocketAddrs,
    {
        block_on(TcpStream::connect(url).or(async {
            let _ = cancel_recv.recv().await;
            Err(io::Error::other(ConnCancelled {}))
        }))
    }

    fn tcp_write(conn: &mut TcpStream, buf: &str) -> Result<usize, io::Error> {
        block_on(conn.write(format!("{}\n", buf).as_bytes()))
    }

    fn tcp_read(
        mut conn: TcpStream,
        buf: &mut Vec<u8>,
        cancel_recv: &CancelRecv,
    ) -> Result<usize, io::Error> {
        block_on(conn.read_to_end(buf).or(async {
            let _ = cancel_recv.recv().await;
            Err(io::Error::other(ConnCancelled {}))
        }))
    }

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

        match tcp_connect((url.host(), url.port()), &cancel_recv) {
            Ok(mut conn) => {
                tcp_write(&mut conn, url.selector());

                let mut bytes = Vec::new();
                if let Err(e) = tcp_read(conn, &mut bytes, &cancel_recv) {
                    if e.kind() == ErrorKind::Other {
                        let e = match e.downcast::<ConnCancelled>() {
                            Ok(_cc) => {
                                debug!(target: "nex-ballast-bg", "conection cancelled");
                                continue;
                            }
                            Err(e) => e,
                        };

                        debug!(target: "nex-ballast-bg", "unexpected ErrorKind::Other: {}", e);
                        continue;
                    } else {
                        debug!(target: "nex-ballast-bg", "unexpected error: {}", e);
                        continue;
                    }
                }

                let nex_string = String::from_utf8_lossy(&mut bytes).into_owned();
                // debug!(target: "nex-ballast-bg", "{}", nex_string);
                let _ = send.send(Ok(nex_string));
            }
            Err(e) => {
                debug!(target: "nex-ballast-bg", "connect error {}", e);
                let _ = send.send(Err(e.into()));
            }
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
