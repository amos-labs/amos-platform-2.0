# AMOS Social Media Package — Design Specification

## An Intelligence Layer for Autonomous Social Media Campaigns

**April 2026 | AMOS Labs**

---

## Overview

This spec defines `amos-social` — a full `AmosPackage` implementation that goes beyond posting tools to provide a complete social media intelligence layer. The package bundles three things:

1. **Tools** — posting, scheduling, analytics, and calendar management across Twitter/X, LinkedIn, Reddit, Hacker News, and Moltbook
2. **System Prompts** — domain expertise for campaign strategy, platform-native content creation, engagement analysis, and multi-week orchestration
3. **Schemas** — campaign tracking, content items, published posts, and analytics data

A social media package that only posts is a thin API wrapper. This package *strategizes, creates, adapts, and optimizes* — encoding the entire reasoning chain from macro thesis to platform-specific content to data-driven iteration.

The package integrates with the credential vault for OAuth/API key management, the automation engine for scheduled delivery, and the bounty system for autonomous campaign execution. It earns a 0.5% attribution fee on bounties completed using its tools (see `PACKAGE_ECONOMY_INTEGRATION.md`).

The meta-narrative is deliberate: AMOS promotes itself through its own bounty system. A content calendar is loaded, bounties are posted for each scheduled item, and agents execute the posts via the social tools. This is the first public demonstration of the AMOS economic loop in production.

---

## Architecture

### Where It Fits

```
┌──────────────────────────────────────────────────────────────────┐
│                        AMOS Harness                              │
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐    │
│  │              amos-social (AmosPackage)                    │    │
│  │                                                          │    │
│  │  ┌────────────────┐  ┌──────────────────────────────┐    │    │
│  │  │  System Prompts │  │          Tools               │    │    │
│  │  │                │  │                              │    │    │
│  │  │  • Strategist  │  │  • PostTweetTool             │    │    │
│  │  │  • Creator     │  │  • PostThreadTool            │    │    │
│  │  │  • Analyst     │  │  • PostLinkedInTool          │    │    │
│  │  │  • Orchestrator│  │  • PostRedditTool            │    │    │
│  │  │                │  │  • PostHackerNewsTool        │    │    │
│  │  └────────────────┘  │  • PostMoltbookTool          │    │    │
│  │                      │  • CommentMoltbookTool       │    │    │
│  │  ┌────────────────┐  │  • LoadContentCalendarTool   │    │    │
│  │  │   Schemas      │  │  • ScheduleContentTool       │    │    │
│  │                      │  • GetPostAnalyticsTool      │    │    │
│  │                      │  • GetCampaignReportTool     │    │    │
│  │  │                │  └──────────────────────────────┘    │    │
│  │  │  • campaigns   │                                      │    │
│  │  │  • content     │                                      │    │
│  │  │  • posts       │                                      │    │
│  │  │  • analytics   │                                      │    │
│  │  └────────────────┘                                      │    │
│  └──────────────────────────────────────────────────────────┘    │
│                                                                  │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────┐             │
│  │  Automation  │  │ Task Queue / │  │ Credential │             │
│  │    Engine    │  │   Bounty     │  │   Vault    │             │
│  └──────────────┘  └──────────────┘  └────────────┘             │
│         │                                    │                   │
│  ┌──────▼────────────────────────────────────▼───┐               │
│  │             ApiExecutor (HTTP out)             │               │
│  └──────────────────────┬────────────────────────┘               │
└─────────────────────────┼────────────────────────────────────────┘
                          │
                          ▼
                   External APIs
                   (X, LinkedIn, Reddit, HN, Moltbook)
```

### Crate Structure

```
amos-packages/amos-social/
    Cargo.toml
    src/
        lib.rs                          # AmosPackage implementation
        tools/
            mod.rs                      # Tool registration
            twitter.rs                  # PostTweetTool, PostThreadTool
            linkedin.rs                 # PostLinkedInTool
            reddit.rs                   # PostRedditTool
            hackernews.rs               # PostHackerNewsTool
            moltbook.rs                 # PostMoltbookTool, CommentMoltbookTool
            calendar.rs                 # LoadContentCalendarTool, ScheduleContentTool
            analytics.rs                # GetPostAnalyticsTool, GetCampaignReportTool
        prompts/
            mod.rs                      # Prompt composition
            system.md                   # Master system prompt
            strategist.md               # Campaign strategy framework
            creator.md                  # Content creation per platform
            analyst.md                  # Engagement analysis patterns
            orchestrator.md             # Campaign orchestration logic
```

### Package Implementation

The social package implements `AmosPackage` following the same pattern as `amos-education`:

```rust
pub struct SocialPackage;

#[async_trait]
impl AmosPackage for SocialPackage {
    fn name(&self) -> &str { "social" }
    fn display_name(&self) -> &str { "Social Media Intelligence" }

    fn description(&self) -> &str {
        "Campaign strategy, content creation, multi-platform posting, \
         engagement analytics, and autonomous campaign orchestration"
    }

    fn version(&self) -> &str { env!("CARGO_PKG_VERSION") }

    fn system_prompt(&self) -> Option<&str> {
        Some(include_str!("prompts/system.md"))
    }

    fn register_tools(&self, registry: &mut dyn PackageToolRegistry, ctx: &PackageContext) {
        let db = ctx.db_pool.clone();
        let pkg = self.name();

        // Posting tools (7)
        registry.register_package_tool(Arc::new(PostTweetTool::new(db.clone())), pkg);
        registry.register_package_tool(Arc::new(PostThreadTool::new(db.clone())), pkg);
        registry.register_package_tool(Arc::new(PostLinkedInTool::new(db.clone())), pkg);
        registry.register_package_tool(Arc::new(PostRedditTool::new(db.clone())), pkg);
        registry.register_package_tool(Arc::new(PostHackerNewsTool::new(db.clone())), pkg);
        registry.register_package_tool(Arc::new(PostMoltbookTool::new(db.clone())), pkg);
        registry.register_package_tool(Arc::new(CommentMoltbookTool::new(db.clone())), pkg);

        // Calendar & scheduling tools (2)
        registry.register_package_tool(Arc::new(LoadContentCalendarTool::new(db.clone())), pkg);
        registry.register_package_tool(Arc::new(ScheduleContentTool::new(db.clone())), pkg);

        // Analytics tools (2)
        registry.register_package_tool(Arc::new(GetPostAnalyticsTool::new(db.clone())), pkg);
        registry.register_package_tool(Arc::new(GetCampaignReportTool::new(db.clone())), pkg);

        tracing::info!("Registered 11 social tools");
    }

    async fn on_activate(&self, ctx: &PackageContext) -> Result<()> {
        bootstrap_schemas(&ctx.db_pool).await?;
        tracing::info!("Social package activated — schemas bootstrapped");
        Ok(())
    }
}
```

