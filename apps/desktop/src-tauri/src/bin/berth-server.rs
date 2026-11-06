use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use berth_desktop_lib::connectors::monitor;
use berth_desktop_lib::connectors::secrets::{self, FileStore};
use berth_desktop_lib::connectors::Connectors;
use berth_desktop_lib::legacy;
use berth_desktop_lib::remote::{Pinned, Remote, WebAsset, WebSource};
use berth_desktop_lib::settings;
use qrcode::render::unicode::Dense1x2;
use qrcode::QrCode;

const USAGE: &str = "berth-server: Berth core without a window, served to browsers and phones

Usage: berth-server [options]

  --bind <ip>          address to listen on (BERTH_BIND, default 127.0.0.1)
  --port <port>        port (BERTH_PORT, default 4477)
  --data-dir <dir>     settings, connectors and secrets (BERTH_HOME, default ~/.berth)
  --web-dir <dir>      built web app (BERTH_WEB_DIR, default <exe dir>/web)
  --public-url <url>   address phones open, for the pairing link (BERTH_PUBLIC_URL)
  --secrets <mode>     auto | file | keyring (BERTH_SECRETS, default auto)
  -h, --help           this help";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Secrets {
    Auto,
    File,
    Keyring,
}

struct Options {
    bind: IpAddr,
    port: u16,
    data: PathBuf,
    web: PathBuf,
    public_url: Option<String>,
    secrets: Secrets,
}

fn env(name: &str) -> Option<String> {
    legacy::env(name)
}

fn default_web() -> PathBuf {
    let beside = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("web")));
    let checkout = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web/dist");
    match beside {
        Some(dir) if dir.join("index.html").is_file() => dir,
        _ if checkout.join("index.html").is_file() => checkout,
        Some(dir) => dir,
        None => PathBuf::from("web"),
    }
}

fn parse_secrets(value: &str) -> Result<Secrets, String> {
    match value {
        "auto" => Ok(Secrets::Auto),
        "file" => Ok(Secrets::File),
        "keyring" => Ok(Secrets::Keyring),
        other => Err(format!("unknown secrets mode: {other}")),
    }
}

impl Options {
    fn parse() -> Result<Option<Self>, String> {
        let mut bind = env("BERTH_BIND");
        let mut port = env("BERTH_PORT");
        let mut data = env("BERTH_HOME").map(PathBuf::from);
        let mut web = env("BERTH_WEB_DIR").map(PathBuf::from);
        let mut public_url = env("BERTH_PUBLIC_URL");
        let mut secrets = env("BERTH_SECRETS");
        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            if arg == "-h" || arg == "--help" {
                return Ok(None);
            }
            let (name, inline) = match arg.split_once('=') {
                Some((name, value)) => (name.to_owned(), Some(value.to_owned())),
                None => (arg.clone(), None),
            };
            let slot = match name.as_str() {
                "--bind" => &mut bind,
                "--port" => &mut port,
                "--public-url" => &mut public_url,
                "--secrets" => &mut secrets,
                "--data-dir" | "--web-dir" => {
                    let value = inline
                        .or_else(|| args.next())
                        .ok_or_else(|| format!("{name} needs a value"))?;
                    let target = if name == "--data-dir" {
                        &mut data
                    } else {
                        &mut web
                    };
                    *target = Some(PathBuf::from(value));
                    continue;
                }
                _ => return Err(format!("unknown option: {arg}")),
            };
            *slot = Some(
                inline
                    .or_else(|| args.next())
                    .ok_or_else(|| format!("{name} needs a value"))?,
            );
        }
        let bind = match bind {
            Some(bind) => bind
                .parse()
                .map_err(|_| format!("bad bind address: {bind}"))?,
            None => IpAddr::V4(Ipv4Addr::LOCALHOST),
        };
        let port = match port {
            Some(port) => port.parse().map_err(|_| format!("bad port: {port}"))?,
            None => 4477,
        };
        let data = match data {
            Some(data) => data,
            None => settings::default_berth_dir()
                .map_err(|_| "HOME is not set: pass --data-dir".to_owned())?,
        };
        Ok(Some(Self {
            bind,
            port,
            data,
            web: web.unwrap_or_else(default_web),
            public_url,
            secrets: secrets
                .as_deref()
                .map_or(Ok(Secrets::Auto), parse_secrets)?,
        }))
    }
}

