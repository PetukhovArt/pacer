use std::fmt;
use std::net::IpAddr;
use std::sync::OnceLock;
use std::time::Duration;

use reqwest::{Client, Method, Proxy, RequestBuilder, Response, StatusCode, Url};
use serde_json::Value;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const TOTAL_TIMEOUT: Duration = Duration::from_secs(20);
const JSON: &str = "application/json";
const ANY: &str = "*/*";
const TEXT_LIMIT: u64 = 5 * 1024 * 1024;

#[derive(Debug, Clone)]
pub enum ApiError {
    Unreachable(String),
    Failed(String),
    Rejected(String),
}

impl ApiError {
    pub fn failed(message: impl Into<String>) -> Self {
        ApiError::Failed(message.into())
    }

    pub fn message(&self) -> &str {
        match self {
            ApiError::Unreachable(message)
            | ApiError::Failed(message)
            | ApiError::Rejected(message) => message,
        }
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message())
    }
}

impl From<ApiError> for String {
    fn from(error: ApiError) -> Self {
        error.message().to_owned()
    }
}

pub type ApiResult<T> = Result<T, ApiError>;

fn env_var(names: &[&str]) -> Option<String> {
    names
        .iter()
        .filter_map(|name| std::env::var(name).ok())
        .map(|value| value.trim().to_owned())
        .find(|value| !value.is_empty())
}

fn is_private_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback() || v4.is_private() || v4.is_link_local() || v4.is_unspecified()
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unspecified()
                || (v6.segments()[0] & 0xfe00) == 0xfc00
                || (v6.segments()[0] & 0xffc0) == 0xfe80
        }
    }
}

fn bypass(host: &str) -> bool {
    let host = host
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_ascii_lowercase();
    if let Ok(ip) = host.parse::<IpAddr>() {
        if is_private_ip(ip) {
            return true;
        }
    }
    if host == "localhost" || !host.contains('.') {
        return true;
    }
    if [".local", ".lan", ".internal", ".localdomain"]
        .iter()
        .any(|suffix| host.ends_with(suffix))
    {
        return true;
    }
    env_var(&["NO_PROXY", "no_proxy"]).is_some_and(|list| {
        list.split(',').map(str::trim).any(|entry| {
            if entry == "*" {
                return true;
            }
            let entry = entry.trim_start_matches('*').trim_start_matches('.');
            !entry.is_empty() && (host == entry || host.ends_with(&format!(".{entry}")))
        })
    })
}

fn proxy_for(url: &Url) -> Option<Url> {
    if bypass(url.host_str()?) {
        return None;
    }
    let raw = if url.scheme() == "https" {
        env_var(&["HTTPS_PROXY", "https_proxy", "ALL_PROXY", "all_proxy"])
    } else {
        env_var(&["HTTP_PROXY", "http_proxy", "ALL_PROXY", "all_proxy"])
    }?;
    Url::parse(&raw)
        .or_else(|_| Url::parse(&format!("http://{raw}")))
        .ok()
}

fn client() -> &'static Client {
    static CLIENT: OnceLock<Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        Client::builder()
            .user_agent("veyra-desktop")
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(TOTAL_TIMEOUT)
            .no_proxy()
            .proxy(Proxy::custom(proxy_for))
            .build()
            .expect("http client")
    })
}

pub fn host_of(url: &Url) -> String {
    url.host_str().unwrap_or("").to_owned()
}

fn transport_error(host: &str, error: &reqwest::Error) -> ApiError {
    if error.is_timeout() {
        ApiError::Unreachable(format!("{host} не отвечает (таймаут)"))
    } else if error.is_connect() {
        ApiError::Unreachable(format!("нет связи с {host}"))
    } else if error.is_redirect() {
        ApiError::Failed(format!("слишком много перенаправлений от {host}"))
    } else if error.is_decode() || error.is_body() {
        ApiError::Unreachable(format!("обрыв соединения с {host}"))
    } else {
        ApiError::Unreachable(format!("нет связи с {host}"))
    }
}

fn remote_message(body: &str) -> Option<String> {
    let body = body.trim();
    if body.is_empty() || body.starts_with('<') {
        return None;
    }
    let text = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|json| {
            json["message"]
                .as_str()
                .or_else(|| json["errorMessages"][0].as_str())
                .or_else(|| json["error_description"].as_str())
                .or_else(|| json["error"].as_str())
                .map(str::to_owned)
        })
        .unwrap_or_else(|| body.to_owned());
    Some(text.chars().take(200).collect())
}