Enabled via:
```bash
AMOS_PACKAGES=social
```

---

## Tool Definitions

### 1. PostTweetTool

Posts a single tweet to Twitter/X.

```rust
pub struct PostTweetTool {
    api_executor: Arc<ApiExecutor>,
}

// Tool trait implementation
fn name(&self) -> &str { "post_tweet" }

fn description(&self) -> &str {
    "Post a single tweet to Twitter/X. Requires a Twitter API connection \
     with OAuth 2.0 credentials stored in the vault. Supports text up to \
     280 characters. Returns the tweet ID and URL on success."
}

fn parameters_schema(&self) -> JsonValue {
    json!({
        "type": "object",
        "properties": {
            "connection_id": {
                "type": "string",
                "description": "UUID of the Twitter/X API connection"
            },
            "text": {
                "type": "string",
                "description": "Tweet text (max 280 characters)",
                "maxLength": 280
            },
            "reply_to": {
                "type": "string",
                "description": "Optional: Tweet ID to reply to (for building threads manually)"
            },
            "quote_tweet_id": {
                "type": "string",
                "description": "Optional: Tweet ID to quote"
            }
        },
        "required": ["connection_id", "text"]
    })
}

fn category(&self) -> ToolCategory { ToolCategory::Integration }
```

**API Target:** `POST https://api.twitter.com/2/tweets`

**Auth:** OAuth 2.0 with PKCE (User Context) — requires `tweet.read` and `tweet.write` scopes.

**Execution Flow:**
1. Validate text length (<=280 chars)
2. Resolve credentials via `ApiExecutor` (vault-backed OAuth tokens)
3. Build request body: `{ "text": "...", "reply": { "in_reply_to_tweet_id": "..." } }`
4. POST to Twitter API v2
5. Return tweet ID + constructed URL (`https://x.com/{username}/status/{id}`)
6. On 401: return error indicating token refresh needed

**ToolResult on success:**
```json
{
    "tweet_id": "1234567890",
    "url": "https://x.com/amoslabs/status/1234567890",
    "text": "...",
    "created_at": "2026-04-07T14:30:00Z"
}
```

---

### 2. PostThreadTool

Posts a multi-tweet thread to Twitter/X as a single atomic operation.

```rust
fn name(&self) -> &str { "post_thread" }

fn description(&self) -> &str {
    "Post a multi-tweet thread to Twitter/X. Takes an array of tweet texts \
     and posts them sequentially as replies to each other. Each tweet must be \
     <=280 characters. Returns all tweet IDs and URLs. If any tweet fails, \
     returns partial results with error details."
}

fn parameters_schema(&self) -> JsonValue {
    json!({
        "type": "object",
        "properties": {
            "connection_id": {
                "type": "string",
                "description": "UUID of the Twitter/X API connection"
            },
            "tweets": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "text": {
                            "type": "string",
                            "description": "Tweet text (max 280 characters)",
                            "maxLength": 280
                        }
                    },
                    "required": ["text"]
                },
                "description": "Array of tweets in thread order. First tweet is the root.",
                "minItems": 2,
                "maxItems": 25
            }
        },
        "required": ["connection_id", "tweets"]
    })
}
```

**Execution Flow:**
1. Validate all tweet texts (<=280 chars each)
2. Post first tweet via `POST /2/tweets`
3. For each subsequent tweet: POST with `reply.in_reply_to_tweet_id` set to the previous tweet's ID
4. Collect all results; if a tweet fails mid-thread, return partial results with the error
5. Include a 1-second delay between tweets to respect rate limits

**ToolResult on success:**
```json
{
    "thread_id": "1234567890",
    "thread_url": "https://x.com/amoslabs/status/1234567890",
    "tweets": [
        { "tweet_id": "1234567890", "url": "...", "text": "...", "position": 1 },
        { "tweet_id": "1234567891", "url": "...", "text": "...", "position": 2 }
    ],
    "total_tweets": 7,
    "status": "complete"
}
```

**Partial failure result:**
```json
{
    "thread_id": "1234567890",
    "tweets": [ ... ],
    "total_tweets": 7,
    "completed_tweets": 4,
    "status": "partial",
    "error": "Rate limited at tweet 5. Thread is partially posted.",
    "resume_from": 5
}
```

---

### 3. PostLinkedInTool

Posts content to a LinkedIn profile or company page.

```rust
fn name(&self) -> &str { "post_linkedin" }

fn description(&self) -> &str {
    "Post content to LinkedIn (personal profile or company page). \
     Supports text posts up to 3000 characters. Requires a LinkedIn \
     API connection with OAuth 2.0 credentials in the vault."
}

fn parameters_schema(&self) -> JsonValue {
    json!({
        "type": "object",
        "properties": {
            "connection_id": {
                "type": "string",
                "description": "UUID of the LinkedIn API connection"
            },
            "text": {
                "type": "string",
                "description": "Post text (max 3000 characters)",
                "maxLength": 3000
            },
            "visibility": {
                "type": "string",
                "enum": ["PUBLIC", "CONNECTIONS"],
                "description": "Post visibility (default: PUBLIC)",
                "default": "PUBLIC"
            },
            "post_as": {
                "type": "string",
                "enum": ["personal", "organization"],
                "description": "Post as personal profile or organization page (default: personal)",
                "default": "personal"
            },
            "organization_id": {
                "type": "string",
                "description": "Required if post_as is 'organization'. LinkedIn organization URN."
            }
        },
        "required": ["connection_id", "text"]
    })
}
```

**API Target:** `POST https://api.linkedin.com/v2/posts`

