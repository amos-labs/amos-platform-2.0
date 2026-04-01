//! Multi-harness orchestration module.
//!
//! The orchestrator enables a primary harness to discover and delegate work to
//! specialist harnesses. Each specialist runs a subset of packages (e.g.,
//! autoresearch, education) keeping tool counts manageable per LLM call.
//!
//! Discovery modes:
//! - **Platform API** (production): queries the platform's `/sync/siblings` endpoint
//! - **Environment** (dev): parses `AMOS_SIBLING_HARNESSES=name:url,name:url`

pub mod discovery;
pub mod proxy;
pub mod tools;

use crate::tools::ToolRegistry;
use amos_core::AppConfig;
use std::sync::Arc;

pub use discovery::SiblingHarness;
pub use proxy::HarnessProxy;

/// Top-level orchestrator that owns discovery + proxy and registers tools.
pub struct HarnessOrchestrator {
    pub proxy: Arc<HarnessProxy>,
}

impl HarnessOrchestrator {
    /// Create a new orchestrator. Starts background discovery refresh.
    pub fn new(config: Arc<AppConfig>) -> Self {
        let proxy = Arc::new(HarnessProxy::new(config));
        Self { proxy }
    }

    /// Register the 5 orchestrator tools into the tool registry.
    /// Only called on primary harness instances.
    pub fn register_tools(&self, registry: &mut ToolRegistry) {
        let proxy = self.proxy.clone();
        registry.register(Arc::new(tools::ListHarnessesTool::new(proxy.clone())));
        registry.register(Arc::new(tools::DelegateToHarnessTool::new(proxy.clone())));
        registry.register(Arc::new(tools::SubmitTaskToHarnessTool::new(proxy.clone())));
        registry.register(Arc::new(tools::GetHarnessStatusTool::new(proxy.clone())));
        registry.register(Arc::new(tools::BroadcastToHarnessesTool::new(proxy)));
    }

    /// Trigger an immediate discovery refresh.
    pub async fn refresh_discovery(&self) {
        self.proxy.refresh().await;
    }
}
