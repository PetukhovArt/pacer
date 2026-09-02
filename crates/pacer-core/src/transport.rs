//! The DAEMON SOCKET's transport: the stream type the IPC CODEC runs over,
//! how the daemon binds it and how a client reaches it.
//!
//! Unix keeps an `AF_UNIX` socket in the RUNTIME DIR, where mode 0700 on the
//! dir is the whole authorization story — a connect that succeeds is already
//! a connect from this user. Windows has no such socket, so the DAEMON
//! SOCKET is a loopback TCP listener plus a bearer token, the same model the
//! HOOK RECEIVER already uses on *every* platform (`hooks/mod.rs` binds
//! `127.0.0.1:0` and hands out a 32-byte token). One local-authorization
//! mechanism, not two.
//!
//! The port and the token live in the ENDPOINT FILE beside the PIDFILE in the
//! RUNTIME DIR. Whoever can read that file could read the RUNTIME DIR anyway,
//! so the boundary is unchanged; on Windows the user's profile is already
//! closed to other unprivileged users by its inherited ACL.
//!
//! Callers see one API on both platforms: [`bind`], `Listener::accept`,
//! `Authorizer::authorize` (server side) and [`connect`] (client side).

#[cfg(unix)]
pub use self::unix_transport::*;
#[cfg(windows)]
pub use self::windows_transport::*;

/// Where the transport is reachable, for log lines and error messages.
pub fn endpoint_description() -> String {
    endpoint_path().display().to_string()
}

/// Remove whatever a dead daemon left behind (the socket file, or the
/// ENDPOINT FILE) so a fresh bind is not refused by a stale name.
pub fn unlink_stale() {
    let _ = std::fs::remove_file(endpoint_path());
}

// ---------------------------------------------------------------------------
// Unix: AF_UNIX socket; the RUNTIME DIR's 0700 mode is the authorization.
// ---------------------------------------------------------------------------
#[cfg(unix)]
mod unix_transport {
    use crate::paths;
    use std::io;

    pub type Stream = tokio::net::UnixStream;

    /// The file that names the transport: the socket itself.
    pub fn endpoint_path() -> std::path::PathBuf {
        paths::socket_path()
    }

    pub struct Listener {
        inner: tokio::net::UnixListener,
    }

    impl Listener {
        pub async fn accept(&self) -> io::Result<Stream> {
            self.inner.accept().await.map(|(stream, _)| stream)
        }

        /// Handle for the gate a connecting client has to clear.
        pub fn authorizer(&self) -> Authorizer {
            Authorizer
        }
    }

    /// No-op on Unix: reaching the socket at all required entering the
    /// 0700 RUNTIME DIR.
    #[derive(Clone)]
    pub struct Authorizer;

    impl Authorizer {
        pub async fn authorize(&self, _stream: &mut Stream) -> io::Result<()> {
            Ok(())
        }
    }

    pub async fn bind() -> io::Result<Listener> {
        super::unlink_stale();
        Ok(Listener {
            inner: tokio::net::UnixListener::bind(endpoint_path())?,
        })
    }

    pub async fn connect() -> io::Result<Stream> {
        tokio::net::UnixStream::connect(endpoint_path()).await
    }
}

// ---------------------------------------------------------------------------
// Windows: loopback TCP + bearer token, presented ahead of `Hello`.
// ---------------------------------------------------------------------------
#[cfg(windows)]
mod windows_transport {
    use crate::codec::{read_frame, write_frame};
    use crate::paths;
    use std::io;
    use std::sync::Arc;
    use subtle::ConstantTimeEq;

    pub type Stream = tokio::net::TcpStream;

    /// A client that presents no token within this long is dropped, so one
    /// silent connection cannot pin a served task forever.
    const AUTH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

    /// The ENDPOINT FILE: `<port>\n<token>\n` beside the PIDFILE.
    pub fn endpoint_path() -> std::path::PathBuf {
        paths::endpoint_path()
    }

    pub struct Listener {
        inner: tokio::net::TcpListener,
        token: Arc<String>,
    }

