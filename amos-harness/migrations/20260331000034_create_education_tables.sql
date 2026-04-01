-- Education Package: Performance-critical tables with proper indexes.
-- These complement the dynamic schema collections (edu_*) with tables
-- that need specialized indexing for SCORM runtime, law search, and learning paths.

-- ── SCORM Runtime Data ──────────────────────────────────────────────────
-- Stores CMI data model state per learner per SCO.
-- Needs fast read/write during active course playback.

CREATE TABLE IF NOT EXISTS edu_scorm_runtime (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    package_id UUID NOT NULL,              -- references edu_scorm_packages record
    learner_id UUID NOT NULL,              -- references edu_learners record
    sco_id VARCHAR(255) NOT NULL,          -- SCO identifier from manifest
    cmi_data JSONB NOT NULL DEFAULT '{}',  -- full CMI data model state
    lesson_status VARCHAR(50) DEFAULT 'not attempted',
    score_raw DECIMAL(5,2),
    score_min DECIMAL(5,2) DEFAULT 0,
    score_max DECIMAL(5,2) DEFAULT 100,
    total_time_seconds INTEGER DEFAULT 0,
    session_time_seconds INTEGER DEFAULT 0,
    suspend_data TEXT,                     -- SCORM suspend_data for bookmarking
    lesson_location VARCHAR(1000),         -- last position in course
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(package_id, learner_id, sco_id)
);

CREATE INDEX IF NOT EXISTS idx_scorm_runtime_learner ON edu_scorm_runtime(learner_id);
CREATE INDEX IF NOT EXISTS idx_scorm_runtime_package ON edu_scorm_runtime(package_id);
CREATE INDEX IF NOT EXISTS idx_scorm_runtime_status ON edu_scorm_runtime(lesson_status);

-- ── Learner Sessions ────────────────────────────────────────────────────
-- Tracks active and historical learning sessions for analytics.

CREATE TABLE IF NOT EXISTS edu_learner_sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    learner_id UUID NOT NULL,
    course_id UUID NOT NULL,
    package_id UUID,                       -- SCORM package if applicable
    session_type VARCHAR(50) NOT NULL DEFAULT 'scorm',  -- scorm, assessment, law_review, adaptive
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    ended_at TIMESTAMPTZ,
    duration_seconds INTEGER,
    progress_percent DECIMAL(5,2) DEFAULT 0,
    score DECIMAL(5,2),
    status VARCHAR(50) NOT NULL DEFAULT 'active',  -- active, completed, abandoned, timed_out
    metadata JSONB NOT NULL DEFAULT '{}'   -- session-specific data (device, IP, etc.)
);

CREATE INDEX IF NOT EXISTS idx_sessions_learner ON edu_learner_sessions(learner_id);
CREATE INDEX IF NOT EXISTS idx_sessions_course ON edu_learner_sessions(course_id);
CREATE INDEX IF NOT EXISTS idx_sessions_active ON edu_learner_sessions(status) WHERE status = 'active';
CREATE INDEX IF NOT EXISTS idx_sessions_started ON edu_learner_sessions(started_at DESC);

-- ── Law Statutes ────────────────────────────────────────────────────────
-- Indexed state laws with vector embeddings for semantic search.
-- Officers can query by topic, keyword, or natural language.

CREATE TABLE IF NOT EXISTS edu_law_statutes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    state_code VARCHAR(2) NOT NULL,        -- e.g., 'TX', 'CA', 'FL'
    statute_number VARCHAR(100) NOT NULL,  -- e.g., '18.2-308.1', 'PC 832'
    title VARCHAR(1000) NOT NULL,
    full_text TEXT NOT NULL,               -- complete statute text
    summary TEXT,                          -- AI-generated plain-language summary
    category VARCHAR(255),                 -- e.g., 'use_of_force', 'traffic', 'criminal_procedure'
    subcategory VARCHAR(255),
    effective_date DATE,
    last_amended DATE,
    source_url TEXT,                        -- link to official source
    embedding vector(1536),                -- semantic search embedding
    metadata JSONB NOT NULL DEFAULT '{}',  -- extra fields (penalties, cross-references, etc.)
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(state_code, statute_number)
);

CREATE INDEX IF NOT EXISTS idx_statutes_state ON edu_law_statutes(state_code);
CREATE INDEX IF NOT EXISTS idx_statutes_category ON edu_law_statutes(state_code, category);
CREATE INDEX IF NOT EXISTS idx_statutes_text ON edu_law_statutes USING GIN(to_tsvector('english', full_text));
CREATE INDEX IF NOT EXISTS idx_statutes_title ON edu_law_statutes USING GIN(to_tsvector('english', title));
CREATE INDEX IF NOT EXISTS idx_statutes_embedding
    ON edu_law_statutes USING hnsw (embedding vector_cosine_ops);

-- ── Learning Paths ──────────────────────────────────────────────────────
-- AI-generated personalized learning paths for each officer.

CREATE TABLE IF NOT EXISTS edu_learning_paths (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    learner_id UUID NOT NULL,
    title VARCHAR(500) NOT NULL,
    description TEXT,
    status VARCHAR(50) NOT NULL DEFAULT 'active',  -- active, completed, archived
    generated_by VARCHAR(100) DEFAULT 'learning_coach',  -- which agent created it
    path_data JSONB NOT NULL DEFAULT '[]', -- ordered list of steps: [{course_id, type, reason, priority}]
    target_competencies JSONB DEFAULT '[]', -- competency areas this path addresses
    estimated_hours DECIMAL(6,2),
    completed_steps INTEGER DEFAULT 0,
    total_steps INTEGER DEFAULT 0,
    due_date TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_paths_learner ON edu_learning_paths(learner_id);
CREATE INDEX IF NOT EXISTS idx_paths_active ON edu_learning_paths(learner_id, status) WHERE status = 'active';

-- ── Knowledge Gaps ──────────────────────────────────────────────────────
-- Tracks identified knowledge gaps per learner for adaptive learning.

CREATE TABLE IF NOT EXISTS edu_knowledge_gaps (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    learner_id UUID NOT NULL,
    competency_area VARCHAR(255) NOT NULL,  -- e.g., 'use_of_force', 'miranda_rights', 'traffic_law'
    confidence_score DECIMAL(5,4) DEFAULT 0, -- 0.0 to 1.0 (how confident the system is about the gap)
    proficiency_level DECIMAL(5,4) DEFAULT 0, -- 0.0 to 1.0 (learner's estimated proficiency)
    evidence JSONB NOT NULL DEFAULT '[]',   -- [{source, type, score, date}] backing data
    last_assessed TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    recommended_action VARCHAR(500),        -- what the learning coach suggests
    status VARCHAR(50) NOT NULL DEFAULT 'identified', -- identified, addressed, resolved
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(learner_id, competency_area)
);

CREATE INDEX IF NOT EXISTS idx_gaps_learner ON edu_knowledge_gaps(learner_id);
CREATE INDEX IF NOT EXISTS idx_gaps_proficiency ON edu_knowledge_gaps(learner_id, proficiency_level);
CREATE INDEX IF NOT EXISTS idx_gaps_active ON edu_knowledge_gaps(status) WHERE status != 'resolved';
