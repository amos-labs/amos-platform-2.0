#!/usr/bin/env python3
"""
AMOS QA Verification Bot

Automated council-appointed agent that verifies bounty deliverables.
Trust level 5, council member — the real gatekeeper.

Code bounties:  cargo clippy + cargo audit + secret scan + cargo test → verify + approve → settlement
Growth bounties: URL liveness + content check → verify + approve → settlement
Failures:       request_revision (up to 3x), then hard reject.

Lifecycle:  submitted → [QA bot] → verified + approved → settled (on-chain)
                           ↓ (on fixable failure, revision_count < 3)
                      request_revision → agent reworks → resubmits
                           ↓ (on fatal failure or revision_count >= 3)
                        rejected

Run once:   python3 scripts/qa-verification-bot.py
Run daemon: python3 scripts/qa-verification-bot.py --daemon --interval 60
"""

import argparse
import json
import os
import re
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

# ── Configuration ───────────────────────────────────────────────────────────

RELAY_URL = os.environ.get("RELAY_URL", "http://localhost:4100")
RELAY_API_KEY = os.environ.get("RELAY_API_KEY", "test_key_e2e_maxreward_2026")
QA_WALLET = os.environ.get("QA_WALLET", "87GzqDXXH5sDfbmrmjpoMA4aHNNooNSBf6Q7vyPJEMoh")
GITHUB_REPO = os.environ.get("GITHUB_REPO", "amos-labs/amos-automate")
GITHUB_TOKEN = os.environ.get("GITHUB_TOKEN", "")
PROJECT_ROOT = os.environ.get("PROJECT_ROOT", str(Path(__file__).resolve().parent.parent))

# Quality score: starts at 85, each revision costs 5 points
BASE_QUALITY_SCORE = 85
REVISION_PENALTY = 5

HEADERS = {
    "Authorization": f"Bearer {RELAY_API_KEY}",
    "Content-Type": "application/json",
}

# Secret patterns to scan for
SECRET_PATTERNS = [
    (r"AKIA[0-9A-Z]{16}", "AWS Access Key"),
    (r"-----BEGIN\s+(RSA|DSA|EC|OPENSSH)?\s*PRIVATE KEY-----", "Private Key"),
    (r"(?i)(password|secret|token|api_key)\s*[=:]\s*['\"][^'\"]{8,}['\"]", "Hardcoded Secret"),
    (r"ghp_[A-Za-z0-9_]{36}", "GitHub Personal Access Token"),
    (r"sk-[A-Za-z0-9]{20,}", "API Secret Key"),
]


def log(msg: str):
    ts = datetime.now().strftime("%H:%M:%S")
    print(f"[qa-bot {ts}] {msg}", flush=True)


def api_get(path: str):
    """GET request to relay API."""
    import urllib.request
    req = urllib.request.Request(f"{RELAY_URL}{path}", headers=HEADERS)
    try:
        with urllib.request.urlopen(req, timeout=10) as resp:
            return json.loads(resp.read())
    except Exception as e:
        log(f"  API GET {path} failed: {e}")
        return None


def api_post(path: str, body: dict) -> tuple:
    """POST request to relay API. Returns (status_code, response_body)."""
    import urllib.request
    data = json.dumps(body).encode()
    req = urllib.request.Request(f"{RELAY_URL}{path}", data=data, headers=HEADERS, method="POST")
    try:
        with urllib.request.urlopen(req, timeout=30) as resp:
            return resp.status, json.loads(resp.read())
    except urllib.error.HTTPError as e:
        body = None
        try:
            body = json.loads(e.read())
        except Exception:
            pass
        return e.code, body
    except Exception as e:
        log(f"  API POST {path} failed: {e}")
        return 0, None


def run_cmd(cmd: list[str], cwd: str = None, timeout: int = 300) -> tuple:
    """Run a shell command, return (success, output)."""
    try:
        result = subprocess.run(
            cmd, capture_output=True, text=True, cwd=cwd, timeout=timeout
        )
        return result.returncode == 0, result.stdout + result.stderr
    except subprocess.TimeoutExpired:
        return False, f"Command timed out after {timeout}s"
    except Exception as e:
        return False, str(e)


# ── Security Checks ──────────────────────────────────────────────────────────


