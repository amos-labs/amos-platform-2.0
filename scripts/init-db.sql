-- AMOS Database Initialization Script
-- =====================================
-- Runs once when the postgres container is first created.
-- Creates separate databases for harness and platform, and enables pgvector.

-- Create the harness development database
SELECT 'CREATE DATABASE amos_harness_development'
WHERE NOT EXISTS (SELECT FROM pg_database WHERE datname = 'amos_harness_development')\gexec

-- Create the platform development database
SELECT 'CREATE DATABASE amos_platform_development'
WHERE NOT EXISTS (SELECT FROM pg_database WHERE datname = 'amos_platform_development')\gexec

-- Enable pgvector on the default database
CREATE EXTENSION IF NOT EXISTS vector;

-- Enable pgvector on the harness database
\c amos_harness_development
CREATE EXTENSION IF NOT EXISTS vector;

-- Enable pgvector on the platform database
\c amos_platform_development
CREATE EXTENSION IF NOT EXISTS vector;

-- Switch back to default
\c amos_development