fn install_secrets(options: &Options) -> String {
    let file = match options.secrets {
        Secrets::File => true,
        Secrets::Keyring => false,
        Secrets::Auto => cfg!(target_os = "linux") || !secrets::keyring_available(),
    };
    if !file {
        return "OS keyring, service \"berth\", entry \"remote-pairing-token\"".to_owned();
    }
    let store = FileStore::new(options.data.join("secrets.json"));
    let note = format!("{} (key \"remote-pairing-token\")", store.path().display());
    secrets::install(Box::new(store));
    note
}

fn mime(path: &Path) -> &'static str {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match extension.as_str() {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "map" => "application/json",
        "webmanifest" => "application/manifest+json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "wasm" => "application/wasm",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn directory(root: PathBuf) -> WebSource {
    WebSource::Assets(Arc::new(move |path: &str| {
        let path = if path == "/" { "/index.html" } else { path };
        let mut file = root.clone();
        for part in path.split('/').filter(|part| !part.is_empty()) {
            if part == "." || part == ".." || part.contains(['\\', ':']) {
                return None;
            }
            file.push(part);
        }
        let bytes = std::fs::read(&file).ok()?;
        Some(WebAsset {
            bytes,
            mime: mime(&file).to_owned(),
        })
    }))
}

fn qr(text: &str) -> Option<String> {
    let code = QrCode::new(text.as_bytes()).ok()?;
    Some(
        code.render::<Dense1x2>()
            .dark_color(Dense1x2::Light)
            .light_color(Dense1x2::Dark)
            .quiet_zone(true)
            .build(),
    )
}

async fn stopped() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        if let Ok(mut term) = signal(SignalKind::terminate()) {
            tokio::select! {
                _ = term.recv() => {}
                _ = tokio::signal::ctrl_c() => {}
            }
            return;
        }
    }
    let _ = tokio::signal::ctrl_c().await;
}

async fn serve(options: Options, token_note: String) -> ExitCode {
    if !options.web.join("index.html").is_file() {
        eprintln!(
            "berth-server: no index.html in {}: build apps/web or pass --web-dir",
            options.web.display()
        );
    }
    let connectors = Arc::new(Connectors::load());
    tokio::spawn(monitor::run(connectors.clone()));
    tokio::spawn(berth_desktop_lib::stratum::sweep_once());
    let remote = Remote::pinned(
        connectors,
        directory(options.web.clone()),
        Pinned {
            addr: SocketAddr::new(options.bind, options.port),
            public_url: options.public_url.clone(),
        },
    );
    remote.refresh().await;
    let pairing = match remote.pairing() {
        Ok(pairing) => pairing,
        Err(message) => {
            eprintln!("berth-server: cannot start: {message}");
            return ExitCode::FAILURE;
        }
    };
    println!(
        "berth-server listening on {}",
        SocketAddr::new(options.bind, options.port)
    );
    println!("data: {}", options.data.display());
    println!("web: {}", options.web.display());
    println!("pairing token: {token_note}");
    println!("open on the phone or in a browser:\n\n  {pairing}\n");
    if let Some(code) = qr(&pairing) {
        println!("{code}");
    }
    remote.spawn_watch();
    stopped().await;
    println!("berth-server: stopping");
    remote.shutdown();
    ExitCode::SUCCESS
}

fn main() -> ExitCode {
    let options = match Options::parse() {
        Ok(Some(options)) => options,
        Ok(None) => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(message) => {
            eprintln!("berth-server: {message}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    settings::set_berth_dir(options.data.clone());
    let token_note = install_secrets(&options);
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("berth-server: {error}");
            return ExitCode::FAILURE;
        }
    };
    runtime.block_on(serve(options, token_note))
}