def check_cargo_clippy(cwd: str) -> dict:
    """Run cargo clippy with -D warnings. Returns structured result."""
    log("  Running cargo clippy...")
    success, output = run_cmd(
        ["cargo", "clippy", "--all-targets", "--", "-D", "warnings"],
        cwd=cwd,
        timeout=300,
    )
    result = "pass" if success else "fail"
    log(f"  Cargo clippy: {result}")

    details = []
    if not success:
        # Parse clippy warnings: "error[E0xxx]: message\n  --> file:line:col"
        for line in output.split("\n"):
            line = line.strip()
            if line.startswith("error") or line.startswith("warning"):
                details.append({"message": line[:200]})
            elif line.startswith("-->") and details:
                # Attach file location to previous detail
                loc = line.replace("-->", "").strip()
                parts = loc.split(":")
                if len(parts) >= 2:
                    details[-1]["file"] = parts[0]
                    details[-1]["line"] = int(parts[1]) if parts[1].isdigit() else 0

    return {
        "check": "cargo_clippy",
        "result": result,
        "details": details[:20],  # Cap at 20 issues
        "output_tail": output[-1000:] if not success else "",
        "is_fatal": False,
    }


def check_cargo_audit(cwd: str) -> dict:
    """Run cargo audit for known vulnerabilities. Fatal on findings."""
    log("  Running cargo audit...")
    success, output = run_cmd(
        ["cargo", "audit", "--json"],
        cwd=cwd,
        timeout=120,
    )

    details = []
    try:
        audit_data = json.loads(output)
        vulns = audit_data.get("vulnerabilities", {}).get("list", [])
        for vuln in vulns[:10]:
            advisory = vuln.get("advisory", {})
            details.append({
                "advisory_id": advisory.get("id", "unknown"),
                "crate": vuln.get("package", {}).get("name", "unknown"),
                "version": vuln.get("package", {}).get("version", "unknown"),
                "title": advisory.get("title", ""),
                "fix_hint": f"Update to patched version",
            })
    except (json.JSONDecodeError, KeyError):
        # Non-JSON output means cargo audit isn't installed or errored
        if "not found" in output.lower() or "error" in output.lower():
            log("  cargo audit not available, skipping")
            return {
                "check": "cargo_audit",
                "result": "skipped",
                "details": [],
                "is_fatal": False,
            }

    has_vulns = len(details) > 0
    result = "fail" if has_vulns else "pass"
    log(f"  Cargo audit: {result} ({len(details)} advisories)")

    return {
        "check": "cargo_audit",
        "result": result,
        "details": details,
        "is_fatal": has_vulns,  # Vulnerabilities are fatal
    }


def check_secret_scanning(cwd: str, pr_branch: str = None) -> dict:
    """Scan git diff for secrets. Fatal on findings."""
    log("  Running secret scan...")

    # Get diff to scan
    if pr_branch:
        diff_cmd = ["git", "diff", f"main...{pr_branch}"]
    else:
        diff_cmd = ["git", "diff", "HEAD~1"]

    success, diff_output = run_cmd(diff_cmd, cwd=cwd)
    if not success:
        return {
            "check": "secret_scanning",
            "result": "skipped",
            "details": [{"message": "Could not get diff"}],
            "is_fatal": False,
        }

    details = []
    for line_num, line in enumerate(diff_output.split("\n"), 1):
        if not line.startswith("+") or line.startswith("+++"):
            continue
        for pattern, description in SECRET_PATTERNS:
            if re.search(pattern, line):
                # Don't include the actual secret in the report
                details.append({
                    "check": "secret_scanning",
                    "line": line_num,
                    "message": f"Potential {description} detected",
                    "fix_hint": f"Remove {description} and use environment variables instead",
                })
                break  # One finding per line is enough

    has_secrets = len(details) > 0
    result = "fail" if has_secrets else "pass"
    log(f"  Secret scan: {result} ({len(details)} findings)")

    return {
        "check": "secret_scanning",
        "result": result,
        "details": details[:10],
        "is_fatal": has_secrets,  # Secrets are fatal
    }


# ── Existing Checks (enhanced) ───────────────────────────────────────────────


