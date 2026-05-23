use std::sync::Arc;

use async_trait::async_trait;
use cersei_hooks::{Hook, HookAction, HookContext, HookEvent};

use crate::session::{Session, SessionEvent};

/// Writes Falanx audit trail events (SessionEvent) when Cersei agent lifecycle
/// events fire. Wraps an Arc<Session> and the current iteration number.
pub struct FalanxAuditHook {
    pub session: Arc<Session>,
    pub iteration: u32,
}

#[async_trait]
impl Hook for FalanxAuditHook {
    fn name(&self) -> &str {
        "falanx-audit"
    }

    fn events(&self) -> &[HookEvent] {
        &[HookEvent::PreToolUse, HookEvent::PostToolUse, HookEvent::Stop]
    }

    async fn on_event(&self, ctx: &HookContext) -> HookAction {
        match ctx.event {
            HookEvent::PreToolUse => {
                let agent = ctx
                    .tool_name
                    .as_deref()
                    .unwrap_or("unknown")
                    .to_string();
                let _ = self.session.append(SessionEvent::AgentInvoked {
                    agent,
                    iteration: self.iteration,
                });
            }
            HookEvent::Stop => {
                // Agent completed — orchestrator writes RunCompleted explicitly.
            }
            _ => {}
        }
        HookAction::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_hook_implements_hook_trait() {
        fn assert_hook<T: Hook + Send + Sync>() {}
        assert_hook::<FalanxAuditHook>();
    }

    #[test]
    fn audit_hook_name() {
        let dir = tempfile::tempdir().unwrap();
        let session = Arc::new(Session::new(dir.path(), "test").unwrap());
        let hook = FalanxAuditHook { session, iteration: 0 };
        assert_eq!(hook.name(), "falanx-audit");
    }
}
