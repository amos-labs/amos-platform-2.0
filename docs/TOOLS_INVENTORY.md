# DETAILED TOOLS INVENTORY

## All 60+ Tools by Category

### CORE TOOLS (Keep - Essential)

#### Platform Tools (4 tools, 307 LOC)
1. **PlatformQueryTool** - Query collections/records
2. **PlatformCreateTool** - Create records in collections
3. **PlatformUpdateTool** - Update collection records
4. **PlatformExecuteTool** - Execute platform operations

#### Canvas Tools (5 tools, 415 LOC)
1. **LoadCanvasTool** - Load existing canvases
2. **CreateDynamicCanvasTool** - Generate data-driven canvases (uses Bedrock)
3. **CreateFreeformCanvasTool** - Create custom HTML/JS canvases
4. **UpdateCanvasTool** - Modify canvas configuration
5. **PublishCanvasTool** - Publish canvases publicly

#### Credential Tools (2 tools, 270 LOC)
1. **CollectCredentialTool** - Capture credentials via Secure Input Canvas
2. **ListVaultCredentialsTool** - List stored credentials (no plaintext)

#### Schema Tools (7 tools, 566 LOC)
1. **DefineCollectionTool** - Create dynamic data models
2. **ListCollectionsTool** - List all collections
3. **GetCollectionTool** - Get collection schema
4. **CreateRecordTool** - Add records
5. **QueryRecordsTool** - Search records
6. **UpdateRecordTool** - Update records
7. **DeleteRecordTool** - Delete records

#### Task Tools (5 tools, 610 LOC)
1. **CreateTaskTool** - Create internal background tasks
2. **CreateBountyTool** - Post external bounties for OpenClaw agents
3. **ListTasksTool** - List task status
4. **GetTaskResultTool** - Retrieve task results
5. **CancelTaskTool** - Cancel pending tasks

#### Integration Tools (8 tools, 974 LOC)
1. **ListIntegrationsTool** - List available integrations
2. **ListConnectionsTool** - List active API connections
3. **CreateConnectionTool** - Add new integration connection
4. **TestConnectionTool** - Test connection before using
5. **ExecuteIntegrationActionTool** - Call integration API
6. **ListOperationsTool** - List available integration operations
7. **CreateSyncConfigTool** - Configure ETL sync
8. **TriggerSyncTool** - Manually trigger sync

---

### FEATURE TOOLS (Keep but Isolate - Nice-to-Have)

#### Revision Tools (5 tools, 650 LOC)
1. **ListRevisionsTool** - List entity revisions
2. **GetRevisionTool** - Get specific revision
3. **RevertEntityTool** - Revert to previous revision
4. **ListTemplatesTool** - List available templates
5. **CheckTemplateUpdatesTool** - Check for template updates

#### Site Tools (5 tools, 446 LOC)
1. **CreateSiteTool** - Create new website
2. **CreatePageTool** - Add pages to site
3. **UpdatePageTool** - Modify page content
4. **PublishSiteTool** - Publish site publicly
5. **ListSitesTool** - List sites

#### OpenClaw Tools (5 tools, 535 LOC)
1. **RegisterAgentTool** - Register autonomous agent
2. **ListAgentsTool** - List registered agents
3. **AssignTaskTool** - Assign task to specific agent
4. **GetAgentStatusTool** - Check agent status
5. **StopAgentTool** - Stop running agent

#### Memory Tools (2 tools, 237 LOC)
1. **RememberThisTool** - Store information in working memory
2. **SearchMemoryTool** - Retrieve from memory (salience-based)

#### Document Tools (1 tool, 188 LOC)
1. **GenerateDocumentTool** - Export to PDF/DOCX

#### Image Generation Tools (1 tool, 160 LOC)
1. **GenerateImageTool** - Generate images (Google Imagen)

#### Web Tools (2 tools, 195 LOC)
1. **WebSearchTool** - Search the web
2. **ViewWebPageTool** - Scrape web pages

---

### STUB TOOLS (REMOVE - Security Risk)

#### System Tools (2 tools, 161 LOC) - REMOVE IMMEDIATELY
1. **ReadFileTool** - Read arbitrary files
2. **BashTool** - Execute arbitrary bash commands

**Why Remove**:
- Direct filesystem and shell access in production AI system is dangerous
- No validation of commands/paths
- Potential privilege escalation risk
- Can read sensitive files (.env, /etc/passwd, etc.)

---

## TOOL REGISTRATION ORDER

In `tools/mod.rs::default_registry()` (477 LOC):

