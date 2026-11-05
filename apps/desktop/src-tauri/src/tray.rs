use std::sync::Mutex;
use std::time::Duration;

use serde_json::{Map, Value};
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, State, Window, WindowEvent, Wry};

use crate::settings;

const TRAY_ID: &str = "main";
const MAIN_WINDOW: &str = "main";
const NEW_SESSION_EVENT: &str = "tray://new-session";
const POLL_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Lang {
    En,
    Ru,
}

struct Labels {
    show: &'static str,
    new_session: &'static str,
    remote: &'static str,
    quit: &'static str,
}

impl Lang {
    fn labels(self) -> Labels {
        match self {
            Self::En => Labels {
                show: "Show Veyra",
                new_session: "New session…",
                remote: "Remote access",
                quit: "Quit Veyra",
            },
            Self::Ru => Labels {
                show: "Показать Veyra",
                new_session: "Новая сессия…",
                remote: "Удалённый доступ",
                quit: "Выйти из Veyra",
            },
        }
    }

    fn status(self, running: u32, waiting: u32) -> String {
        let mut parts = Vec::new();
        if running > 0 {
            parts.push(match self {
                Self::En => format!("{running} running"),
                Self::Ru => format!("{running} работают"),
            });
        }
        if waiting > 0 {
            parts.push(match self {
                Self::En => format!("{waiting} waiting"),
                Self::Ru => format!("{waiting} ждут ответа"),
            });
        }
        if parts.is_empty() {
            "Veyra".to_owned()
        } else {
            format!("Veyra\n{}", parts.join(", "))
        }
    }
}

struct Prefs {
    lang: Lang,
    remote: bool,
    close_to_tray: bool,
}

fn read_prefs() -> Prefs {
    let value: Value = serde_json::from_str(&settings::settings_text()).unwrap_or(Value::Null);
    let flag = |key: &str| value.get(key).and_then(Value::as_bool).unwrap_or(false);
    Prefs {
        lang: match value.get("ui.language").and_then(Value::as_str) {
            Some("ru") => Lang::Ru,
            _ => Lang::En,
        },
        remote: flag("remote.enabled"),
        close_to_tray: flag("window.closeToTray"),
    }
}

#[derive(Default)]
pub struct TrayStatus(Mutex<(u32, u32)>);

fn refresh_tooltip(app: &AppHandle, lang: Lang) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return;
    };
    let (running, waiting) = *app
        .state::<TrayStatus>()
        .0
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let _ = tray.set_tooltip(Some(lang.status(running, waiting)));
}

#[tauri::command]
pub fn tray_status(app: AppHandle, status: State<'_, TrayStatus>, running: u32, waiting: u32) {
    *status.0.lock().unwrap_or_else(|e| e.into_inner()) = (running, waiting);
    refresh_tooltip(&app, read_prefs().lang);
}

fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

async fn write_remote(enabled: bool) {
    let text = settings::settings_text();
    let mut object = match serde_json::from_str::<Value>(&text) {
        Ok(Value::Object(object)) => object,
        _ if text.trim().is_empty() => Map::new(),
        _ => return,
    };
    if enabled {
        object.insert("remote.enabled".to_owned(), Value::Bool(true));
    } else {
        object.remove("remote.enabled");
    }
    let Ok(mut out) = serde_json::to_string_pretty(&object) else {
        return;
    };
    out.push('\n');
    let _ = settings::settings_write(out, None).await;
}

pub fn install(app: &AppHandle) -> tauri::Result<()> {
    app.manage(TrayStatus::default());
    let prefs = read_prefs();
    let labels = prefs.lang.labels();
    let show = MenuItem::with_id(app, "show", labels.show, true, None::<&str>)?;
    let new_session =
        MenuItem::with_id(app, "new-session", labels.new_session, true, None::<&str>)?;
    let remote = CheckMenuItem::with_id(
        app,
        "remote",
        labels.remote,
        true,
        prefs.remote,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, "quit", labels.quit, true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&show, &new_session, &remote, &separator, &quit])?;

    let toggled = remote.clone();
    let mut builder = TrayIconBuilder::<Wry>::with_id(TRAY_ID)
        .tooltip("Veyra")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "show" => show_main(app),
            "new-session" => {
                show_main(app);
                let _ = app.emit(NEW_SESSION_EVENT, ());
            }
            "remote" => {
                let enabled = toggled.is_checked().unwrap_or(false);
                tauri::async_runtime::spawn(write_remote(enabled));
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut seen = settings::settings_text();
        loop {
            tokio::time::sleep(POLL_INTERVAL).await;
            let text = settings::settings_text();
            if text == seen {
                continue;
            }
            seen = text;
            let prefs = read_prefs();
            let labels = prefs.lang.labels();
            let _ = show.set_text(labels.show);
            let _ = new_session.set_text(labels.new_session);
            let _ = remote.set_text(labels.remote);
            let _ = quit.set_text(labels.quit);
            let _ = remote.set_checked(prefs.remote);
            refresh_tooltip(&app, prefs.lang);
        }
    });
    Ok(())
}

pub fn on_window_event(window: &Window, event: &WindowEvent) {
    if let WindowEvent::CloseRequested { api, .. } = event {
        if window.label() == MAIN_WINDOW && read_prefs().close_to_tray {
            api.prevent_close();
            let _ = window.hide();
        }
    }
}