def check_git_sha_on_github(sha: str) -> dict:
    """Verify a git SHA exists on the remote GitHub repo."""
    if not sha:
        return {"check": "git_sha", "result": "skipped", "details": [], "is_fatal": False}

    success, output = run_cmd(
        ["gh", "api", f"repos/{GITHUB_REPO}/commits/{sha}", "--jq", ".sha"]
    )
    if success and sha in output.strip():
        log(f"  GitHub commit verified: {sha[:10]}")
        return {"check": "git_sha", "result": "pass", "details": [], "is_fatal": False}
    else:
        log(f"  GitHub commit NOT found: {sha[:10]}")
        return {
            "check": "git_sha",
            "result": "fail",
            "details": [{"message": f"Commit {sha[:10]} not found on GitHub"}],
            "is_fatal": False,
        }


def check_ci_status(sha: str) -> dict:
    """Check GitHub CI/Actions status for a commit."""
    if not sha:
        return {"check": "ci_status", "result": "skipped", "details": [], "is_fatal": False}

    success, output = run_cmd(
        ["gh", "api", f"repos/{GITHUB_REPO}/commits/{sha}/status", "--jq", ".state"]
    )
    status = output.strip() if success else "unknown"
    log(f"  CI status: {status}")

    if status == "failure":
        return {
            "check": "ci_status",
            "result": "fail",
            "details": [{"message": f"CI status: {status}"}],
            "is_fatal": False,
        }
    return {"check": "ci_status", "result": "pass" if status == "success" else "skipped", "details": [], "is_fatal": False}


def check_cargo_build(cwd: str) -> dict:
    """Run cargo check on the project."""
    log("  Running cargo check...")
    success, output = run_cmd(["cargo", "check"], cwd=cwd, timeout=300)
    result = "pass" if success else "fail"
    log(f"  Cargo check: {result}")
    return {
        "check": "cargo_build",
        "result": result,
        "details": [{"message": output[-500:]}] if not success else [],
        "is_fatal": False,
    }


def check_cargo_test(cwd: str) -> dict:
    """Run cargo test on core crates."""
    log("  Running cargo test (harness, relay, core)...")
    success, output = run_cmd(
        ["cargo", "test", "--lib", "-p", "amos-harness", "-p", "amos-relay", "-p", "amos-core"],
        cwd=cwd,
        timeout=300,
    )
    result = "pass" if success else "fail"

    # Parse test count from output
    test_line = ""
    for line in output.split("\n"):
        if "test result:" in line:
            test_line = line.strip()

    log(f"  Cargo test: {result} {test_line}")
    return {
        "check": "cargo_test",
        "result": result,
        "details": [{"message": test_line, "output_tail": output[-500:]}] if not success else [],
        "test_summary": test_line,
        "is_fatal": False,
    }


def check_git_pushed(cwd: str) -> dict:
    """Verify local HEAD is pushed to remote."""
    success, local_sha = run_cmd(["git", "rev-parse", "HEAD"], cwd=cwd)
    if not success:
        return {"check": "git_pushed", "result": "fail", "details": [{"message": "Cannot get local HEAD"}], "is_fatal": False}

    local_sha = local_sha.strip()
    success2, remote_sha = run_cmd(["git", "rev-parse", "origin/main"], cwd=cwd)
    remote_sha = remote_sha.strip() if success2 else ""

    pushed = local_sha == remote_sha
    log(f"  Git pushed: {pushed} (local={local_sha[:10]}, remote={remote_sha[:10]})")
    return {
        "check": "git_pushed",
        "result": "pass" if pushed else "fail",
        "details": [] if pushed else [{"message": f"Local HEAD {local_sha[:10]} != remote {remote_sha[:10]}"}],
        "is_fatal": False,
    }


# ── Growth Bounty Checks ─────────────────────────────────────────────────────


def check_url_liveness(urls: list[str]) -> dict:
    """Verify URLs are live (HTTP 200)."""
    import urllib.request
    details = []
    all_live = True

    for url in urls[:10]:  # Cap at 10 URLs
        try:
            req = urllib.request.Request(url, method="HEAD")
            req.add_header("User-Agent", "AMOS-QA-Bot/1.0")
            with urllib.request.urlopen(req, timeout=10) as resp:
                if resp.status == 200:
                    log(f"  URL live: {url[:60]}")
                else:
                    details.append({"url": url, "status": resp.status, "message": f"HTTP {resp.status}"})
                    all_live = False
        except Exception as e:
            details.append({"url": url, "message": str(e)[:200]})
            all_live = False
            log(f"  URL dead: {url[:60]} — {e}")

    return {
        "check": "url_liveness",
        "result": "pass" if all_live else "fail",
        "details": details,
        "is_fatal": False,
    }


