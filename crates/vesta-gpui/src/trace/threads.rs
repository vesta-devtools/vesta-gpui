//! Capture-local thread identities shared by CPU, GPU and heap observations.
use super::lock;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Mutex, thread::ThreadId};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TraceThread {
    pub thread: u32,
    pub name: Option<String>,
}
#[derive(Default)]
pub(super) struct Registry(Mutex<(HashMap<ThreadId, u32>, Vec<TraceThread>)>);
impl Registry {
    pub fn current(&self) -> u32 {
        let _quiet = super::memory::suppress();
        let current = std::thread::current();
        let mut registry = lock(&self.0);
        if let Some(id) = registry.0.get(&current.id()) {
            return *id;
        }
        let id = registry.1.len() as u32 + 1;
        let name = current.name().map(|name| name.chars().take(128).collect());
        registry.0.insert(current.id(), id);
        registry.1.push(TraceThread { thread: id, name });
        id
    }
    pub fn snapshot(&self) -> Vec<TraceThread> {
        lock(&self.0).1.clone()
    }
    pub fn clear(&self) {
        *lock(&self.0) = (HashMap::new(), Vec::new());
    }
}
