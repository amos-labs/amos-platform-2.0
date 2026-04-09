-- Hide the bounty canvas from the navigation sidebar.
-- Bounty management is now handled by the standalone marketplace app (marketplace.amoslabs.com).
-- The harness still proxies bounty API calls to the relay for internal agent use.

UPDATE canvases
SET is_system = false,
    nav_order = NULL
WHERE slug = 'system-bounties';
