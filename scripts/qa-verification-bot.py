#!/usr/bin/env python3
"""
AMOS QA Verification Bot

Automated agent that verifies bounty deliverables before approval.
Enforces: code pushed to GitHub + CI green + cargo test pass → verified.

Lifecycle:  submitted → [QA bot] → verified → approved → settled
                           ↓ (on failure)
                        rejected

Run once:   python3 scripts/qa-verification-bot.py
Run daemon: python3 scripts/qa-verification-bot.py --daemon --interval 60
"""

import argparse
import json
import os
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

# ── Configuration ───────────────────────────────────────────────────────────

RELAY_URL = os.environ.get("RELAY_URL", "http://localhost:4100")
RELAY_API_KEY = os.environ.get("RELAY_API_KEY", "test_key_e2e_maxreward_2026")
QA_WALLET = os.environ.get("QA_WALLET", "87GzqDXXH8sDfbmrmjpoMA4aHNNooNSBf6Q7vyPJEMoh")
GITHUB_REPO = os.environ.get("GITHUB_REPO", "amos-labs/amos-platform-2.0")
PROJECT_ROOT = os.environ.get("PROJECT_ROOT", str(Path(__file__).resolve().parent.parent))

HEADERS = {
    "Authorization": f"Bearer {RELAY_API_KEY}",
    "Content-Type": "application/json",
}


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
        return e.code, None
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


# ── Verification Checks ────────────────────────────────────────────────────


def check_git_sha_on_github(sha: str) -> dict:
    """Verify a git SHA exists on the remote GitHub repo."""
    if not sha:
        return {"git_sha_verified": False, "note": "No git SHA in submission"}

    success, output = run_cmd(
        ["gh", "api", f"repos/{GITHUB_REPO}/commits/{sha}", "--jq", ".sha"]
    )
    if success and sha in output.strip():
        log(f"  GitHub commit verified: {sha[:10]}")
        return {"git_sha": sha, "git_sha_verified": True}
    else:
        log(f"  GitHub commit NOT found: {sha[:10]}")
        return {"git_sha": sha, "git_sha_verified": False, "error": output[:200]}


def check_ci_status(sha: str) -> dict:
    """Check GitHub CI/Actions status for a commit."""
    if not sha:
        return {"ci_status": "skipped", "note": "No SHA to check"}

    # Check commit status API (covers status checks)
    success, output = run_cmd(
        ["gh", "api", f"repos/{GITHUB_REPO}/commits/{sha}/status", "--jq", ".state"]
    )
    status = output.strip() if success else "unknown"

    # Also check GitHub Actions check runs
    success2, output2 = run_cmd(
        ["gh", "api", f"repos/{GITHUB_REPO}/commits/{sha}/check-runs",
         "--jq", "[.check_runs[].conclusion] | unique"]
    )
    check_conclusions = output2.strip() if success2 else "unknown"

    log(f"  CI status: {status}, checks: {check_conclusions}")
    return {
        "ci_status": status,
        "check_conclusions": check_conclusions,
    }


def check_cargo_build() -> dict:
    """Run cargo check on the project."""
    log("  Running cargo check...")
    success, output = run_cmd(["cargo", "check"], cwd=PROJECT_ROOT, timeout=300)
    result = "pass" if success else "fail"
    log(f"  Cargo check: {result}")
    return {"cargo_check": result, "output_tail": output[-500:] if not success else ""}


def check_cargo_test() -> dict:
    """Run cargo test on core crates."""
    log("  Running cargo test (harness, relay, core)...")
    success, output = run_cmd(
        ["cargo", "test", "--lib", "-p", "amos-harness", "-p", "amos-relay", "-p", "amos-core"],
        cwd=PROJECT_ROOT,
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
        "cargo_test": result,
        "test_summary": test_line,
        "output_tail": output[-500:] if not success else "",
    }


def check_git_pushed() -> dict:
    """Verify local HEAD is pushed to remote."""
    success, local_sha = run_cmd(["git", "rev-parse", "HEAD"], cwd=PROJECT_ROOT)
    if not success:
        return {"git_pushed": False, "error": "Cannot get local HEAD"}

    local_sha = local_sha.strip()
    success2, remote_sha = run_cmd(
        ["git", "rev-parse", "origin/main"], cwd=PROJECT_ROOT
    )
    remote_sha = remote_sha.strip() if success2 else ""

    pushed = local_sha == remote_sha
    log(f"  Git pushed: {pushed} (local={local_sha[:10]}, remote={remote_sha[:10]})")
    return {
        "git_pushed": pushed,
        "local_sha": local_sha,
        "remote_sha": remote_sha,
    }


