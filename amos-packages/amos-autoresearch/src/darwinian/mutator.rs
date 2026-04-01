//! LLM-based prompt mutation — proposes targeted find/replace edits to an
//! agent's system prompt based on its fitness scorecard and peer prompts.

use crate::types::{Experiment, ExperimentStatus, ExperimentType};
use amos_core::{AmosError, Result};
use chrono::Utc;
use serde_json::Value as JsonValue;
use sqlx::{PgPool, Row};
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Drives prompt mutation via an LLM (Ollama by default). Given a
/// low-performing agent, it fetches the agent's prompt and scorecard, asks
/// the LLM for a targeted find/replace edit, and records the experiment.
pub struct Mutator {
    db_pool: PgPool,
    http_client: reqwest::Client,
}

/// A single find/replace mutation proposed by the LLM.
#[derive(Debug, serde::Deserialize)]
pub struct MutationProposal {
    pub find: String,
    pub replace: String,
    pub reasoning: String,
}

/// Ollama `/api/generate` request body.
#[derive(Debug, serde::Serialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    stream: bool,
}

/// Ollama `/api/generate` response body (non-streaming).
#[derive(Debug, serde::Deserialize)]
struct OllamaResponse {
    response: String,
}

impl Mutator {
    /// Create a new `Mutator`.
    pub fn new(db_pool: PgPool, http_client: reqwest::Client) -> Self {
        Self {
            db_pool,
            http_client,
        }
    }

    /// Find the lowest-fitness agent in the swarm that is eligible for
    /// mutation. An agent is eligible when it has no active or evaluating
    /// experiment **and** its cooldown period (if any) has elapsed.
    ///
    /// Returns `None` if no eligible agent exists.
    pub async fn find_mutation_target(&self, swarm_id: Uuid) -> Result<Option<i32>> {
        debug!(swarm_id = %swarm_id, "searching for mutation target");

        let row = sqlx::query(
            r#"
            SELECT m.agent_id
            FROM agent_swarm_members m
            LEFT JOIN darwinian_experiments e
                ON e.agent_id = m.agent_id
                AND e.swarm_id = m.swarm_id
                AND e.status IN ('active', 'evaluating')
            LEFT JOIN openclaw_agents a
                ON a.id = m.agent_id
            WHERE m.swarm_id = $1
              AND e.id IS NULL
              AND (a.experiment_cooldown_until IS NULL
                   OR a.experiment_cooldown_until < NOW())
            ORDER BY COALESCE(m.fitness_score, 0.0) ASC
            LIMIT 1
            "#,
        )
        .bind(swarm_id)
        .fetch_optional(&self.db_pool)
        .await
        .map_err(|e| AmosError::Internal(format!("Failed to find mutation target: {e}")))?;

        Ok(row.map(|r| r.get("agent_id")))
    }

