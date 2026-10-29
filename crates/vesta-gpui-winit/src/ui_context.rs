//! Host-owned context generations protect explicit automation across view changes.
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

pub struct UiAutomationContext {
    identity: Option<(String, Option<String>)>,
    generation: Arc<AtomicU64>,
}
impl Default for UiAutomationContext {
    fn default() -> Self {
        Self {
            identity: None,
            generation: Arc::new(AtomicU64::new(1)),
        }
    }
}
impl UiAutomationContext {
    /// Call at actual context transitions as well as request/presentation boundaries.
    /// The first observation initializes identity without invalidating an empty view.
    pub fn observe(&mut self, project: &str, subject: Option<&str>) -> Result<bool, &'static str> {
        if self.generation.load(Ordering::Acquire) == 0 {
            return Err("context_generation_exhausted");
        }
        let identity = (project.to_owned(), subject.map(str::to_owned));
        if self.identity.as_ref() == Some(&identity) {
            return Ok(false);
        }
        let changed = self.identity.is_some();
        if changed {
            self.invalidate()?;
        }
        self.identity = Some(identity);
        Ok(changed)
    }
    /// Retire grants when the visible surface changes its authorization boundary.
    pub fn invalidate(&mut self) -> Result<(), &'static str> {
        let old = self.generation.load(Ordering::Acquire);
        let next = if old == 0 {
            0
        } else {
            old.checked_add(1).unwrap_or(0)
        };
        self.generation.store(next, Ordering::Release);
        if next == 0 {
            return Err("context_generation_exhausted");
        }
        Ok(())
    }
    pub fn metadata(&self, host: &str) -> Result<Value, &'static str> {
        let (project, subject) = self.identity.as_ref().ok_or("context_not_ready")?;
        let generation = self.generation.load(Ordering::Acquire);
        if generation == 0 {
            return Err("context_generation_exhausted");
        }
        Ok(
            json!({"schema_version":1,"host":host,"project_root":project,"subject":subject,"generation":generation.to_string()}),
        )
    }
    /// Consume only the automation precondition; original request parsers stay authoritative.
    pub fn admit(
        &self,
        body: &mut Value,
        instance: u128,
    ) -> Result<Option<UiContextLease>, &'static str> {
        let value = body
            .as_object_mut()
            .ok_or("invalid_request")?
            .remove("expected_context");
        let Some(value) = value else {
            return Ok(None);
        };
        if value.as_object().is_none_or(|o| {
            o.len() != 2 || !o.contains_key("instance") || !o.contains_key("generation")
        }) {
            return Err("invalid_context");
        }
        if value["instance"].as_str() != Some(instance.to_string().as_str()) {
            return Err("stale_context");
        }
        let generation = value["generation"]
            .as_str()
            .and_then(|s| s.parse::<u64>().ok())
            .filter(|n| {
                *n != 0 && n.to_string() == value["generation"].as_str().unwrap_or_default()
            })
            .ok_or("invalid_context")?;
        let lease = UiContextLease {
            generation: Arc::clone(&self.generation),
            expected: generation,
        };
        if self.identity.is_none() || !lease.is_current() {
            return Err("stale_context");
        }
        Ok(Some(lease))
    }
}

#[derive(Clone)]
pub struct UiContextLease {
    generation: Arc<AtomicU64>,
    expected: u64,
}
impl UiContextLease {
    pub fn is_current(&self) -> bool {
        self.generation.load(Ordering::Acquire) == self.expected
    }
}

/// Check before and after serialization, including GPU waits on the IO worker.
pub struct UiScopedResponse<R> {
    response: R,
    context: Option<UiContextLease>,
}
impl<R> UiScopedResponse<R> {
    pub fn new(response: R, context: Option<UiContextLease>) -> Self {
        Self { response, context }
    }
    pub fn serialize(
        self,
        serialize: impl FnOnce(R) -> Result<Value, &'static str>,
    ) -> Result<Value, &'static str> {
        let valid = || self.context.as_ref().is_none_or(UiContextLease::is_current);
        if !valid() {
            return Err("context_changed");
        }
        let result = serialize(self.response);
        if !valid() {
            return Err("context_changed");
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn context_aba_and_inflight_serialization_never_reuse_a_retired_grant()
    -> Result<(), &'static str> {
        let mut context = UiAutomationContext::default();
        context.observe("project-a", Some("session-a"))?;
        let original =
            json!({"instance":"123","generation":context.metadata("agent")?["generation"]});
        let mut request = json!({"op":"snapshot","expected_context":original});
        let lease = context.admit(&mut request, 123)?;
        assert_eq!(
            context
                .admit(&mut json!({"expected_context":original}), 124)
                .err(),
            Some("stale_context")
        );
        assert!(request.get("expected_context").is_none());
        let response = UiScopedResponse::new(json!({"pixels":"private"}), lease);
        assert_eq!(
            response
                .serialize(|value| {
                    context.observe("project-b", Some("session-b"))?;
                    context.observe("project-a", Some("session-a"))?;
                    Ok(value)
                })
                .expect_err("must not expose another context"),
            "context_changed"
        );
        assert_eq!(
            context
                .admit(&mut json!({"expected_context":original}), 123)
                .err(),
            Some("stale_context")
        );
        assert!(
            context.admit(&mut json!({"op":"windows"}), 123)?.is_none(),
            "legacy trusted API remains explicit"
        );
        let mut timed_out = false;
        let waited = crate::ui_wait::wait(
            json!({"schema_version":1,"request_id":"wait","op":"wait","instance":"123","window":"1",
                "after_revision":"1","expected_context":original}),
            &|request, _: std::sync::mpsc::SyncSender<Result<Value, &'static str>>| {
                assert_eq!(
                    request["expected_context"], original,
                    "each wait subquery must retain the grant"
                );
                Err("stale_context")
            },
            &Ok,
            &mut timed_out,
        );
        assert_eq!(waited.err(), Some("stale_context"));
        assert!(!timed_out);
        Ok(())
    }
}