```rust
// Core platform tools (ESSENTIAL)
- PlatformQueryTool
- PlatformCreateTool
- PlatformUpdateTool
- PlatformExecuteTool

// Canvas tools (ESSENTIAL)
- LoadCanvasTool
- CreateDynamicCanvasTool
- CreateFreeformCanvasTool
- UpdateCanvasTool
- PublishCanvasTool

// Web tools (FEATURE - could be removed)
- WebSearchTool
- ViewWebPageTool

// System tools (STUB - REMOVE)
- ReadFileTool          // SECURITY RISK
- BashTool              // SECURITY RISK

// Memory tools (FEATURE - experimental)
- RememberThisTool
- SearchMemoryTool

// OpenClaw tools (FEATURE - external integration)
- RegisterAgentTool
- ListAgentsTool
- AssignTaskTool
- GetAgentStatusTool
- StopAgentTool

// Schema tools (ESSENTIAL)
- DefineCollectionTool
- ListCollectionsTool
- GetCollectionTool
- CreateRecordTool
- QueryRecordsTool
- UpdateRecordTool
- DeleteRecordTool

// Site tools (FEATURE - website builder)
- CreateSiteTool
- CreatePageTool
- UpdatePageTool
- PublishSiteTool
- ListSitesTool

// Task queue tools (ESSENTIAL)
- CreateTaskTool
- CreateBountyTool
- ListTasksTool
- GetTaskResultTool
- CancelTaskTool

// Document tools (FEATURE)
- GenerateDocumentTool

// Image tools (FEATURE)
- GenerateImageTool

// Revision tools (FEATURE)
- ListRevisionsTool
- GetRevisionTool
- RevertEntityTool
- ListTemplatesTool
- CheckTemplateUpdatesTool

// Credential tools (ESSENTIAL)
- CollectCredentialTool
- ListVaultCredentialsTool

// Integration tools (ESSENTIAL)
- ListIntegrationsTool
- ListConnectionsTool
- CreateConnectionTool
- TestConnectionTool
- ExecuteIntegrationActionTool
- ListOperationsTool
- CreateSyncConfigTool
- TriggerSyncTool
```

---

## TOOL DEPENDENCIES

```
Agent Loop
    ├─ Tool Registry
    │  ├─ Platform Tools (CRUD)
    │  │  └─ Schema Tools (collection definitions)
    │  ├─ Canvas Tools
    │  │  └─ Canvas Engine (renderer)
    │  ├─ Integration Tools
    │  │  └─ API Executor (http + auth)
    │  │  └─ ETL Pipeline
    │  ├─ Task Tools
    │  │  └─ Task Queue
    │  │  └─ Sub-Agent (internal execution)
    │  ├─ OpenClaw Tools (DUPLICATE of Task Tools)
    │  │  └─ OpenClaw Manager (WebSocket)
    │  ├─ Credential Tools
    │  │  └─ Credential Vault (AES-256)
    │  ├─ Site Tools (website builder)
    │  │  └─ Site Engine
    │  ├─ Revision Tools (version control)
    │  ├─ Memory Tools (salience-based)
    │  ├─ Web Tools
    │  ├─ System Tools (REMOVE)
    │  ├─ Document Tools (PDF/DOCX)
    │  └─ Image Tools (Google Imagen)
```

---

## CONSOLIDATION OPPORTUNITIES

### 1. Merge OpenClaw + Task Tools
- Both handle work delegation
- OpenClaw is just external dispatcher
- Task Queue handles both internal + external
- **Potential savings: 235 LOC**

### 2. Reduce Schema Tools (7 → 4)
- DefineCollection + GetCollection = same operation
- Create/Update/Delete/Query could be unified
- **Potential savings: 200 LOC**

### 3. Web Tools → Integration Tools
- Web search is just HTTP API call
- Should be generic integration action
- **Potential savings: 195 LOC**

### 4. Memory Tools → Schema Tools
- Memory is just specialized collection
- Could use DefineCollection + QueryRecords
- **Potential savings: 237 LOC**

### 5. Remove System Tools
- Bash execution is security liability
- File read should go through Document tools
- **Potential savings: 161 LOC**

---

## TOTAL TOOL CONSOLIDATION POTENTIAL

**Current**: 60 tools in 6,400 LOC  
**After consolidation**: 40 tools in ~5,600 LOC (12% reduction)  
**After full refactor**: 30 tools in ~4,800 LOC (25% reduction)

**Keeps full functionality** while removing duplication and security risks.