async fn status_error(host: &str, response: Response) -> ApiError {
    let status = response.status();
    let detail = response
        .text()
        .await
        .ok()
        .and_then(|body| remote_message(&body));
    let code = status.as_u16();
    let base = match status {
        StatusCode::UNAUTHORIZED => format!("токен отклонён (401) на {host}"),
        StatusCode::FORBIDDEN => format!("доступ запрещён (403) на {host}"),
        StatusCode::NOT_FOUND => format!("не найдено (404) на {host}"),
        StatusCode::TOO_MANY_REQUESTS => format!("лимит запросов исчерпан (429) на {host}"),
        StatusCode::BAD_GATEWAY | StatusCode::SERVICE_UNAVAILABLE | StatusCode::GATEWAY_TIMEOUT => {
            let message = format!("{host} недоступен ({code})");
            return ApiError::Unreachable(match detail {
                Some(detail) => format!("{message}: {detail}"),
                None => message,
            });
        }
        _ => format!("ошибка {code} от {host}"),
    };
    let message = match detail {
        Some(detail) => format!("{base}: {detail}"),
        None => base,
    };
    if status == StatusCode::UNAUTHORIZED {
        ApiError::Rejected(message)
    } else {
        ApiError::Failed(message)
    }
}

#[derive(Clone, Default)]
pub struct Api {
    headers: Vec<(&'static str, String)>,
}

impl Api {
    pub fn new(headers: Vec<(&'static str, String)>) -> Self {
        Api { headers }
    }

    fn builder(&self, method: Method, url: Url, auth: bool, accept: &str) -> RequestBuilder {
        let mut request = client().request(method, url).header("Accept", accept);
        if auth {
            for (name, value) in &self.headers {
                request = request.header(*name, value);
            }
        }
        request
    }

    pub fn parse_url(raw: &str, query: &[(&str, &str)]) -> ApiResult<Url> {
        let mut url =
            Url::parse(raw).map_err(|_| ApiError::failed(format!("некорректный адрес: {raw}")))?;
        if !query.is_empty() {
            url.query_pairs_mut().extend_pairs(query);
        }
        Ok(url)
    }

    async fn send(&self, request: RequestBuilder, host: &str) -> ApiResult<Response> {
        let response = request
            .send()
            .await
            .map_err(|error| transport_error(host, &error))?;
        if response.status().is_success() {
            Ok(response)
        } else {
            Err(status_error(host, response).await)
        }
    }

    pub async fn get(&self, raw: &str, query: &[(&str, &str)]) -> ApiResult<Response> {
        let url = Self::parse_url(raw, query)?;
        let host = host_of(&url);
        self.send(self.builder(Method::GET, url, true, JSON), &host)
            .await
    }

    pub async fn get_unauth(&self, url: Url) -> ApiResult<Response> {
        let host = host_of(&url);
        self.send(self.builder(Method::GET, url, false, ANY), &host)
            .await
    }

    pub async fn get_with_auth(&self, url: Url) -> ApiResult<Response> {
        let host = host_of(&url);
        self.send(self.builder(Method::GET, url, true, ANY), &host)
            .await
    }

    pub async fn get_text(
        &self,
        raw: &str,
        query: &[(&str, &str)],
        accept: &str,
    ) -> ApiResult<String> {
        let url = Self::parse_url(raw, query)?;
        let host = host_of(&url);
        let response = self
            .send(self.builder(Method::GET, url, true, accept), &host)
            .await?;
        if response
            .content_length()
            .is_some_and(|len| len > TEXT_LIMIT)
        {
            return Err(ApiError::failed("файл слишком большой"));
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|_| ApiError::failed(format!("обрыв соединения с {host}")))?;
        if bytes.len() as u64 > TEXT_LIMIT {
            return Err(ApiError::failed("файл слишком большой"));
        }
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    pub async fn json(&self, raw: &str, query: &[(&str, &str)]) -> ApiResult<Value> {
        let response = self.get(raw, query).await?;
        let host = response.url().host_str().unwrap_or("").to_owned();
        response
            .json::<Value>()
            .await
            .map_err(|_| ApiError::failed(format!("неожиданный ответ от {host}: не JSON")))
    }

    pub async fn post_json(&self, raw: &str, body: &Value) -> ApiResult<Value> {
        let url = Self::parse_url(raw, &[])?;
        let host = host_of(&url);
        let request = self.builder(Method::POST, url, true, JSON).json(body);
        let response = self.send(request, &host).await?;
        response
            .json::<Value>()
            .await
            .map_err(|_| ApiError::failed(format!("неожиданный ответ от {host}: не JSON")))
    }
}

pub fn text(value: &Value) -> String {
    value.as_str().unwrap_or_default().to_owned()
}

pub fn opt(value: &Value) -> Option<String> {
    value.as_str().filter(|s| !s.is_empty()).map(str::to_owned)
}

pub fn encode_segment(segment: &str) -> String {
    url::form_urlencoded::byte_serialize(segment.as_bytes())
        .collect::<String>()
        .replace('+', "%20")
}
