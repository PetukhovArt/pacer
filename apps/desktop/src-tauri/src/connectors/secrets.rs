use std::sync::OnceLock;

use keyring::Entry;

use super::http::{ApiError, ApiResult};

const SERVICE: &str = "veyra";

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

struct Keyring;

impl Keyring {
    fn entry(id: &str) -> Result<Entry, String> {
        Entry::new(SERVICE, id).map_err(|error| format!("хранилище ключей недоступно: {error}"))
    }
}

impl SecretStore for Keyring {
    fn save(&self, id: &str, secret: &str) -> Result<(), String> {
        Self::entry(id)?
            .set_password(secret)
            .map_err(|error| format!("не удалось сохранить токен в хранилище ключей: {error}"))
    }

    fn load(&self, id: &str) -> Lookup {
        let entry = match Self::entry(id) {
            Ok(entry) => entry,
            Err(error) => return Lookup::Failed(error),
        };
        match entry.get_password() {
            Ok(secret) => Lookup::Found(secret),
            Err(keyring::Error::NoEntry) => Lookup::Missing,
            Err(other) => Lookup::Failed(format!("не удалось прочитать токен: {other}")),
        }
    }

    fn delete(&self, id: &str) {
        if let Ok(entry) = Self::entry(id) {
            let _ = entry.delete_credential();
        }
    }
}

static STORE: OnceLock<Box<dyn SecretStore>> = OnceLock::new();

pub fn install(store: Box<dyn SecretStore>) -> bool {
    STORE.set(store).is_ok()
}

fn store() -> &'static dyn SecretStore {
    STORE.get_or_init(|| Box::new(Keyring)).as_ref()
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
