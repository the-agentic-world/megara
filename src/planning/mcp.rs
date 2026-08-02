use std::path::Path;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use rmcp::{
    model::{
        CallToolRequestParams, CallToolResponse, Implementation, ListToolsResult,
        ServerCapabilities, ServerInfo, Tool,
    },
    service::RequestContext,
    transport::stdio,
    ErrorData, RoleServer, ServerHandler, ServiceExt,
};

use super::service::PlanningService;

#[path = "mcp/bridge.rs"]
mod bridge;
#[path = "mcp/catalog.rs"]
mod catalog;

#[derive(Clone)]
struct PlanningMcpServer {
    service: Arc<Mutex<PlanningService>>,
}

impl ServerHandler for PlanningMcpServer {
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let Some(spec) = catalog::tool_spec(&request.name) else {
            return Err(ErrorData::invalid_params("unknown planning tool", None));
        };
        bridge::call_tool(&self.service, spec, request)
    }

    async fn list_tools(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult::with_all_items(catalog::tool_catalog()))
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        catalog::tool_spec(name).map(catalog::tool_value)
    }

    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("megara-planning", env!("CARGO_PKG_VERSION")))
            .with_instructions(
                "Megara manages planning state only; use returned work items with the current host model, submit typed proposals, and never infer approval.",
            )
    }
}

pub fn run(project: &Path) -> Result<()> {
    let service = PlanningService::open_project(project)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()?;
    runtime.block_on(async move {
        let server = PlanningMcpServer {
            service: Arc::new(Mutex::new(service)),
        };
        let running = server.serve(stdio()).await?;
        running.waiting().await?;
        Ok::<(), anyhow::Error>(())
    })?;
    Ok(())
}
