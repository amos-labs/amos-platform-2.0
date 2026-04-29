# Security Policy

This document describes the current security posture of the AMOS platform,
the threat model the existing controls cover (and don't cover), the planned
hardening work, and how to report security issues.

This root file is intentionally kept at repository root for GitHub security
disclosure workflows. For the broader STRIDE threat model and system-wide
security analysis, see [docs/features/security.md](docs/features/security.md).

It focuses on the relay's oracle keypair because that key signs on-chain
`submit_bounty_proof` transactions and therefore holds real value. The same
principles apply to any other signing keys that get added later.

## Key management — current state

### Storage model

The relay's oracle keypair is stored as a plaintext JSON file on the
local filesystem in the Solana CLI's standard format (a JSON array of
64 bytes: the 32-byte secret key followed by the 32-byte public key).

The path is configurable via `config.solana.oracle_keypair_path` and is
loaded by `amos-relay/src/solana.rs::SolanaClient::load_oracle_keypair`.

### Enforced controls

On load, the relay performs the following checks:

1. **File permissions (Unix).** The file mode **must** be `0600`
   (owner read/write only). If any group or world bit is set
   (`mode & 0o077 != 0`), the relay **refuses to start** with a
   `Configuration error` pointing at the file and instructing the operator
   to `chmod 600 <path>`. This is a hard error, not a warning — an oracle
   signing key readable by any other local user is considered compromised.

2. **Path hygiene (warning).** If the keypair lives in a location that is
   commonly misused for secret storage, the relay emits a startup warning
   but still loads:

   - Anywhere under `$HOME` (home directories are frequently backed up,
     synced to cloud storage, or shared with other user processes).
   - `/tmp` or `/var/tmp` (world-traversable and periodically swept).
   - Any ancestor directory with the world-writable bit set (`o+w`) —
     if the parent directory is writable by other local users, they can
     swap the keypair wholesale, even if the file itself is `0600`.

   The warning names the specific location and tells the operator how to
   move or tighten it.

### Operator checklist

For production deployments:

- Put the keypair in a dedicated secrets directory such as
  `/etc/amos/secrets/oracle.json`.
- `chown` the directory and file to the relay's service user.
- `chmod 700` the directory, `chmod 600` the file.
- Do **not** check the keypair into source control, container images, or
  configuration-management repositories.

## Threat model

### What the current controls defend against

- **Casual filesystem exposure on a shared host.** A keypair left at
  `0644` is readable by any process running as any user on the machine,
  including sandboxed services that get compromised. The `0600` check
  removes that exposure and makes the misconfiguration loud instead of
  silent.
- **Accidental placement in backup-or-sync paths.** The home-directory
  warning catches the common footgun where an operator drops the key in
  `~/amos-founder.json` and then forgets that `~` is synced to iCloud /
  Dropbox / a user backup.
- **Directory-swap attacks.** The world-writable-ancestor warning catches
  the case where an attacker with write access to a parent directory can
  replace the keypair file without ever having to read the original.

### What the current controls do NOT defend against

These are explicit non-goals of the current posture and will be
addressed in Phase 2 (see below):

- **Root or ring-0 compromise.** A local root exploit, a malicious
  kernel module, or a hypervisor escape can read the keypair regardless
  of file permissions. Filesystem permissions are a last line, not the
  only line.
- **Process memory disclosure.** Once the relay loads the key, it lives
  in the relay process's memory. A memory-disclosure bug (heap
  use-after-free, `/proc/<pid>/mem` read, coredump leak) in the relay
  itself exposes the key.
- **Supply-chain compromise of the relay binary.** If a malicious
  dependency is linked in, it can read the key at load time. Dependency
  pinning and review help but don't eliminate this.
- **Host compromise during an interactive SSH session.** If an attacker
  has an interactive shell as the service user, they can read the key
  directly.
- **Theft of the keypair file while at rest in backups.** Backups of
  `/etc/amos/secrets/` (or wherever the key lives) are not handled by
  the relay. Encrypt backups separately.
- **Multisig / high-value transaction protection.** There is currently
  no second-signer requirement. If the oracle key is stolen, the
  attacker can sign any bounty settlement within the protocol's
  per-bounty caps until the key is rotated on-chain.

## Phase 2 — planned hardening

Tracked as separate future bounties. Nothing in this list is landed yet.

- **KMS-backed signing.** Move the oracle key into AWS KMS (or an
  equivalent HSM-backed key manager) so the relay never sees the raw
  private key — signing happens by calling the KMS API. This removes
  both the filesystem and process-memory exposure paths.
- **Secrets Manager fallback.** For deployments where KMS signing isn't
  practical, load the keypair from AWS Secrets Manager (or Vault) at
  startup into a `secrecy::SecretVec` with aggressive zeroization.
- **Multisig for high-value operations.** For bounty payouts above a
  threshold (tentatively 500 AMOS), require a second signer before the
  on-chain program accepts the settlement. This caps the blast radius
  if the oracle key is stolen.
- **Key rotation runbook.** Documented, tested procedure for rotating
  the oracle key — including the on-chain program update, old-key
  revocation, and relay cutover.
- **Structured audit log.** Emit a structured audit event every time
  the key is used to sign, so that a compromise can be reconstructed
  after the fact.

## Reporting a security issue

**Please do not open a public GitHub issue for security problems.**

If you believe you have found a vulnerability in the AMOS platform:

1. Use GitHub's private security advisory feature on this repository
   ("Security" tab → "Report a vulnerability") to file a private
   report. This gives us a coordinated channel and preserves a
   timestamp.
2. If private advisories are unavailable, email the maintainers at the
   contact address listed in the repository's `Cargo.toml` package
   metadata with subject line starting `[AMOS SECURITY]`.

We will acknowledge within 72 hours and aim to confirm, mitigate, and
coordinate a public disclosure within 30 days of the initial report.
Please include:

- A description of the issue and its impact.
- Steps to reproduce, ideally with a proof-of-concept.
- Your preferred credit line (or a note that you'd prefer to remain
  anonymous).

Do not perform testing that would disrupt the production relay, exfiltrate
real user data, or drain tokens from the treasury — use devnet or a
local development harness.
