use std::{collections::VecDeque, net::ToSocketAddrs};

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

// impl AsRef<str> for NexUrl {
//     fn as_ref(&self) -> &str {
//         &self.0
//     }
// }

pub struct UrlStack {
    stack: VecDeque<NexUrl>,
    ptr: Option<usize>,
}

impl UrlStack {
    pub fn new() -> Self {
        Self {
            stack: VecDeque::new(),
            ptr: None,
        }
    }

    fn ptr_at_beginning(&self) -> bool {
        self.ptr.map_or(true, |p| p == 0)
    }

    fn ptr_at_end(&self) -> bool {
        self.ptr.map_or(false, |p| p == self.stack.len() - 1)
    }

    pub fn push(&mut self, url: NexUrl) {
        if self.ptr_at_end() {
            if self.stack.len() >= 10 {
                self.stack.pop_front();
                self.stack.push_back(url);
            } else {
                self.stack.push_back(url);
                *self.ptr.as_mut().unwrap() += 1;
            }
        } else if let Some(ref mut ptr) = self.ptr {
            self.stack.truncate(*ptr + 1);
            self.stack.push_back(url);
            *ptr += 1;
        } else {
            assert!(self.stack.len() == 0);
            self.stack.push_back(url);
            self.ptr = Some(0);
        }
    }

    pub fn ptr(&self) -> Option<usize> {
        self.ptr
    }

    pub fn set_ptr(&mut self, ptr: usize) {
        assert!(self.ptr.is_some());
        assert!(ptr < self.stack.len());
        self.ptr.replace(ptr);
    }

    pub fn truncate(&mut self, url: NexUrl, curr: usize) {
        if self.ptr_at_end() {
            if self.stack.len() >= 10 {
                self.stack.pop_front();
                self.stack.push_back(url);
            } else {
                self.stack.push_back(url);
                *self.ptr.as_mut().unwrap() += 1;
            }
        }
    }

    // pub fn curr(&self) -> Option<NexUrl> {}

    pub fn iter(&self) -> impl Iterator<Item = (usize, &NexUrl)> {
        let (first, rest) = self.stack.as_slices();
        rest.iter()
            .rev()
            .chain(first.iter().rev())
            .enumerate()
            .map(|(i, u)| (self.stack.len() - 1 - i, u))
        // .map(|(i, n)| (self.ptr.map_or(false, |p| i == p), n))

        // for u in first.iter().chain(rest) {
        //     ui.button(format!("{}, {}", u.host(), u.selector()));
        // }

        // if let Some(url_stack_ptr) = ballast.url_stack_ptr {
        //     if url_stack_ptr > 0 {
        //         let (first, rest) = ballast.url_stack.as_slices();

        //         for u in first.iter().chain(rest).take(url_stack_ptr - 1) {
        //             ui.button(format!("{}, {}", u.host(), u.selector()));
        //         }
        //     }
        // }
    }
}
