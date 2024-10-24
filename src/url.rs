use std::cmp::min;
use std::iter;
use std::net::ToSocketAddrs;

use url::Url;

#[derive(Debug)]
pub struct TryFromStringError;

#[derive(Clone, Debug)]
pub struct NexUrl {
    host: String,
    port: u16,
    selector: String,
}

impl NexUrl {
    pub fn host(&self) -> &str {
        &*self.host
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn selector(&self) -> &str {
        &*self.selector
    }
}

impl TryFrom<&str> for NexUrl {
    type Error = TryFromStringError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match Url::parse(&value) {
            Ok(u) if u.has_host() => {
                if u.scheme() != "nex" {
                    return Err(TryFromStringError);
                }

                let host = u.host().unwrap().to_owned();
                Ok(Self {
                    host: host.to_string(),
                    port: u.port().unwrap_or(1900),
                    selector: u.path().to_owned(),
                })
            }
            Ok(u) if !u.has_host() => {
                Err(TryFromStringError)
                // debug!(target: "nex-ballast-bg", "not a domain: {}", u);
                // let _ = send.send(Err(eyre!("not a domain: {}", u)));
                // continue;
            }
            Ok(_) => {
                unreachable!()
            }
            Err(_) => {
                Err(TryFromStringError)
                // debug!(target: "nex-ballast-bg", "{}", e);
                // let _ = send.send(Err(e.into()));
                // continue;
            }
        }
    }
}

impl ToSocketAddrs for NexUrl {
    type Iter = <(String, u16) as ToSocketAddrs>::Iter;

    fn to_socket_addrs(&self) -> std::io::Result<Self::Iter> {
        (&*self.host, self.port).to_socket_addrs()
    }
}

impl ToString for NexUrl {
    fn to_string(&self) -> String {
        if self.port() == 1900 {
            format!("nex://{}{}", self.host(), self.selector())
        } else {
            format!("nex://{}:{}{}", self.host(), self.port(), self.selector())
        }
    }
}

// impl AsRef<str> for NexUrl {
//     fn as_ref(&self) -> &str {
//         &self.0
//     }
// }

pub struct UrlStack {
    stack: Vec<NexUrl>,
    ptr: Option<usize>,
}

#[derive(Clone, Copy, Debug)]
pub struct UrlStackPtr(usize);

impl Into<usize> for UrlStackPtr {
    fn into(self) -> usize {
        self.0
    }
}

// TODO: Implement a stack depth limit of some sort, or go back to VecDeque?
impl UrlStack {
    pub fn new() -> Self {
        Self {
            stack: Vec::new(),
            ptr: None,
        }
    }

    pub fn push(&mut self, url: NexUrl) {
        match self.ptr {
            /* A push should remove all stack entries above it. */
            Some(ptr) => {
                // debug!(target: "nex-ballast-fg", "stack {}",)
                self.truncate(url, UrlStackPtr(ptr));
            }
            /* If we just started, nothing to truncate. */
            None => {
                self.ptr = Some(0);
                self.stack.push(url);
                return;
            }
        };
    }

    pub fn truncate(&mut self, url: NexUrl, ptr: UrlStackPtr) {
        assert!(self.ptr.is_some());
        assert!(ptr.0 < self.stack.len());

        self.stack.truncate(ptr.0 + 1);
        self.stack.push(url);
        self.ptr = Some(ptr.0 + 1);
    }

    pub fn ptr(&self) -> Option<usize> {
        self.ptr
    }

    pub fn set_ptr(&mut self, ptr: UrlStackPtr) {
        assert!(self.ptr.is_some());
        assert!(ptr.0 < self.stack.len());
        self.ptr.replace(ptr.0);
    }

    pub fn iter(&self) -> Box<dyn Iterator<Item = (UrlStackPtr, &NexUrl)> + '_> {
        match self.ptr {
            Some(ptr) if self.stack.len() > 1 => {
                let end = min(ptr + 5, self.stack.len());
                let begin = ptr.saturating_sub(5);

                return Box::new(
                    (begin..end)
                        .rev()
                        .map(UrlStackPtr)
                        .zip(self.stack[begin..end].iter().rev()),
                );
            }
            Some(_) | None => {
                return Box::new(iter::empty());
            }
        }
    }
}