**Auth:** OAuth 2.0 — requires `w_member_social` scope (personal) or `w_organization_social` scope (company page). The LinkedIn API uses a Community Management API for organization posting.

**Request Body (personal post):**
```json
{
    "author": "urn:li:person:{person_id}",
    "lifecycleState": "PUBLISHED",
    "specificContent": {
        "com.linkedin.ugc.ShareContent": {
            "shareCommentary": { "text": "..." },
            "shareMediaCategory": "NONE"
        }
    },
    "visibility": {
        "com.linkedin.ugc.MemberNetworkVisibility": "PUBLIC"
    }
}
```

**Execution Flow:**
1. Resolve OAuth credentials from vault
2. If `post_as == "personal"`: fetch user profile URN via `GET /v2/userinfo`
3. If `post_as == "organization"`: use provided `organization_id`
4. Build UGC post payload
5. POST to LinkedIn API
6. Return post URN + constructed URL

**ToolResult on success:**
```json
{
    "post_urn": "urn:li:share:7654321",
    "url": "https://www.linkedin.com/feed/update/urn:li:share:7654321",
    "text": "...",
    "visibility": "PUBLIC",
    "posted_as": "personal",
    "created_at": "2026-04-07T14:30:00Z"
}
```

---

### 4. PostRedditTool

Posts a submission to a Reddit subreddit.

```rust
fn name(&self) -> &str { "post_reddit" }

fn description(&self) -> &str {
    "Post a submission to a Reddit subreddit. Supports text (self) posts \
     and link posts. Requires a Reddit API connection with OAuth 2.0 \
     credentials in the vault."
}

fn parameters_schema(&self) -> JsonValue {
    json!({
        "type": "object",
        "properties": {
            "connection_id": {
                "type": "string",
                "description": "UUID of the Reddit API connection"
            },
            "subreddit": {
                "type": "string",
                "description": "Subreddit name without r/ prefix (e.g., 'artificial')"
            },
            "title": {
                "type": "string",
                "description": "Post title (max 300 characters)",
                "maxLength": 300
            },
            "text": {
                "type": "string",
                "description": "Post body text (markdown supported). Required for self posts."
            },
            "url": {
                "type": "string",
                "description": "URL for link posts. Mutually exclusive with text."
            },
            "flair_id": {
                "type": "string",
                "description": "Optional: Flair ID for the post"
            }
        },
        "required": ["connection_id", "subreddit", "title"]
    })
}
```

**API Target:** `POST https://oauth.reddit.com/api/submit`

**Auth:** OAuth 2.0 — "script" or "web app" type. Requires `submit` scope. Reddit also requires a User-Agent header identifying the app.

**Execution Flow:**
1. Resolve OAuth credentials from vault
2. Determine post type (`self` if text provided, `link` if URL provided)
3. POST to `/api/submit` with `sr`, `title`, `kind`, `text`/`url`
4. Return post URL from response

**ToolResult on success:**
```json
{
    "post_id": "t3_abc123",
    "url": "https://www.reddit.com/r/artificial/comments/abc123/...",
    "subreddit": "artificial",
    "title": "...",
    "kind": "self",
    "created_at": "2026-04-07T14:30:00Z"
}
```

**Rate Limit Note:** Reddit enforces a 10-minute cooldown between posts for new accounts. The tool should check for `RATELIMIT` errors and return a clear message with the retry-after time.

---

### 5. PostHackerNewsTool

Posts a submission to Hacker News.

```rust
fn name(&self) -> &str { "post_hackernews" }

fn description(&self) -> &str {
    "Post a submission to Hacker News (news.ycombinator.com). Supports \
     link posts ('url' type) and Show HN / Ask HN text posts. Requires \
     HN account credentials in the vault. Note: HN has no official API \
     for posting — this tool uses authenticated form submission."
}

fn parameters_schema(&self) -> JsonValue {
    json!({
        "type": "object",
        "properties": {
            "connection_id": {
                "type": "string",
                "description": "UUID of the HN account connection"
            },
            "title": {
                "type": "string",
                "description": "Post title (max ~80 characters recommended). Prefix with 'Show HN: ' for Show HN posts.",
                "maxLength": 80
            },
            "url": {
                "type": "string",
                "description": "URL for link posts. Omit for text-only (Ask HN) posts."
            },
            "text": {
                "type": "string",
                "description": "Body text for Ask HN or Show HN posts. Only used when url is omitted."
            }
        },
        "required": ["connection_id", "title"]
    })
}
```

**API Target:** `POST https://news.ycombinator.com/submit` (form submission)

**Auth:** Cookie-based session. Login via `POST /login` with username + password, capture `user` cookie. Credentials stored in vault as `{ "username": "...", "password": "..." }`.

**Execution Flow:**
1. Resolve credentials from vault
2. Authenticate via `POST /login` → capture session cookie
3. `GET /submit` → extract CSRF token (`fnid` hidden field)
4. `POST /submit` with `fnid`, `fnop=submit-page`, `title`, `url`/`text`
5. Follow redirect to the new post
6. Extract post ID from redirect URL
7. Return constructed URL

**ToolResult on success:**
```json
{
    "post_id": "40123456",
    "url": "https://news.ycombinator.com/item?id=40123456",
    "title": "Show HN: AMOS – Open protocol for a two-sided bounty marketplace...",
    "type": "show_hn",
    "created_at": "2026-04-07T14:30:00Z"
}
```

**Important Constraints:**
- HN has no official posting API — form submission with CSRF token is the only path
- Account karma affects posting rate limits
- Duplicate URL detection may reject link reposts
- The `ApiExecutor` needs cookie jar support for session-based auth (see Implementation Notes)

---

### 6. PostMoltbookTool

Posts a submission to a Moltbook submolt.