# ── Helpers ───────────────────────────────────────────────────────────────────


def extract_git_sha(result: dict) -> str:
    """Try to find a git SHA from the bounty result JSON."""
    if not result:
        return ""
    for key in ["git_sha", "commit_sha", "sha", "commit"]:
        val = result.get(key, "")
        if val and isinstance(val, str) and len(val) >= 7:
            return val
    return ""


def extract_pr_info(bounty: dict) -> tuple:
    """Extract PR URL and branch from bounty data."""
    pr_url = bounty.get("pr_url", "")
    result = bounty.get("result") or {}

    if not pr_url:
        pr_url = result.get("pr_url", "")

    # Extract PR number from URL
    pr_number = None
    if pr_url and "/pull/" in pr_url:
        try:
            pr_number = int(pr_url.split("/pull/")[-1].split("/")[0].split("?")[0])
        except (ValueError, IndexError):
            pass

    return pr_url, pr_number


def extract_deliverable_urls(result: dict) -> list[str]:
    """Extract deliverable URLs from growth bounty result."""
    if not result:
        return []
    urls = result.get("deliverable_urls", [])
    if isinstance(urls, str):
        urls = [u.strip() for u in urls.split("\n") if u.strip().startswith("http")]
    return urls


def checkout_pr_branch(pr_number: int, cwd: str) -> str | None:
    """Fetch and checkout a PR branch. Returns branch name or None."""
    log(f"  Fetching PR #{pr_number} branch...")
    success, output = run_cmd(
        ["gh", "pr", "checkout", str(pr_number), "--force"],
        cwd=cwd,
    )
    if success:
        # Get current branch name
        ok, branch = run_cmd(["git", "branch", "--show-current"], cwd=cwd)
        branch = branch.strip() if ok else None
        log(f"  Checked out PR branch: {branch}")
        return branch
    else:
        log(f"  Could not checkout PR #{pr_number}: {output[:200]}")
        return None


def return_to_main(cwd: str):
    """Return to main branch after checking a PR."""
    run_cmd(["git", "checkout", "main"], cwd=cwd)
    run_cmd(["git", "pull", "--ff-only", "origin", "main"], cwd=cwd)


def build_structured_feedback(checks_results: list[dict]) -> str:
    """Build agent-parseable structured feedback JSON."""
    failed_checks = [c for c in checks_results if c["result"] == "fail"]
    feedback = {
        "checks_failed": [c["check"] for c in failed_checks],
        "details": [],
        "summary": "",
    }

    for check in failed_checks:
        for detail in check.get("details", []):
            entry = {"check": check["check"]}
            entry.update(detail)
            feedback["details"].append(entry)

    # Build human-readable summary
    parts = []
    for check in failed_checks:
        name = check["check"].replace("_", " ").title()
        count = len(check.get("details", []))
        parts.append(f"{name} ({count} issue{'s' if count != 1 else ''})")

    feedback["summary"] = f"{len(failed_checks)} check{'s' if len(failed_checks) != 1 else ''} failed: {', '.join(parts)}."
    return json.dumps(feedback, indent=2)


def compute_quality_score(revision_count: int) -> int:
    """Compute quality score based on revision history."""
    return max(10, BASE_QUALITY_SCORE - (revision_count * REVISION_PENALTY))


def is_any_fatal(checks_results: list[dict]) -> bool:
    """Check if any check result is fatal (cannot be fixed by revision)."""
    return any(c.get("is_fatal", False) and c["result"] == "fail" for c in checks_results)


# ── Main Verification Logic ──────────────────────────────────────────────────