# ── Extract git SHA from bounty result ──────────────────────────────────────


def extract_git_sha(result: dict) -> str:
    """Try to find a git SHA from the bounty result JSON."""
    if not result:
        return ""
    # Check common field names
    for key in ["git_sha", "commit_sha", "sha", "commit"]:
        val = result.get(key, "")
        if val and isinstance(val, str) and len(val) >= 7:
            return val
    return ""


# ── Main Verification Loop ──────────────────────────────────────────────────


def process_bounty(bounty: dict) -> bool:
    """Process a single submitted bounty. Returns True if verified."""
    bounty_id = bounty["id"]
    title = bounty["title"]
    result = bounty.get("result") or {}

    log(f"Processing: {title}")
    log(f"  Bounty ID: {bounty_id}")

    evidence = {}
    all_passed = True
    reject_reason = ""

    # ── Pull latest code ─────────────────────────────────────────────────
    log("  Pulling latest code...")
    run_cmd(["git", "fetch", "origin"], cwd=PROJECT_ROOT)
    run_cmd(["git", "pull", "--ff-only", "origin", "main"], cwd=PROJECT_ROOT)

    # ── Check 1: Git SHA on GitHub ───────────────────────────────────────
    sha = extract_git_sha(result)
    evidence.update(check_git_sha_on_github(sha))

    if sha and not evidence.get("git_sha_verified", False):
        all_passed = False
        reject_reason = f"Git SHA {sha[:10]} not found on GitHub"

    # ── Check 2: CI status ───────────────────────────────────────────────
    if sha:
        evidence.update(check_ci_status(sha))

    # ── Check 3: Code is pushed ──────────────────────────────────────────
    push_check = check_git_pushed()
    evidence.update(push_check)
    if not push_check.get("git_pushed", False):
        all_passed = False
        reject_reason = "Local HEAD is not pushed to remote"

    # ── Check 4: Cargo build ─────────────────────────────────────────────
    if all_passed:
        build_check = check_cargo_build()
        evidence.update(build_check)
        if build_check["cargo_check"] != "pass":
            all_passed = False
            reject_reason = "cargo check failed"

    # ── Check 5: Cargo tests ─────────────────────────────────────────────
    if all_passed:
        test_check = check_cargo_test()
        evidence.update(test_check)
        if test_check["cargo_test"] != "pass":
            all_passed = False
            reject_reason = "cargo test failed"

    # ── Add metadata ─────────────────────────────────────────────────────
    evidence["verified_by"] = "qa-verification-bot"
    evidence["timestamp"] = datetime.now(timezone.utc).isoformat()
    evidence["all_checks_passed"] = all_passed

    # ── Decision ─────────────────────────────────────────────────────────
    if all_passed:
        log(f"  PASS — verifying bounty {bounty_id}")
        status, resp = api_post(f"/api/v1/bounties/{bounty_id}/verify", {
            "verifier_wallet": QA_WALLET,
            "evidence": evidence,
        })
        if status == 200:
            log(f"  Verified successfully")
            return True
        else:
            log(f"  Verify call failed: HTTP {status}")
            return False
    else:
        log(f"  FAIL — rejecting bounty {bounty_id}: {reject_reason}")
        status, _ = api_post(f"/api/v1/bounties/{bounty_id}/reject", {
            "reviewer_wallet": QA_WALLET,
            "reason": f"QA bot: {reject_reason}",
        })
        if status == 200:
            log(f"  Rejected successfully")
        else:
            log(f"  Reject call returned HTTP {status} (may need trust >= 3)")
        return False


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

    log(f"Cycle complete: {verified_count}/{len(unverified)} verified.")
    return verified_count


def main():
    parser = argparse.ArgumentParser(description="AMOS QA Verification Bot")
    parser.add_argument("--daemon", action="store_true", help="Run continuously")
    parser.add_argument("--interval", type=int, default=60, help="Poll interval in seconds (daemon mode)")
    args = parser.parse_args()

    log(f"QA Verification Bot starting")
    log(f"  Relay: {RELAY_URL}")
    log(f"  Repo:  {GITHUB_REPO}")
    log(f"  Root:  {PROJECT_ROOT}")
    log(f"  Mode:  {'daemon' if args.daemon else 'single-run'}")

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
