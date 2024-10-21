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

// impl AsRef<str> for NexUrl {
//     fn as_ref(&self) -> &str {
//         &self.0
//     }
// }
