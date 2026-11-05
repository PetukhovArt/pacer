use keyring::Entry;

use super::http::{ApiError, ApiResult};

const SERVICE: &str = "veyra";

fn entry(id: &str) -> Result<Entry, String> {
    Entry::new(SERVICE, id).map_err(|error| format!("хранилище ключей недоступно: {error}"))
}

pub fn save(id: &str, token: &str) -> Result<(), String> {
    entry(id)?
        .set_password(token)
        .map_err(|error| format!("не удалось сохранить токен в хранилище ключей: {error}"))
}

pub fn load(id: &str) -> ApiResult<String> {
    entry(id)
        .map_err(ApiError::Failed)?
        .get_password()
        .map_err(|error| match error {
            keyring::Error::NoEntry => {
                ApiError::failed("токен не найден в хранилище ключей: введите его заново")
            }
            other => ApiError::failed(format!("не удалось прочитать токен: {other}")),
        })
}

pub fn delete(id: &str) {
    if let Ok(entry) = entry(id) {
        let _ = entry.delete_credential();
    }
}