```rust
fn name(&self) -> &str { "post_moltbook" }

fn description(&self) -> &str {
    "Post a submission to a Moltbook submolt. Supports text posts and link posts. \
     Requires a Moltbook API connection with agent credentials and verified claim tweet. \
     Moltbook is agent-only; posts come from verified agent identities."
}

fn parameters_schema(&self) -> JsonValue {
    json!({
        "type": "object",
        "properties": {
            "connection_id": {
                "type": "string",
                "description": "UUID of the Moltbook API connection"
            },
            "submolt": {
                "type": "string",
                "description": "Target submolt name (e.g., 'protocols', 'ai-agents')"
            },
            "title": {
                "type": "string",
                "description": "Post title (max 300 characters)",
                "maxLength": 300
            },
            "body": {
                "type": "string",
                "description": "Post body text (markdown supported, max 10000 characters)",
                "maxLength": 10000
            },
            "post_type": {
                "type": "string",
                "enum": ["text", "link"],
                "description": "Post type — 'text' for text posts, 'link' for link posts (default: 'text')",
                "default": "text"
            },
            "link_url": {
                "type": "string",
                "description": "Required when post_type is 'link'. URL to link to."
            }
        },
        "required": ["connection_id", "submolt", "title", "body"]
    })
}
```

**API Target:** `POST https://api.moltbook.com/v1/posts` (TBD — needs reverse-engineering or official API when available)

**Auth:** Agent authentication via claim tweet verification + API token. The Moltbook auth flow is unique: 1) owner posts a claim tweet linking agent_id, 2) Moltbook verifies the claim tweet, 3) issues an API token for the agent. Store the API token in credential vault.

**Execution Flow:**
1. Resolve credentials from vault (API token)
2. Validate parameters (title <=300 chars, body <=10000 chars)
3. Validate post_type and link_url requirement
4. Build request body:
   ```json
   {
       "submolt": "protocols",
       "title": "...",
       "body": "...",
       "type": "text"
   }
   ```
5. POST to Moltbook API
6. Return post ID + URL

**ToolResult on success:**
```json
{
    "post_id": "m_abc123xyz",
    "url": "https://moltbook.com/m/protocols/posts/m_abc123xyz",
    "submolt": "protocols",
    "title": "...",
    "body": "...",
    "post_type": "text",
    "created_at": "2026-04-07T14:30:00Z"
}
```

**Category:** `ToolCategory::Integration`

**Important Notes:**
- Moltbook is agent-only; posts must come from a verified agent identity
- The AMOS agent's identity on Moltbook IS the AMOS harness itself — meta-demonstration of the platform
- Posts should be substantive protocol discussions rather than promotional content
- Agent-to-agent tone: technical, direct, no marketing language

---

### 7. CommentMoltbookTool

Comments on a Moltbook post or replies to another comment.

```rust
fn name(&self) -> &str { "comment_moltbook" }

fn description(&self) -> &str {
    "Post a comment on a Moltbook post or reply to another comment. \
     Requires the post ID and optional parent comment ID for threading. \
     Requires a Moltbook API connection with agent credentials."
}

fn parameters_schema(&self) -> JsonValue {
    json!({
        "type": "object",
        "properties": {
            "connection_id": {
                "type": "string",
                "description": "UUID of the Moltbook API connection"
            },
            "post_id": {
                "type": "string",
                "description": "ID of the post to comment on"
            },
            "body": {
                "type": "string",
                "description": "Comment body text (markdown supported, max 5000 characters)",
                "maxLength": 5000
            },
            "reply_to_comment_id": {
                "type": "string",
                "description": "Optional: ID of a parent comment to reply to (for threading)"
            }
        },
        "required": ["connection_id", "post_id", "body"]
    })
}
```

**API Target:** `POST https://api.moltbook.com/v1/comments` (TBD)

**Auth:** Same as PostMoltbookTool — agent credentials via API token in vault.

**Execution Flow:**
1. Resolve credentials from vault (API token)
2. Validate parameters (body <=5000 chars)
3. Build request body:
   ```json
   {
       "post_id": "m_abc123xyz",
       "body": "...",
       "reply_to_comment_id": "optional"
   }
   ```
4. POST to Moltbook API
5. Return comment ID + URL

**ToolResult on success:**
```json
{
    "comment_id": "c_def456uvw",
    "url": "https://moltbook.com/m/protocols/posts/m_abc123xyz/comments/c_def456uvw",
    "post_id": "m_abc123xyz",
    "body": "...",
    "created_at": "2026-04-07T14:35:00Z"
}
```

**Category:** `ToolCategory::Integration`

---

### 8. LoadContentCalendarTool

Loads a content calendar from a markdown file or schema records into a structured schedule.

```rust
fn name(&self) -> &str { "load_content_calendar" }

fn description(&self) -> &str {
    "Load a content calendar from a markdown file or from schema records. \
     Parses the calendar into a structured list of scheduled posts with \
     platform, content reference, scheduled date, and status."
}

fn parameters_schema(&self) -> JsonValue {
    json!({
        "type": "object",
        "properties": {
            "source": {
                "type": "string",
                "enum": ["file", "schema"],
                "description": "Load from a markdown file or from schema records"
            },
            "file_path": {
                "type": "string",
                "description": "Path to the content calendar markdown file (when source=file)"
            },
            "collection": {
                "type": "string",
                "description": "Schema collection name (when source=schema, default: 'content_calendar')"
            }
        },
        "required": ["source"]
    })
}
```

**Purpose:** This is the bridge between the content kit document and the automation system. It parses the markdown content calendar table (or schema records) and returns a structured list that `ScheduleContentTool` can consume.

**ToolResult:**
```json
{
    "calendar": [
        {
            "week": 1,
            "platform": "twitter",
            "content_type": "thread",
            "content_ref": "Thread 1",
            "description": "Energy → Dual Threat → AMOS",
            "goal": "Thought leadership, frame the macro thesis",
            "scheduled_date": null,
            "status": "pending"
        }
    ],
    "total_items": 9,
    "platforms": ["twitter", "linkedin", "hackernews", "reddit"]
}
```

---

### 9. ScheduleContentTool

Creates automation rules to post content on the calendar schedule.

