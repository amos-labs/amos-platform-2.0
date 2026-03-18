-- Update the system-settings canvas with current model IDs and BYOK-first messaging.

UPDATE canvases SET
    js_content = REPLACE(
        REPLACE(
            REPLACE(
                REPLACE(
                    REPLACE(js_content,
                        'claude-sonnet-4-20250514', 'claude-sonnet-4-6'),
                    'claude-opus-4-20250514', 'claude-opus-4-6'),
                'claude-3-5-haiku-20241022', 'claude-haiku-4-5'),
            'By default, AMOS uses AWS Bedrock. Add your own API key to use Anthropic or OpenAI directly.',
            'Add your Anthropic or OpenAI API key to enable AI chat.'),
        'Provider created! You can now activate it.',
        'Provider created and activated!')
WHERE slug = 'system-settings';