    impl Listener {
        pub async fn accept(&self) -> io::Result<Stream> {
            let (stream, _) = self.inner.accept().await?;
            // Frames are small and request/reply-shaped; Nagle would add
            // 40ms to every keystroke forwarded to a PTY SESSION.
            let _ = stream.set_nodelay(true);
            Ok(stream)
        }

        pub fn authorizer(&self) -> Authorizer {
            Authorizer {
                token: self.token.clone(),
            }
        }
    }

    /// Reads the token frame a client sends ahead of `Hello` and compares it
    /// in constant time, exactly as the HOOK RECEIVER compares its bearer.
    #[derive(Clone)]
    pub struct Authorizer {
        token: Arc<String>,
    }

    impl Authorizer {
        pub async fn authorize(&self, stream: &mut Stream) -> io::Result<()> {
            let presented = tokio::time::timeout(AUTH_TIMEOUT, read_frame::<String, _>(stream))
                .await
                .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "no token frame"))??
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "closed before the token frame",
                    )
                })?;
            let ok: bool = presented.as_bytes().ct_eq(self.token.as_bytes()).into();
            if ok {
                Ok(())
            } else {
                Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "bad daemon token",
                ))
            }
        }
    }

    pub async fn bind() -> io::Result<Listener> {
        let inner = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await?;
        let port = inner.local_addr()?.port();
        let token = generate_token();
        write_endpoint_file(port, &token)?;
        Ok(Listener {
            inner,
            token: Arc::new(token),
        })
    }

    pub async fn connect() -> io::Result<Stream> {
        let (port, token) = read_endpoint_file()?;
        let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port)).await?;
        let _ = stream.set_nodelay(true);
        write_frame(&mut stream, &token).await?;
        Ok(stream)
    }

    /// The ENDPOINT FILE is rewritten, not appended: a shorter token must not
    /// leave the tail of a longer one behind for a client to read.
    fn write_endpoint_file(port: u16, token: &str) -> io::Result<()> {
        std::fs::write(endpoint_path(), format!("{port}\n{token}\n"))
    }

    fn read_endpoint_file() -> io::Result<(u16, String)> {
        let path = endpoint_path();
        let text = std::fs::read_to_string(&path)?;
        parse_endpoint(&text).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unreadable endpoint file {}", path.display()),
            )
        })
    }

    fn parse_endpoint(text: &str) -> Option<(u16, String)> {
        let mut lines = text.lines();
        let port: u16 = lines.next()?.trim().parse().ok()?;
        let token = lines.next()?.trim().to_string();
        (port != 0 && !token.is_empty()).then_some((port, token))
    }

    /// 32 random bytes, hex — the HOOK RECEIVER's `generate_token` verbatim.
    fn generate_token() -> String {
        use rand::Rng;
        let mut bytes = [0u8; 32];
        rand::rng().fill_bytes(&mut bytes);
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// The token gate is the whole authorization boundary on Windows, so
        /// both verdicts are tested: the right token is served, a wrong one
        /// is refused before any `Hello` is read.
        #[tokio::test]
        async fn the_token_gate_admits_the_right_token_and_refuses_a_wrong_one() {
            let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
                .await
                .unwrap();
            let port = listener.local_addr().unwrap().port();
            let auth = Authorizer {
                token: Arc::new("s3cret".to_string()),
            };

            for (presented, expected_ok) in [("s3cret", true), ("wrong", false)] {
                let mut client = tokio::net::TcpStream::connect(("127.0.0.1", port))
                    .await
                    .unwrap();
                let (mut served, _) = listener.accept().await.unwrap();
                write_frame(&mut client, &presented.to_string())
                    .await
                    .unwrap();
                assert_eq!(
                    auth.authorize(&mut served).await.is_ok(),
                    expected_ok,
                    "token {presented:?}"
                );
            }
        }

        #[test]
        fn the_endpoint_file_round_trips_and_junk_is_refused() {
            assert_eq!(
                parse_endpoint("51234\ndeadbeef\n"),
                Some((51234, "deadbeef".to_string()))
            );
            for junk in ["", "51234\n", "0\ntok\n", "51234\n\n", "notaport\ntok\n"] {
                assert_eq!(parse_endpoint(junk), None, "junk: {junk:?}");
            }
        }
    }
}