    /// Propose and (if an LLM is available) apply a prompt mutation for the
    /// given agent.
    ///
    /// 1. Fetches the agent's current `system_prompt`.
    /// 2. Fetches the agent's latest scorecard.
    /// 3. Fetches anonymized peer prompts from the same swarm.
    /// 4. Builds a mutation prompt and sends it to Ollama.
    /// 5. Parses the response as a [`MutationProposal`].
    /// 6. Applies the find/replace to produce a mutated prompt.
    /// 7. Persists the experiment and updates the agent's prompt.
    /// 8. Sets the agent's cooldown.
    ///
    /// If Ollama is unreachable the experiment is saved with status
    /// `proposed` and no prompt change is applied.
    pub async fn propose_mutation(&self, agent_id: i32, swarm_id: Uuid) -> Result<Experiment> {
        // 1. Fetch current system prompt
        let current_prompt = self.get_agent_prompt(agent_id).await?;

        // 2. Fetch latest scorecard
        let scorecard_summary = self
            .get_latest_scorecard_summary(agent_id, swarm_id)
            .await?;

        // 3. Fetch anonymized peer prompts
        let peer_prompts = self.get_peer_prompts(agent_id, swarm_id).await?;

        // 4. Build the mutation prompt
        let llm_prompt = format!(
            r#"You are an expert prompt engineer optimizing an AI agent's system prompt for better performance.

Current prompt:
{current_prompt}

Performance scorecard:
{scorecard_summary}

Peer agent prompts for reference (anonymized):
{peer_prompts}

Propose exactly ONE small, targeted change to improve this agent's performance.
Respond with JSON only:
{{"find": "exact text to find in the prompt", "replace": "replacement text", "reasoning": "why this change should improve performance"}}"#
        );

        // 5. Try calling Ollama
        let proposal = self.call_llm(&llm_prompt).await;

        match proposal {
            Ok(p) => {
                // 6. Apply find/replace
                let mutated_prompt = self.apply_mutation_to_text(&current_prompt, &p);

                // 7. Compute baseline fitness
                let baseline = self.get_current_fitness(agent_id, swarm_id).await?;

                // 8. Persist experiment as 'active'
                let experiment = self
                    .save_experiment(
                        agent_id,
                        swarm_id,
                        &current_prompt,
                        &mutated_prompt,
                        &p,
                        baseline,
                        ExperimentStatus::Active,
                    )
                    .await?;

                // 9. Update agent's system prompt
                self.update_agent_prompt(agent_id, &mutated_prompt).await?;

                // 10. Set cooldown
                self.set_cooldown(agent_id, experiment.cooldown_days)
                    .await?;

                info!(
                    agent_id = agent_id,
                    experiment_id = %experiment.id,
                    "mutation applied — experiment active"
                );

                Ok(experiment)
            }
            Err(e) => {
                warn!(
                    agent_id = agent_id,
                    error = %e,
                    "LLM unavailable — saving experiment as proposed"
                );

                // Save as 'proposed' so it can be retried later
                let experiment = self
                    .save_experiment(
                        agent_id,
                        swarm_id,
                        &current_prompt,
                        &current_prompt, // no change yet
                        &MutationProposal {
                            find: String::new(),
                            replace: String::new(),
                            reasoning: format!("LLM unavailable: {e}"),
                        },
                        None,
                        ExperimentStatus::Proposed,
                    )
                    .await?;

                Ok(experiment)
            }
        }
    }

    /// Apply a find/replace mutation to the given prompt text. If the `find`
    /// string is not present the original text is returned unchanged.
    pub fn apply_mutation(
        &self,
        _agent_id: i32,
        prompt: &str,
        proposal: &MutationProposal,
    ) -> String {
        self.apply_mutation_to_text(prompt, proposal)
    }

    // ── Private helpers ─────────────────────────────────────────────────

    fn apply_mutation_to_text(&self, prompt: &str, proposal: &MutationProposal) -> String {
        if proposal.find.is_empty() {
            return prompt.to_string();
        }
        prompt.replacen(&proposal.find, &proposal.replace, 1)
    }

    async fn get_agent_prompt(&self, agent_id: i32) -> Result<String> {
        let row = sqlx::query("SELECT system_prompt FROM openclaw_agents WHERE id = $1")
            .bind(agent_id)
            .fetch_optional(&self.db_pool)
            .await
            .map_err(|e| AmosError::Internal(format!("Failed to fetch agent prompt: {e}")))?;

        match row {
            Some(r) => {
                let prompt: Option<String> = r.get("system_prompt");
                Ok(prompt.unwrap_or_default())
            }
            None => Err(AmosError::NotFound {
                entity: "Agent".into(),
                id: agent_id.to_string(),
            }),
        }
    }

