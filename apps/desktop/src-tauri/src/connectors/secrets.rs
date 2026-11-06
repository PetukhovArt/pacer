use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};

use keyring::Entry;

use super::http::{ApiError, ApiResult};

pub const SERVICE: &str = "berth";
const ADOPTED_FILE: &str = "keyring-adopted.json";

pub enum Lookup {
    Found(String),
    Missing,
    Failed(String),
}

pub trait SecretStore: Send + Sync {
    fn save(&self, id: &str, secret: &str) -> Result<(), String>;
    fn load(&self, id: &str) -> Lookup;
    fn delete(&self, id: &str);
}

struct Keyring {
    settled: Mutex<()>,
}

impl Keyring {
    fn new() -> Self {
        Self {
            settled: Mutex::new(()),
        }
    }

    fn entry(service: &str, id: &str) -> Result<Entry, String> {
        Entry::new(service, id).map_err(|error| format!("хранилище ключей недоступно: {error}"))
    }

    fn read(service: &str, id: &str) -> Lookup {
        let entry = match Self::entry(service, id) {
            Ok(entry) => entry,
            Err(error) => return Lookup::Failed(error),
        };
        match entry.get_password() {
            Ok(secret) => Lookup::Found(secret),
            Err(keyring::Error::NoEntry) => Lookup::Missing,
            Err(other) => Lookup::Failed(format!("не удалось прочитать токен: {other}")),
        }
    }

    fn settled_path() -> Option<PathBuf> {
        crate::settings::berth_dir()
            .ok()
            .map(|dir| dir.join(ADOPTED_FILE))
    }

    fn settled_ids(path: &Path) -> Vec<String> {
        fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    fn is_settled(&self, id: &str) -> bool {
        let _guard = self.settled.lock().unwrap_or_else(|e| e.into_inner());
        Self::settled_path()
            .is_none_or(|path| Self::settled_ids(&path).iter().any(|known| known == id))
    }

    fn settle(&self, id: &str) {
        let _guard = self.settled.lock().unwrap_or_else(|e| e.into_inner());
        let Some(path) = Self::settled_path() else {
            return;
        };
        let mut ids = Self::settled_ids(&path);
        if ids.iter().any(|known| known == id) {
            return;
        }
        ids.push(id.to_owned());
        if let Some(dir) = path.parent() {
            let _ = fs::create_dir_all(dir);
        }
        if let Ok(text) = serde_json::to_string_pretty(&ids) {
            let _ = fs::write(&path, text);
        }
    }

    fn adopt(&self, id: &str) -> Lookup {
        if self.is_settled(id) {
            return Lookup::Missing;
        }
        match Self::read(crate::legacy::KEYRING_SERVICE, id) {
            Lookup::Found(secret) => {
                if self.save(id, &secret).is_ok() {
                    self.settle(id);
                }
                Lookup::Found(secret)
            }
            Lookup::Missing => {
                self.settle(id);
                Lookup::Missing
            }
            Lookup::Failed(_) => Lookup::Missing,
        }
    }
}

impl SecretStore for Keyring {
    fn save(&self, id: &str, secret: &str) -> Result<(), String> {
        Self::entry(SERVICE, id)?
            .set_password(secret)
            .map_err(|error| format!("не удалось сохранить токен в хранилище ключей: {error}"))
    }

    fn load(&self, id: &str) -> Lookup {
        match Self::read(SERVICE, id) {
            Lookup::Missing => self.adopt(id),
            other => other,
        }
    }

    fn delete(&self, id: &str) {
        if let Ok(entry) = Self::entry(SERVICE, id) {
            let _ = entry.delete_credential();
        }
        self.settle(id);
    }
}

pub fn keyring_available() -> bool {
    match Keyring::entry(SERVICE, "berth-probe").map(|entry| entry.get_password()) {
        Ok(Ok(_) | Err(keyring::Error::NoEntry)) => true,
        Ok(Err(_)) | Err(_) => false,
    }
}

pub struct FileStore {
    path: PathBuf,
    lock: Mutex<()>,
}

impl FileStore {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            lock: Mutex::new(()),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn read(&self) -> Result<BTreeMap<String, String>, String> {
        match fs::read_to_string(&self.path) {
            Ok(text) => serde_json::from_str(&text)
                .map_err(|error| format!("{} повреждён: {error}", self.path.display())),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(BTreeMap::new()),
            Err(error) => Err(format!(
                "не удалось прочитать {}: {error}",
                self.path.display()
            )),
        }
    }

    fn write(&self, secrets: &BTreeMap<String, String>) -> Result<(), String> {
        let fail =
            |error: io::Error| format!("не удалось записать {}: {error}", self.path.display());
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir).map_err(fail)?;
        }
        let text = serde_json::to_string_pretty(secrets).map_err(|error| error.to_string())?;
        let temp = self.path.with_extension("tmp");
        let mut options = fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        let mut file = options.open(&temp).map_err(fail)?;
        #[cfg(unix)]
        file.set_permissions(std::os::unix::fs::PermissionsExt::from_mode(0o600))
            .map_err(fail)?;
        file.write_all(text.as_bytes()).map_err(fail)?;
        file.sync_all().map_err(fail)?;
        drop(file);
        fs::rename(&temp, &self.path).map_err(|error| {
            let _ = fs::remove_file(&temp);
            fail(error)
        })
    }

    fn guard(&self) -> MutexGuard<'_, ()> {
        self.lock.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl SecretStore for FileStore {
    fn save(&self, id: &str, secret: &str) -> Result<(), String> {
        let _guard = self.guard();
        let mut secrets = self.read()?;
        secrets.insert(id.to_owned(), secret.to_owned());
        self.write(&secrets)
    }

    fn load(&self, id: &str) -> Lookup {
        let _guard = self.guard();
        match self.read() {
            Ok(mut secrets) => secrets.remove(id).map_or(Lookup::Missing, Lookup::Found),
            Err(message) => Lookup::Failed(message),
        }
    }

    fn delete(&self, id: &str) {
        let _guard = self.guard();
        if let Ok(mut secrets) = self.read() {
            if secrets.remove(id).is_some() {
                let _ = self.write(&secrets);
            }
        }
    }
}

static STORE: OnceLock<Box<dyn SecretStore>> = OnceLock::new();

pub fn install(store: Box<dyn SecretStore>) -> bool {
    STORE.set(store).is_ok()
}

fn store() -> &'static dyn SecretStore {
    STORE.get_or_init(|| Box::new(Keyring::new())).as_ref()
}

pub fn save(id: &str, token: &str) -> Result<(), String> {
    store().save(id, token)
}

pub fn lookup(id: &str) -> Lookup {
    store().load(id)
}

pub fn load(id: &str) -> ApiResult<String> {
    match lookup(id) {
        Lookup::Found(secret) => Ok(secret),
        Lookup::Missing => Err(ApiError::failed(
            "токен не найден в хранилище ключей: введите его заново",
        )),
        Lookup::Failed(message) => Err(ApiError::failed(message)),
    }
}

pub fn delete(id: &str) {
    store().delete(id);
}