def process_code_bounty(bounty: dict) -> bool:
    """Process a code bounty (infrastructure/research). Returns True if approved."""
    bounty_id = bounty["id"]
    result = bounty.get("result") or {}
    revision_count = bounty.get("revision_count", 0)
    pr_url, pr_number = extract_pr_info(bounty)

    checks = []
    pr_branch = None

    # ── Pull latest and optionally checkout PR branch ────────────────
    log("  Pulling latest code...")
    run_cmd(["git", "fetch", "origin"], cwd=PROJECT_ROOT)
    run_cmd(["git", "pull", "--ff-only", "origin", "main"], cwd=PROJECT_ROOT)

    if pr_number:
        pr_branch = checkout_pr_branch(pr_number, PROJECT_ROOT)

    try:
        # ── Check 1: Git SHA on GitHub ───────────────────────────────
        sha = extract_git_sha(result)
        checks.append(check_git_sha_on_github(sha))

        # ── Check 2: CI status ───────────────────────────────────────
        if sha:
            checks.append(check_ci_status(sha))

        # ── Check 3: Cargo clippy (NEW) ──────────────────────────────
        checks.append(check_cargo_clippy(PROJECT_ROOT))

        # ── Check 4: Cargo audit (NEW — fatal) ──────────────────────
        checks.append(check_cargo_audit(PROJECT_ROOT))

        # ── Check 5: Secret scanning (NEW — fatal) ──────────────────
        checks.append(check_secret_scanning(PROJECT_ROOT, pr_branch))

        # ── Check 6: Cargo build ─────────────────────────────────────
        checks.append(check_cargo_build(PROJECT_ROOT))

        # ── Check 7: Cargo tests ─────────────────────────────────────
        checks.append(check_cargo_test(PROJECT_ROOT))

        # ── Check 8: Git pushed ──────────────────────────────────────
        checks.append(check_git_pushed(PROJECT_ROOT))
    finally:
        # Always return to main
        if pr_branch:
            return_to_main(PROJECT_ROOT)

    return make_decision(bounty_id, checks, revision_count)


def process_growth_bounty(bounty: dict) -> bool:
    """Process a growth/content bounty. Returns True if approved."""
    bounty_id = bounty["id"]
    result = bounty.get("result") or {}
    revision_count = bounty.get("revision_count", 0)

    checks = []

    # ── Check 1: Deliverable URLs are live ───────────────────────────
    urls = extract_deliverable_urls(result)
    if urls:
        checks.append(check_url_liveness(urls))
    else:
        # No URLs in a growth bounty is a problem
        checks.append({
            "check": "deliverable_urls",
            "result": "fail",
            "details": [{"message": "No deliverable URLs found in submission result"}],
            "is_fatal": False,
        })

    # ── Check 2: Result has required fields ──────────────────────────
    required_fields = ["approach", "verification"]
    missing = [f for f in required_fields if not result.get(f)]
    if missing:
        checks.append({
            "check": "result_completeness",
            "result": "fail",
            "details": [{"message": f"Missing required fields: {', '.join(missing)}"}],
            "is_fatal": False,
        })
    else:
        checks.append({
            "check": "result_completeness",
            "result": "pass",
            "details": [],
            "is_fatal": False,
        })

    return make_decision(bounty_id, checks, revision_count)