```rust
fn name(&self) -> &str { "schedule_content" }

fn description(&self) -> &str {
    "Schedule a content item for posting at a specific date/time. \
     Creates an automation rule with a schedule trigger that fires \
     the appropriate social posting tool. Integrates with the \
     existing automation engine."
}

fn parameters_schema(&self) -> JsonValue {
    json!({
        "type": "object",
        "properties": {
            "platform": {
                "type": "string",
                "enum": ["twitter", "twitter_thread", "linkedin", "reddit", "hackernews", "moltbook"],
                "description": "Target platform"
            },
            "connection_id": {
                "type": "string",
                "description": "UUID of the platform API connection"
            },
            "content": {
                "type": "object",
                "description": "Content payload matching the target tool's schema (e.g., {\"text\": \"...\"} for tweets, {\"tweets\": [...]} for threads)"
            },
            "scheduled_at": {
                "type": "string",
                "description": "ISO 8601 datetime for when to post (e.g., '2026-04-14T09:00:00-04:00')"
            },
            "label": {
                "type": "string",
                "description": "Human-readable label (e.g., 'Week 1 - Thread 1 - Macro Thesis')"
            }
        },
        "required": ["platform", "connection_id", "content", "scheduled_at"]
    })
}
```

**Execution Flow:**
1. Validate the content payload against the target platform's schema
2. Create an automation rule via `AutomationEngine`:
   - Trigger type: `schedule`
   - Trigger config: `{ "cron": null, "run_at": "{scheduled_at}" }` (one-shot)
   - Action type: `run_agent_task`
   - Action config: `{ "tool": "post_tweet|post_thread|...", "params": { ...content } }`
3. Store the scheduled item in a `content_schedule` schema collection for tracking
4. Return the automation ID and scheduled time

**ToolResult:**
```json
{
    "schedule_id": "uuid-here",
    "automation_id": "uuid-here",
    "platform": "twitter_thread",
    "scheduled_at": "2026-04-14T09:00:00-04:00",
    "label": "Week 1 - Thread 1 - Macro Thesis",
    "status": "scheduled"
}
```

---

## System Prompts: The Intelligence Layer

The tools above are the hands. The system prompts below are the expertise. Together they transform a generic agent into a social media strategist.

When the social package is enabled, the following system prompt is injected into the agent's context via `AmosPackage::system_prompt()`. It's composed from multiple prompt files:

### Campaign Strategist Prompt (`prompts/strategist.md`)

Activated when an agent is asked to plan a social media campaign. Encodes:

- **Thesis extraction:** Identify the core positioning ("what's the one thing?") before producing any content
- **Platform mapping:** Map the thesis to platform-specific angles — Twitter wants contrarian hooks, LinkedIn wants professional framing, Reddit wants authenticity, HN wants technical substance
- **Messaging hierarchy:** Week 1 establishes the frame (thought leadership), Week 2 proves credibility (technical), Week 3 expands the conversation (philosophical/viral), Week 4+ sustains presence
- **Audience segmentation:** Different content for developers, business leaders, investors, and community
- **Calendar construction:** Content items with platform, timing, content reference, and dependency chains

### Content Creator Prompt (`prompts/creator.md`)

Activated when producing content for specific platforms. Encodes platform-native voice and constraints:

- **Twitter/X:** Hook in tweet 1 must create curiosity gap. Each tweet self-contained but flowing. End with CTA. 280 char max. Threads of 5-8 tweets perform best. No hashtag spam.
- **LinkedIn:** Professional but not corporate. Lead with bold claim or counterintuitive insight. 1500-2500 chars sweet spot. Line breaks for readability. 2-3 hashtags max.
- **Reddit:** Community member voice, not marketer. Lead with value. Disclose affiliation upfront. Prepare for tough questions with honest answers. Anti-promotional tone.
- **Hacker News:** Technical first. Zero marketing language. Show HN format requires demonstrable build. Concise, factual, clear technical differentiators. Let the work speak.
- **Cross-platform rules:** Never repost the same content across platforms — adapt the voice. Each piece independently valuable, not a teaser pointing to a link.

### Engagement Analyst Prompt (`prompts/analyst.md`)

Activated when reviewing performance data. Encodes:

- **Hook analysis:** Which opening lines drove the most engagement? Extract patterns.
- **Time optimization:** Which posting times and days drove the most impressions per platform?
- **Platform comparison:** Where is the thesis resonating most? Where should we double down?
- **Conversion tracking:** Which posts drove GitHub stars, signups, or other conversion events?
- **Actionable output:** Specific recommendations — "Double down on X, stop doing Y, reframe Z with a stronger hook" — not just metrics.

### Campaign Orchestrator Prompt (`prompts/orchestrator.md`)

Activated when running an ongoing campaign autonomously. Encodes:

- **Status check:** What's been posted vs. scheduled vs. pending?
- **Analytics review:** Pull metrics for posted content, compare against goals
- **Adaptive planning:** Modify upcoming content based on what's performing — amplify winners, reframe underperformers, cross-pollinate successful hooks across platforms
- **Bounty generation:** Post bounties for content creation tasks when operating autonomously within the EAP bounty loop
- **Reporting:** Generate campaign status reports with metrics, trends, and recommendations
- **Moltbook strategy:** For Moltbook posts, use agent-to-agent tone (technical, direct, no marketing language) and focus on substantive protocol discussions rather than promotional content

### How Prompts Compose

The prompts are designed to be activated contextually by the agent based on the task:

```
"Plan a social media campaign"     → Strategist prompt
"Write a Twitter thread about X"   → Creator prompt (Twitter section)
"How did last week's posts do?"    → Analyst prompt
"Run this week's campaign"         → Orchestrator prompt (calls all others)
```

The master `system.md` prompt includes all sections and lets the agent determine which reasoning framework to apply. This is the package's primary value — not the HTTP calls, but the domain expertise that makes agents effective social media operators.

---

### 10. GetPostAnalyticsTool

Retrieves engagement metrics for published posts.

```rust
fn name(&self) -> &str { "get_post_analytics" }

fn description(&self) -> &str {
    "Retrieve engagement analytics for a published post. Returns \
     impressions, likes, reposts, replies, clicks, and other \
     platform-specific metrics. Requires the post ID and platform."
}

fn parameters_schema(&self) -> JsonValue {
    json!({
        "type": "object",
        "properties": {
            "connection_id": {
                "type": "string",
                "description": "UUID of the platform API connection"
            },
            "platform": {
                "type": "string",
                "enum": ["twitter", "linkedin", "reddit", "moltbook"],
                "description": "Platform to query (HN has no analytics API)"
            },
            "post_id": {
                "type": "string",
                "description": "Platform-specific post ID"
            }
        },
        "required": ["connection_id", "platform", "post_id"]
    })
}
```

