-- AMOS Database Initialization Script
-- =====================================
-- Runs once when the postgres container is first created.
-- Creates separate databases for harness and platform, and enables pgvector.
--
-- Note: docker-entrypoint-initdb.d runs .sql files via psql,
-- which supports metacommands like \c and \gexec.

-- Enable pgvector on the default database
CREATE EXTENSION IF NOT EXISTS vector;

-- Create the harness database (if it doesn't already exist)
SELECT 'CREATE DATABASE amos_harness_dev'
WHERE NOT EXISTS (SELECT FROM pg_database WHERE datname = 'amos_harness_dev')\gexec

-- Enable pgvector on the harness database
\c amos_harness_dev
CREATE EXTENSION IF NOT EXISTS vector;

-- Create the relay database (if it doesn't already exist)
SELECT 'CREATE DATABASE amos_relay_dev'
WHERE NOT EXISTS (SELECT FROM pg_database WHERE datname = 'amos_relay_dev')\gexec

-- Enable pgvector on the relay database
\c amos_relay_dev
CREATE EXTENSION IF NOT EXISTS vector;

-- Switch back to default
\c amos_development
