use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use futures_util::future::join_all;
use serde::Serialize;
use serde_json::Value;
use tokio::sync::Notify;

use super::ai::{self, AiKind, AiView};
use super::types::Status;
use super::{ConnectorView, Connectors, Health, Record};
use crate::settings;
use crate::sink::BoxSink;

const TICK: Duration = Duration::from_secs(5);
const FIRST_RETRY: Duration = Duration::from_secs(15);
const MAX_RETRY: Duration = Duration::from_secs(300);
const HEALTHY_RECHECK: Duration = Duration::from_secs(300);
const SETTING: &str = "connectors.autoReconnect";
const PROBES: [&str; 2] = ["1.1.1.1:53", "[2606:4700:4700::1111]:53"];

#[derive(Debug, Clone, Serialize)]
pub struct ConnectorsChanged {
    connectors: Vec<ConnectorView>,
    ai: Vec<AiView>,
}

#[derive(Default)]
pub struct Hub {
    wake: Notify,
    nudged: AtomicBool,
    reports: Mutex<HashMap<String, String>>,
    watchers: Mutex<Vec<BoxSink<ConnectorsChanged>>>,
    published: Mutex<Option<String>>,
}

fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

impl Hub {
    pub(super) fn report_unreachable(&self, id: &str, message: &str) {
        locked(&self.reports).insert(id.to_owned(), message.to_owned());
        self.wake.notify_one();
    }

    fn take_reports(&self) -> HashMap<String, String> {
        std::mem::take(&mut *locked(&self.reports))
    }
}

struct Slot {
    down: bool,
    delay: Duration,
    next: Instant,
}

impl Slot {
    fn healthy(now: Instant) -> Self {
        Slot {
            down: false,
            delay: Duration::ZERO,
            next: now + HEALTHY_RECHECK,
        }
    }

    fn down(delay: Duration, now: Instant) -> Self {
        Slot {
            down: true,
            delay,
            next: now + delay,
        }
    }
}

#[derive(Default)]
struct Schedule(HashMap<String, Slot>);

impl Schedule {
    fn due(&mut self, key: &str, status: Status, now: Instant) -> bool {
        match status {
            Status::Error => {
                self.0.remove(key);
                false
            }
            Status::Unchecked => true,
            Status::Ok => match self.0.get(key) {
                Some(slot) if !slot.down => slot.next <= now,
                _ => {
                    self.0.insert(key.to_owned(), Slot::healthy(now));
                    false
                }
            },
            Status::Unreachable => match self.0.get(key) {
                Some(slot) if slot.down => slot.next <= now,
                _ => {
                    self.0.insert(key.to_owned(), Slot::down(FIRST_RETRY, now));
                    false
                }
            },
        }
    }

    fn done(&mut self, key: &str, status: Status, now: Instant) {
        match status {
            Status::Unreachable => {
                let delay = match self.0.get(key) {
                    Some(slot) if slot.down => (slot.delay * 2).clamp(FIRST_RETRY, MAX_RETRY),
                    _ => FIRST_RETRY,
                };
                self.0.insert(key.to_owned(), Slot::down(delay, now));
            }
            Status::Ok => {
                self.0.insert(key.to_owned(), Slot::healthy(now));
            }
            _ => {
                self.0.remove(key);
            }
        }
    }

    fn mark_down(&mut self, key: &str, now: Instant) {
        if !self.0.get(key).is_some_and(|slot| slot.down) {
            self.0.insert(key.to_owned(), Slot::down(FIRST_RETRY, now));
        }
    }

    fn expedite(&mut self, now: Instant) {
        for slot in self.0.values_mut().filter(|slot| slot.down) {
            slot.next = now;
            slot.delay = Duration::ZERO;
        }
    }
}

fn connector_key(id: &str) -> String {
    format!("c:{id}")
}

fn ai_key(kind: AiKind) -> String {
    format!("a:{kind:?}")
}

fn enabled() -> bool {
    serde_json::from_str::<Value>(&settings::settings_text())
        .ok()
        .and_then(|value| value.get(SETTING).and_then(Value::as_bool))
        .unwrap_or(true)
}

fn network_up() -> bool {
    PROBES.iter().any(|probe| {
        let Ok(addr) = probe.parse::<SocketAddr>() else {
            return false;
        };
        let local = if addr.is_ipv4() {
            "0.0.0.0:0"
        } else {
            "[::]:0"
        };
        UdpSocket::bind(local)
            .and_then(|socket| socket.connect(addr))
            .is_ok()
    })
}

