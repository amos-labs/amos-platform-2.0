-- Add Claude Opus 4.7 to the BYOK Anthropic model dropdown in system-settings canvas.
-- Also update the OpenAI model list to current models.

UPDATE canvases SET
    js_content = REPLACE(
        js_content,
        '"claude-opus-4-6",
            "claude-haiku-4-5"',
        '"claude-opus-4-6",
            "claude-opus-4-7",
            "claude-haiku-4-5"'
    ),
    updated_at = NOW()
WHERE slug = 'system-settings';