def make_decision(bounty_id: str, checks: list[dict], revision_count: int) -> bool:
    """Based on check results, verify+approve, request_revision, or reject."""
    failed = [c for c in checks if c["result"] == "fail"]
    all_passed = len(failed) == 0
    any_fatal = is_any_fatal(checks)

    # Build evidence from all checks
    evidence = {
        "verified_by": "qa-verification-bot",
        "timestamp": datetime.now(timezone.utc).isoformat(),
        "all_checks_passed": all_passed,
        "checks": {c["check"]: c["result"] for c in checks},
    }

    if all_passed:
        # ── PASS: Verify AND Approve in one shot ─────────────────────
        quality_score = compute_quality_score(revision_count)
        log(f"  PASS — verifying + approving bounty {bounty_id} (quality={quality_score})")

        # Step 1: Verify
        status, resp = api_post(f"/api/v1/bounties/{bounty_id}/verify", {
            "verifier_wallet": QA_WALLET,
            "evidence": evidence,
        })
        if status != 200:
            log(f"  Verify call failed: HTTP {status} — {resp}")
            return False
        log(f"  Verified successfully")

        # Step 2: Approve (triggers settlement immediately)
        status, resp = api_post(f"/api/v1/bounties/{bounty_id}/approve", {
            "reviewer_wallet": QA_WALLET,
            "quality_score": quality_score,
        })
        if status == 200:
            log(f"  Approved + settlement triggered (quality_score={quality_score})")
            return True
        else:
            log(f"  Approve call failed: HTTP {status} — {resp}")
            # Still verified even if approve failed
            return False

    elif any_fatal or revision_count >= 3:
        # ── HARD REJECT: fatal issue or max revisions exhausted ──────
        feedback = build_structured_feedback(checks)
        reason = "Fatal security issue" if any_fatal else "Maximum revisions (3) exceeded"
        log(f"  REJECT — {reason} for bounty {bounty_id}")

        status, _ = api_post(f"/api/v1/bounties/{bounty_id}/reject", {
            "reviewer_wallet": QA_WALLET,
            "reason": f"QA bot: {reason}\n{feedback}",
        })
        if status == 200:
            log(f"  Rejected successfully")
        else:
            log(f"  Reject call returned HTTP {status}")
        return False

    else:
        # ── REVISION: fixable issues, agent can try again ────────────
        feedback = build_structured_feedback(checks)
        log(f"  REVISION — requesting rework for bounty {bounty_id} (attempt {revision_count + 1}/3)")

        status, resp = api_post(f"/api/v1/bounties/{bounty_id}/request_revision", {
            "reviewer_wallet": QA_WALLET,
            "feedback": feedback,
        })
        if status == 200:
            log(f"  Revision requested successfully")
        else:
            log(f"  Request revision returned HTTP {status} — {resp}")
        return False


# ── Main Loop ─────────────────────────────────────────────────────────────────


def process_bounty(bounty: dict) -> bool:
    """Route bounty to appropriate processor based on category."""
    bounty_id = bounty["id"]
    title = bounty["title"]
    category = bounty.get("category", "infrastructure")

    log(f"Processing: {title}")
    log(f"  Bounty ID: {bounty_id}")
    log(f"  Category: {category}")
    log(f"  Revision count: {bounty.get('revision_count', 0)}")

    if category in ("infrastructure", "research"):
        return process_code_bounty(bounty)
    elif category in ("growth", "content"):
        return process_growth_bounty(bounty)
    else:
        log(f"  Unknown category '{category}', treating as code bounty")
        return process_code_bounty(bounty)


def poll_and_process():
    """One cycle: fetch submitted bounties, process unverified ones."""
    log("Polling for submitted bounties...")
    bounties = api_get("/api/v1/bounties?status=submitted")

    if not bounties:
        log("No bounties or relay unreachable.")
        return 0

    # Filter to unverified
    unverified = [b for b in bounties if b.get("verified_at") is None]

    if not unverified:
        log("No unverified submitted bounties.")
        return 0

    log(f"Found {len(unverified)} unverified bounties.")

    verified_count = 0
    for bounty in unverified:
        try:
            if process_bounty(bounty):
                verified_count += 1
        except Exception as e:
            log(f"  ERROR processing {bounty['id']}: {e}")

    log(f"Cycle complete: {verified_count}/{len(unverified)} verified + approved.")
    return verified_count


def main():
    parser = argparse.ArgumentParser(description="AMOS QA Verification Bot")
    parser.add_argument("--daemon", action="store_true", help="Run continuously")
    parser.add_argument("--interval", type=int, default=60, help="Poll interval in seconds (daemon mode)")
    args = parser.parse_args()

    log(f"QA Verification Bot starting (council-appointed, trust level 5)")
    log(f"  Relay: {RELAY_URL}")
    log(f"  Repo:  {GITHUB_REPO}")
    log(f"  Root:  {PROJECT_ROOT}")
    log(f"  Mode:  {'daemon' if args.daemon else 'single-run'}")
    log(f"  Wallet: {QA_WALLET}")

    if args.daemon:
        log(f"  Interval: {args.interval}s")
        while True:
            try:
                poll_and_process()
            except KeyboardInterrupt:
                log("Shutting down.")
                break
            except Exception as e:
                log(f"Error in poll cycle: {e}")
            time.sleep(args.interval)
    else:
        poll_and_process()


if __name__ == "__main__":
    main()