**API Targets:**
- Twitter: `GET /2/tweets/{id}?tweet.fields=public_metrics`
- LinkedIn: `GET /v2/organizationalEntityShareStatistics?q=organizationalEntity&shares=urn:li:share:{id}`
- Reddit: `GET /api/info?id={post_id}` (returns score, num_comments, upvote_ratio)

**ToolResult:**
```json
{
    "post_id": "1234567890",
    "platform": "twitter",
    "metrics": {
        "impressions": 12500,
        "likes": 340,
        "reposts": 89,
        "replies": 42,
        "clicks": 156,
        "profile_visits": 78
    },
    "retrieved_at": "2026-04-21T10:00:00Z"
}
```

---

### 11. GetCampaignReportTool

Generates an aggregated campaign report across all platforms and posts.

```rust
fn name(&self) -> &str { "get_campaign_report" }

fn description(&self) -> &str {
    "Generate a campaign performance report aggregating analytics \
     across all published posts. Returns per-post metrics, \
     platform comparisons, top performers, and recommendations."
}

fn parameters_schema(&self) -> JsonValue {
    json!({
        "type": "object",
        "properties": {
            "campaign_id": {
                "type": "string",
                "description": "UUID of the campaign (from social_campaigns schema)"
            },
            "date_range": {
                "type": "object",
                "properties": {
                    "start": { "type": "string", "description": "ISO 8601 start date" },
                    "end": { "type": "string", "description": "ISO 8601 end date" }
                }
            }
        },
        "required": ["campaign_id"]
    })
}
```

**Execution Flow:**
1. Load all posts for the campaign from `social_posts` schema
2. Fetch analytics for each post via platform APIs
3. Aggregate metrics by platform, by content type, and by week
4. Identify top performers (highest engagement rate)
5. Store analytics snapshot in `social_analytics` schema
6. Return comprehensive report

**ToolResult:**
```json
{
    "campaign_id": "uuid-here",
    "date_range": { "start": "2026-04-14", "end": "2026-05-05" },
    "summary": {
        "total_posts": 15,
        "total_impressions": 87000,
        "total_engagements": 4200,
        "engagement_rate": "4.8%",
        "top_platform": "twitter",
        "top_post": { "id": "...", "platform": "twitter", "engagement_rate": "8.2%" }
    },
    "by_platform": {
        "twitter": { "posts": 8, "impressions": 52000, "engagements": 2800 },
        "linkedin": { "posts": 3, "impressions": 28000, "engagements": 1100 },
        "reddit": { "posts": 2, "impressions": 5000, "engagements": 250 },
        "hackernews": { "posts": 1, "impressions": 2000, "engagements": 50 }
    },
    "recommendations": [
        "Thread hooks mentioning 'dual threat' outperform by 3x — use in upcoming content",
        "Tuesday 9am ET posting time consistently outperforms — shift all scheduled posts",
        "LinkedIn audience more engaged with productivity urgency angle than philosophical framing"
    ]
}
```

---

## Token Economy Integration

The social package earns a 0.5% attribution fee on bounties completed using its tools. This fee comes from the staker allocation within the relay's 3% protocol fee — task posters don't pay more.

When an agent completes a social media bounty, the result includes tool attribution:

```json
{
    "tools_used": [
        { "tool": "post_thread", "package": "social", "calls": 1 },
        { "tool": "get_post_analytics", "package": "social", "calls": 2 }
    ]
}
```

The relay routes 0.5% of the bounty value to the social package creator's wallet via the on-chain `PackageRegistry` smart contract.

See `PACKAGE_ECONOMY_INTEGRATION.md` for the full economic model, including decay mechanics, governance questions, and fork incentives.

---

## Credential Setup Flow

Before any posting can happen, API credentials must be collected and stored in the vault. This is a one-time setup per platform.

### Twitter/X

1. Agent calls `collect_credential` with service `twitter_oauth2`:
   - User creates a Twitter Developer App at developer.twitter.com
   - App must have OAuth 2.0 with PKCE enabled, User authentication
   - Required scopes: `tweet.read`, `tweet.write`, `users.read`
   - User enters Client ID + Client Secret via Secure Input Canvas
2. Agent calls `create_connection` with `auth_type: "oauth2"` and `vault_credential_id`
3. OAuth 2.0 PKCE flow initiates:
   - Harness generates PKCE code verifier + challenge
   - Redirects user to Twitter authorization URL
   - Captures callback with authorization code
   - Exchanges code for access token + refresh token
   - Stores tokens in vault, linked to connection

**Token Refresh:** The `ApiExecutor` should detect 401 responses and automatically attempt token refresh using the stored refresh token before returning an error.

### LinkedIn

1. Agent calls `collect_credential` with service `linkedin_oauth2`:
   - User creates a LinkedIn App at linkedin.com/developers
   - Required products: "Share on LinkedIn", "Sign in with LinkedIn using OpenID Connect"
   - Required scopes: `openid`, `profile`, `w_member_social`
   - For org posting: add "Community Management API" product, `w_organization_social` scope
   - User enters Client ID + Client Secret via Secure Input Canvas
2. OAuth 2.0 flow (standard authorization code, not PKCE)
3. Access token + refresh token stored in vault

### Reddit

1. Agent calls `collect_credential` with service `reddit_oauth2`:
   - User creates a Reddit app at reddit.com/prefs/apps (type: "script" for personal use)
   - User enters Client ID + Client Secret + username + password via Secure Input Canvas
2. Agent calls `create_connection` with `auth_type: "oauth2"`
3. Script-type OAuth: `POST /api/v1/access_token` with basic auth (client ID:secret) and `grant_type=password`
4. Access token stored in vault

### Hacker News

1. Agent calls `collect_credential` with service `hackernews`:
   - User enters HN username + password via Secure Input Canvas
2. Agent calls `create_connection` with `auth_type: "basic_auth"` and `vault_credential_id`
3. No OAuth flow — session-based auth handled at execution time

---

## Database Schema Additions

### Integration Definitions (seed data)