impl Connectors {
    fn statuses(&self) -> Vec<(String, Status)> {
        let inner = self.lock();
        inner
            .records
            .iter()
            .map(|record| {
                let status = inner
                    .health
                    .get(&record.id)
                    .map_or(Status::Unchecked, |health| health.status);
                (record.id.clone(), status)
            })
            .collect()
    }

    fn mark_unreachable(&self, id: &str, message: String) -> bool {
        let mut inner = self.lock();
        if !inner.records.iter().any(|record| record.id == id) {
            return false;
        }
        let status = inner.health.get(id).map(|health| health.status);
        if matches!(status, Some(Status::Unreachable | Status::Error)) {
            return false;
        }
        inner.health.insert(
            id.to_owned(),
            Health {
                status: Status::Unreachable,
                error: Some(message),
                account: None,
            },
        );
        true
    }

    pub(super) fn publish(&self) {
        let event = ConnectorsChanged {
            connectors: self.views(),
            ai: ai::snapshots(self),
        };
        let signature = serde_json::to_string(&event).ok();
        {
            let mut last = locked(&self.hub.published);
            if *last == signature {
                return;
            }
            *last = signature;
        }
        locked(&self.hub.watchers).retain(|sink| sink.send(event.clone()));
    }
}

pub async fn connectors_watch(
    state: &Connectors,
    on_event: BoxSink<ConnectorsChanged>,
) -> Result<(), String> {
    locked(&state.hub.watchers).push(on_event);
    Ok(())
}

pub async fn connectors_reconnect(state: &Connectors) -> Result<(), String> {
    state.hub.nudged.store(true, Ordering::Relaxed);
    state.hub.wake.notify_one();
    Ok(())
}

async fn sweep(state: &Connectors, schedule: &mut Schedule, now: Instant) {
    let due: Vec<Record> = state
        .statuses()
        .into_iter()
        .filter(|(id, status)| schedule.due(&connector_key(id), *status, now))
        .filter_map(|(id, _)| state.record(&id).ok())
        .collect();
    let due_ai: Vec<AiKind> = ai::watched(state)
        .into_iter()
        .filter(|(kind, status)| schedule.due(&ai_key(*kind), *status, now))
        .map(|(kind, _)| kind)
        .collect();
    if due.is_empty() && due_ai.is_empty() {
        return;
    }
    let (checked, rechecked) = tokio::join!(
        join_all(due.iter().map(|record| async move {
            let result = state.check_record(record).await;
            (record.id.clone(), Health::from_result(&result))
        })),
        join_all(
            due_ai
                .iter()
                .map(|kind| async move { (*kind, ai::recheck(state, *kind).await) })
        ),
    );
    let finished = Instant::now();
    for (id, health) in checked {
        let status = health.status;
        state.set_health(&id, health);
        schedule.done(&connector_key(&id), status, finished);
    }
    for (kind, status) in rechecked {
        schedule.done(&ai_key(kind), status, finished);
    }
    state.publish();
}

fn absorb_reports(state: &Connectors, schedule: &mut Schedule, now: Instant) {
    let mut changed = false;
    for (id, message) in state.hub.take_reports() {
        if state.mark_unreachable(&id, message) {
            schedule.mark_down(&connector_key(&id), now);
            changed = true;
        }
    }
    if changed {
        state.publish();
    }
}

async fn tick(state: &Connectors, schedule: &mut Schedule, online: &mut bool) {
    if !enabled() {
        return;
    }
    let up = network_up();
    let returned = up && !*online;
    *online = up;
    if !up {
        return;
    }
    let now = Instant::now();
    if state.hub.nudged.swap(false, Ordering::Relaxed) || returned {
        schedule.expedite(now);
    }
    absorb_reports(state, schedule, now);
    sweep(state, schedule, now).await;
}

pub async fn run(state: Arc<Connectors>) {
    let mut schedule = Schedule::default();
    let mut online = true;
    loop {
        tick(&state, &mut schedule, &mut online).await;
        tokio::select! {
            _ = tokio::time::sleep(TICK) => {}
            _ = state.hub.wake.notified() => {}
        }
    }
}