    async fn get_latest_scorecard_summary(&self, agent_id: i32, swarm_id: Uuid) -> Result<String> {
        let row = sqlx::query(
            r#"
            SELECT fitness_score, tasks_completed, tasks_failed,
                   avg_task_duration_ms, total_tokens_used, total_cost_usd,
                   metric_scores
            FROM agent_scorecards
            WHERE agent_id = $1 AND swarm_id = $2
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(agent_id)
        .bind(swarm_id)
        .fetch_optional(&self.db_pool)
        .await
        .map_err(|e| AmosError::Internal(format!("Failed to fetch scorecard: {e}")))?;

        match row {
            Some(r) => {
                let fitness: f64 = r.get("fitness_score");
                let completed: i32 = r.get("tasks_completed");
                let failed: i32 = r.get("tasks_failed");
                let duration: Option<i64> = r.get("avg_task_duration_ms");
                let tokens: i64 = r.get("total_tokens_used");
                let cost: f64 = r.get("total_cost_usd");
                let metrics: JsonValue = r.get("metric_scores");

                Ok(format!(
                    "fitness={fitness:.3}, tasks_completed={completed}, tasks_failed={failed}, \
                     avg_duration_ms={}, tokens={tokens}, cost_usd={cost:.4}, metrics={metrics}",
                    duration.unwrap_or(0)
                ))
            }
            None => Ok("No scorecard available yet.".to_string()),
        }
    }

    async fn get_peer_prompts(&self, agent_id: i32, swarm_id: Uuid) -> Result<String> {
        let rows = sqlx::query(
            r#"
            SELECT a.system_prompt
            FROM agent_swarm_members m
            JOIN openclaw_agents a ON a.id = m.agent_id
            WHERE m.swarm_id = $1
              AND m.agent_id != $2
              AND a.system_prompt IS NOT NULL
            ORDER BY COALESCE(m.fitness_score, 0.0) DESC
            LIMIT 3
            "#,
        )
        .bind(swarm_id)
        .bind(agent_id)
        .fetch_all(&self.db_pool)
        .await
        .map_err(|e| AmosError::Internal(format!("Failed to fetch peer prompts: {e}")))?;

        if rows.is_empty() {
            return Ok("No peer prompts available.".to_string());
        }

        let prompts: Vec<String> = rows
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let prompt: String = r.get("system_prompt");
                format!("--- Peer {} ---\n{}", i + 1, prompt)
            })
            .collect();

        Ok(prompts.join("\n\n"))
    }

    async fn get_current_fitness(&self, agent_id: i32, swarm_id: Uuid) -> Result<Option<f64>> {
        let row = sqlx::query(
            "SELECT fitness_score FROM agent_swarm_members WHERE agent_id = $1 AND swarm_id = $2",
        )
        .bind(agent_id)
        .bind(swarm_id)
        .fetch_optional(&self.db_pool)
        .await
        .map_err(|e| AmosError::Internal(format!("Failed to fetch current fitness: {e}")))?;

        Ok(row.and_then(|r| r.get("fitness_score")))
    }

    async fn call_llm(&self, prompt: &str) -> Result<MutationProposal> {
        let ollama_url = "http://localhost:11434/api/generate";

        // Check if Ollama is reachable
        let health = self
            .http_client
            .get("http://localhost:11434/api/tags")
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await;

        if health.is_err() {
            return Err(AmosError::Internal(
                "Ollama is not reachable at localhost:11434".into(),
            ));
        }

        let request = OllamaRequest {
            model: "llama3.2".to_string(),
            prompt: prompt.to_string(),
            stream: false,
        };

        let response = self
            .http_client
            .post(ollama_url)
            .json(&request)
            .timeout(std::time::Duration::from_secs(120))
            .send()
            .await
            .map_err(|e| AmosError::Internal(format!("Ollama request failed: {e}")))?;

        if !response.status().is_success() {
            return Err(AmosError::Internal(format!(
                "Ollama returned status {}",
                response.status()
            )));
        }

        let body: OllamaResponse = response
            .json()
            .await
            .map_err(|e| AmosError::Internal(format!("Failed to parse Ollama response: {e}")))?;

        // Extract JSON from the response — the LLM may wrap it in markdown fences.
        let json_str = extract_json(&body.response)?;

        let proposal: MutationProposal = serde_json::from_str(&json_str).map_err(|e| {
            AmosError::Internal(format!(
                "Failed to parse mutation proposal JSON: {e}\nRaw response: {}",
                &body.response
            ))
        })?;

        if proposal.find.is_empty() {
            return Err(AmosError::Internal(
                "LLM returned empty find string in mutation proposal".into(),
            ));
        }

        Ok(proposal)
    }

    #[allow(clippy::too_many_arguments)]
    async fn save_experiment(
        &self,
        agent_id: i32,
        swarm_id: Uuid,
        original_prompt: &str,
        mutated_prompt: &str,
        proposal: &MutationProposal,
        baseline_fitness: Option<f64>,
        status: ExperimentStatus,
    ) -> Result<Experiment> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let diff = serde_json::json!({
            "find": proposal.find,
            "replace": proposal.replace,
        });
        let evaluation_days = 7;
        let cooldown_days = 3;

        let started_at = if status == ExperimentStatus::Active {
            Some(now)
        } else {
            None
        };

        let row = sqlx::query(
            r#"
            INSERT INTO darwinian_experiments
                (id, swarm_id, agent_id, experiment_type, diff,
                 original_prompt, mutated_prompt, status,
                 baseline_fitness, evaluation_days, cooldown_days,
                 proposed_by, proposal_reasoning,
                 started_at, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
            RETURNING id, swarm_id, agent_id, experiment_type, diff,
                      original_prompt, mutated_prompt, status,
                      baseline_fitness, final_fitness, fitness_delta,
                      evaluation_days, cooldown_days,
                      proposed_by, proposal_reasoning,
                      started_at, completed_at, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(swarm_id)
        .bind(agent_id)
        .bind(ExperimentType::PromptMutation.as_str())
        .bind(&diff)
        .bind(original_prompt)
        .bind(mutated_prompt)
        .bind(status.as_str())
        .bind(baseline_fitness)
        .bind(evaluation_days)
        .bind(cooldown_days)
        .bind("darwinian_loop")
        .bind(&proposal.reasoning)
        .bind(started_at)
        .bind(now)
        .bind(now)
        .fetch_one(&self.db_pool)
        .await
        .map_err(|e| AmosError::Internal(format!("Failed to save experiment: {e}")))?;

        Ok(experiment_from_row(&row))
    }

    async fn update_agent_prompt(&self, agent_id: i32, prompt: &str) -> Result<()> {
        sqlx::query("UPDATE openclaw_agents SET system_prompt = $1 WHERE id = $2")
            .bind(prompt)
            .bind(agent_id)
            .execute(&self.db_pool)
            .await
            .map_err(|e| AmosError::Internal(format!("Failed to update agent prompt: {e}")))?;
        Ok(())
    }

    async fn set_cooldown(&self, agent_id: i32, cooldown_days: i32) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE openclaw_agents
            SET experiment_cooldown_until = NOW() + ($1 || ' days')::INTERVAL
            WHERE id = $2
            "#,
        )
        .bind(cooldown_days.to_string())
        .bind(agent_id)
        .execute(&self.db_pool)
        .await
        .map_err(|e| AmosError::Internal(format!("Failed to set cooldown: {e}")))?;
        Ok(())
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────