```sql
-- Twitter/X integration definition
INSERT INTO integrations (id, name, slug, description, base_url, auth_types, status) VALUES
(gen_random_uuid(), 'Twitter/X', 'twitter', 'Post tweets and threads to Twitter/X',
 'https://api.twitter.com/2', ARRAY['oauth2'], 'active');

-- LinkedIn integration definition
INSERT INTO integrations (id, name, slug, description, base_url, auth_types, status) VALUES
(gen_random_uuid(), 'LinkedIn', 'linkedin', 'Post to LinkedIn profiles and company pages',
 'https://api.linkedin.com/v2', ARRAY['oauth2'], 'active');

-- Reddit integration definition
INSERT INTO integrations (id, name, slug, description, base_url, auth_types, status) VALUES
(gen_random_uuid(), 'Reddit', 'reddit', 'Post submissions to Reddit subreddits',
 'https://oauth.reddit.com', ARRAY['oauth2'], 'active');

-- Hacker News integration definition
INSERT INTO integrations (id, name, slug, description, base_url, auth_types, status) VALUES
(gen_random_uuid(), 'Hacker News', 'hackernews', 'Post to Hacker News',
 'https://news.ycombinator.com', ARRAY['basic_auth'], 'active');
```

### Content Schedule Tracking (schema collection)

Rather than a dedicated migration, use the existing runtime schema system:

```json
{
    "collection": "content_schedule",
    "schema": {
        "platform": "string",
        "content_ref": "string",
        "content_payload": "json",
        "scheduled_at": "datetime",
        "posted_at": "datetime",
        "automation_id": "uuid",
        "post_url": "string",
        "post_id": "string",
        "status": "enum:pending,scheduled,posted,failed,cancelled",
        "error": "string",
        "label": "string"
    }
}
```

This leverages the JSONB-backed runtime schemas — no migration needed.

---

## Implementation Notes

### ApiExecutor Extensions

The `ApiExecutor` currently handles header-based auth (API key, bearer token, basic auth). Two extensions are needed:

**1. OAuth 2.0 Token Refresh**

```rust
impl ApiExecutor {
    /// Attempt token refresh when a 401 is received.
    /// Returns Ok(new_token) if refresh succeeds, Err if it fails.
    async fn refresh_oauth_token(
        &self,
        credential_id: Uuid,
        refresh_token: &str,
        token_url: &str,
        client_id: &str,
        client_secret: &str,
    ) -> Result<String, ExecutionError> {
        // POST to token_url with grant_type=refresh_token
        // Update credential_vault with new access_token + refresh_token
        // Return new access_token
    }
}
```

Add retry logic to `execute()`:
```rust
// After initial request returns 401:
if response.status() == 401 && credential.auth_type == "oauth2" {
    if let Some(refresh_token) = credential.get("refresh_token") {
        let new_token = self.refresh_oauth_token(...).await?;
        // Retry the original request with new token
    }
}
```

**2. Cookie Jar Support (for HN)**

```rust
// For session-based auth (HN), the ApiExecutor needs a cookie jar
// Option A: Use reqwest::cookie::Jar per-connection
// Option B: Add a "session" auth type that handles login + cookie capture

impl ApiExecutor {
    async fn session_auth(
        &self,
        login_url: &str,
        credentials: &JsonValue,
    ) -> Result<reqwest::cookie::Jar, ExecutionError> {
        // POST to login_url with username/password
        // Capture and return cookies
    }
}
```

### Rate Limiting

Each platform has different rate limits. Implement a per-connection rate limiter:

| Platform | Rate Limit | Strategy |
|----------|-----------|----------|
| Twitter/X | 200 tweets/15 min (user), 300 tweets/15 min (app) | Token bucket, 1-sec delay between thread tweets |
| LinkedIn | 100 posts/day per member | Daily counter, reset at midnight UTC |
| Reddit | ~1 post/10 min for new accounts, relaxes with karma | Exponential backoff on RATELIMIT response |
| HN | Undocumented, roughly ~1 post/hour | Conservative 1-hour minimum between posts |

```rust
pub struct RateLimiter {
    limits: HashMap<String, RateLimit>,
}

pub struct RateLimit {
    max_requests: u32,
    window_seconds: u64,
    current_count: AtomicU32,
    window_start: AtomicU64,
}

impl RateLimiter {
    pub fn check(&self, connection_id: &str) -> Result<(), Duration> {
        // Returns Ok(()) if under limit, Err(retry_after) if exceeded
    }
}
```

### Error Handling Patterns

All social tools should return structured errors that agents can act on:

```json
{
    "success": false,
    "error": "Rate limited",
    "error_code": "RATE_LIMITED",
    "retry_after_seconds": 600,
    "platform": "reddit",
    "details": "Reddit enforces a 10-minute cooldown between posts for this account."
}
```

Standard error codes:
- `AUTH_EXPIRED` — OAuth token expired, refresh failed
- `AUTH_INVALID` — Credentials rejected
- `RATE_LIMITED` — Platform rate limit hit (includes `retry_after_seconds`)
- `CONTENT_REJECTED` — Platform rejected content (too long, banned words, duplicate)
- `PLATFORM_ERROR` — Platform API returned unexpected error
- `NETWORK_ERROR` — Connection failure

---

## The Self-Promotion Bounty Flow

This is where AMOS dogfoods itself. The complete flow for launching the social media campaign:

### Step 1: Credential Setup (Manual, One-Time)

User (Rick) initiates credential collection:
```
Agent: "Let's set up your social media connections. I'll open a secure input form for each platform."
→ collect_credential(service: "twitter_oauth2", label: "AMOS Labs Twitter")
→ collect_credential(service: "linkedin_oauth2", label: "Rick's LinkedIn")
→ collect_credential(service: "reddit_oauth2", label: "AMOS Labs Reddit")
→ collect_credential(service: "hackernews", label: "Rick's HN Account")
```

### Step 2: Load Content Calendar

```
→ load_content_calendar(source: "file", file_path: "docs/social_media_content.md")
Returns: 9 scheduled content items across 4 platforms
```

### Step 3: Schedule All Content

Agent iterates through the calendar and creates scheduled automations:

```
→ schedule_content(
    platform: "twitter_thread",
    connection_id: "{twitter_conn_id}",
    content: { tweets: [{ text: "Three forces are colliding..." }, ...] },
    scheduled_at: "2026-04-14T09:00:00-04:00",
    label: "Week 1 - Thread 1 - Macro Thesis"
  )

→ schedule_content(
    platform: "linkedin",
    connection_id: "{linkedin_conn_id}",
    content: { text: "Threat 1 is the one everyone knows...", visibility: "PUBLIC" },
    scheduled_at: "2026-04-14T11:00:00-04:00",
    label: "Week 1 - LinkedIn Post 3 - Dual Threat"
  )

// ... repeat for all 9 items
```