/// Extract a JSON object from LLM output that may contain markdown fences
/// or surrounding prose.
fn extract_json(text: &str) -> Result<String> {
    // Try to find JSON between ```json ... ``` fences first.
    if let Some(start) = text.find("```json") {
        let after_fence = &text[start + 7..];
        if let Some(end) = after_fence.find("```") {
            return Ok(after_fence[..end].trim().to_string());
        }
    }

    // Try bare ``` fences.
    if let Some(start) = text.find("```") {
        let after_fence = &text[start + 3..];
        if let Some(end) = after_fence.find("```") {
            let inner = after_fence[..end].trim();
            if inner.starts_with('{') {
                return Ok(inner.to_string());
            }
        }
    }

    // Try to find a raw JSON object.
    if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            if end > start {
                return Ok(text[start..=end].to_string());
            }
        }
    }

    Err(AmosError::Internal(format!(
        "Could not extract JSON from LLM response: {text}"
    )))
}

/// Map a database row to an [`Experiment`].
fn experiment_from_row(row: &sqlx::postgres::PgRow) -> Experiment {
    Experiment {
        id: row.get("id"),
        swarm_id: row.get("swarm_id"),
        agent_id: row.get("agent_id"),
        experiment_type: row.get("experiment_type"),
        diff: row.get("diff"),
        original_prompt: row.get("original_prompt"),
        mutated_prompt: row.get("mutated_prompt"),
        status: row.get("status"),
        baseline_fitness: row.get("baseline_fitness"),
        final_fitness: row.get("final_fitness"),
        fitness_delta: row.get("fitness_delta"),
        evaluation_days: row.get("evaluation_days"),
        cooldown_days: row.get("cooldown_days"),
        proposed_by: row.get("proposed_by"),
        proposal_reasoning: row.get("proposal_reasoning"),
        started_at: row.get("started_at"),
        completed_at: row.get("completed_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}