### Step 4: Bounty-Based Execution (The Meta Move)

Instead of the automation engine directly calling the tools, the scheduled automations post **bounties** to the task queue:

```
Automation fires at scheduled time
  → create_bounty(
      title: "Post Thread 1 (Macro Thesis) to Twitter/X",
      description: "Post the 7-tweet macro thesis thread to @amoslabs Twitter account.",
      context: { tool: "post_thread", params: { ... }, connection_id: "..." },
      reward_tokens: 50,
      deadline_at: "2026-04-14T12:00:00Z"
    )
```

An AMOS agent (internal or external via OpenClaw) claims the bounty, executes the `post_thread` tool, submits the result (tweet URLs), and earns the reward.

**This is AMOS promoting itself through its own bounty system.** The first public bounties completed on the network are the social media posts that tell the world the network exists. The meta-narrative writes itself.

### Step 5: Analytics & Iteration

The engagement analyst prompt activates. The agent calls `get_post_analytics` for each posted item, then `get_campaign_report` for the aggregated view. Using the analyst system prompt, it identifies top-performing hooks, optimal posting times, and platform-specific resonance patterns. It then posts new bounties for follow-up content that amplifies winners — using the content creator prompt to produce platform-adapted variations of the best-performing themes.

This is the intelligence loop: strategy → creation → posting → analysis → adapted strategy. The system prompts encode the expertise. The tools execute the operations. The bounty system coordinates the work. The token economy compensates everyone involved.

---

## Implementation Priority

### Phase 1: Package Skeleton + Core Posting (Target: This Week)

1. Create `amos-packages/amos-social/` crate with `AmosPackage` implementation
2. Write the system prompts (strategist, creator, analyst, orchestrator)
3. `PostTweetTool` + `PostThreadTool` — Twitter has the highest reach for the developer audience
4. `PostLinkedInTool` — LinkedIn reaches the business/investor audience
5. Credential vault integration for OAuth 2.0
6. Manual execution (agent calls tools directly, no scheduling yet)

### Phase 2: Full Scheduling + Remaining Platforms (Target: Week 2)

7. `PostRedditTool` + `PostHackerNewsTool` + `PostMoltbookTool` + `CommentMoltbookTool`
8. `LoadContentCalendarTool` + `ScheduleContentTool`
9. Automation engine integration for scheduled posting
10. Rate limiter implementation
11. Bootstrap `social_campaigns`, `social_content`, `social_posts` schemas

### Phase 3: Bounty Loop + Analytics (Target: Week 3)

12. Wire scheduled automations to create bounties instead of direct tool calls
13. Internal agent claims and executes social bounties
14. `GetPostAnalyticsTool` per platform
15. `GetCampaignReportTool` with aggregated metrics
16. Bootstrap `social_analytics` schema

### Phase 4: Intelligence Loop (Target: Week 4+)

17. Agent-driven content optimization (analyst prompt → new content bounties)
18. Cross-platform content adaptation (successful Twitter hooks → LinkedIn adaptations)
19. Package attribution fee integration with relay smart contract
20. Autonomous campaign orchestration (orchestrator prompt running on weekly schedule)

---

## Security Considerations

1. **Credentials never in chat.** All API keys and OAuth tokens are collected via the Secure Input Canvas and stored encrypted in the vault. The agent only sees opaque `credential_id` references.

2. **No credential logging.** The `ApiExecutor` strips vault references before logging requests. OAuth tokens are never included in tool results.

3. **Connection scoping.** Social connections are per-harness. Multi-tenant platform ensures no cross-customer credential access.

4. **Content approval.** For the initial launch, consider adding an approval step where the agent shows the post content to the user before executing. This can be implemented via the task message bus (`ApprovalRequest` message type).

5. **Token refresh isolation.** OAuth refresh operations happen within the `ApiExecutor` and update the vault atomically. No race conditions on concurrent refreshes (use database-level locking on the credential row).

---

## Testing Strategy

### Unit Tests

- Tool parameter validation (text length, required fields, enum values)
- Content calendar parser (markdown table → structured data)
- Rate limiter logic
- OAuth token refresh flow (mocked HTTP)

### Integration Tests

- Full OAuth 2.0 flow with test accounts per platform
- Post + verify + delete cycle for each platform
- Thread posting with partial failure simulation
- Credential vault round-trip (store → resolve → use)

### Dogfood Test

- Load the actual `social_media_content.md`
- Schedule all 9 content items
- Execute Week 1 manually
- Verify posts appear on all platforms
- Delete test posts

---

## Appendix: API Reference Summary

| Platform | API | Auth | Post Endpoint | Rate Limit |
|----------|-----|------|---------------|------------|
| Twitter/X | v2 REST | OAuth 2.0 PKCE | `POST /2/tweets` | 200/15min |
| LinkedIn | v2 REST | OAuth 2.0 | `POST /v2/posts` | 100/day |
| Reddit | OAuth REST | OAuth 2.0 (script) | `POST /api/submit` | ~1/10min |
| Hacker News | Form POST | Cookie session | `POST /submit` | ~1/hour |

---

## Related Documents

- **Package Creation Guide:** `PACKAGE_CREATION_GUIDE.md` — how to build packages for the AMOS harness
- **Package Economy Integration:** `PACKAGE_ECONOMY_INTEGRATION.md` — how packages earn from the token economy
- **EAP Specification:** `EAP_SPECIFICATION_v1.md` — the protocol agents use to discover work and earn tokens
- **Token Economy Equations:** `token_economy_equations.md` — the math behind decay, emission, and revenue sharing
- **Social Media Content Kit:** `social_media_content.md` — the actual content this package will post first

---

*This spec is itself a bounty. The first agent to implement it earns reputation on a system that doesn't exist yet — which is exactly the kind of bootstrapping problem AMOS is designed to solve. The tools are the hands. The prompts are the brain. The token economy is the incentive. Together they're the first intelligence layer in the AMOS package ecosystem.*
