# Desired-State Reconciliation Plan

## Objective

Move Mistborn host inspection, desired-versus-observed comparison, remediation
planning, and safety policy into the Rust binary without attempting an
all-at-once rewrite of the existing Bash installers.

The resulting command model is:

```text
mistborn status
mistborn doctor
mistborn plan [REMEDIATION]
mistborn reconcile [REMEDIATION]
mistborn doctor --fix [--safe]
```

`status`, `doctor`, and `plan` are always read-only. `reconcile` is the only
primitive that changes host configuration. `doctor --fix` is a convenience
workflow that first produces a plan and then invokes reconciliation under the
same safety rules.

## Invariants

1. Running `status`, `doctor`, or `plan` never changes the host.
2. Every diagnostic that can be repaired has a stable remediation ID, such as
   `security/plex-firewall`.
3. SSH, UFW, Tailscale access, destructive cleanup, and any action that can
   remove remote access always require explicit confirmation, including under
   `--safe` or `--yes`-style automation.
4. `--safe` applies only remediations explicitly classified as low risk. A
   warning or failure is never implicitly considered repaired.
5. Reconciliation follows `inspect -> compare -> plan -> apply -> verify`.
   Applied work is successful only when the post-apply inspection matches the
   desired state.
6. The desired-state configuration, observed state, and migration history are
   separate data models:

   - `/etc/mistborn/config.toml`: what should be true.
   - Inspectors: what is true now.
   - `/var/lib/mistborn-bootstrap/*.json`: what migrations ran and at which
     revisions.
7. Mistborn changes only settings and rules it owns. Unrelated administrator
   UFW rules and configuration remain valid and untouched.
8. Configuration and state writes use a temporary file, `fsync`, validation,
   and atomic rename. Existing readable files are backed up before a schema
   migration.
9. A partial failure records each completed and failed remediation, stops when
   a dependency or access-safety boundary is crossed, and is safe to resume.
10. Existing installer commands and environment-variable inputs remain
    compatible until their bounded area has moved to the Rust engine.

## Terminology and command ownership

The repository already has `mistborn-bootstrap plan`, which compares installer
task revisions and input fingerprints with recorded migration state. Keep that
command during this migration.

`mistborn plan` is different: it compares durable desired configuration with
live host observations and emits remediations. Documentation and help text must
call the former an **installer migration plan** and the latter a **host
reconciliation plan** to avoid ambiguity.

The installed `mistborn` script becomes a thin launcher for Rust subcommands.
Update and Runtipi operations may remain Bash-backed adapters while their
behavior is unchanged.

## Configuration v1

Install `/etc/mistborn/config.toml` with root ownership and mode `0644`. Secrets
must not be stored in this file.

```toml
version = 1
profile = "vps"

[ssh]
port = 22
password_authentication = false
root_login = false

[firewall]
enabled = true
default_incoming = "deny"
public_tcp_ports = [80, 443]

[firewall.plex]
enabled = true
public_remote_access = true
lan_cidr = "192.168.1.0/24"
tailscale = true

[tailscale]
ssh = true
advertise_exit_node = false
auto_update = true
```

All fields are typed and validated before inspection or mutation. Unknown keys
are rejected so misspellings cannot silently weaken policy. CIDRs and ports are
validated without shell parsing. Optional sections distinguish "unmanaged"
from an explicit false value; omission must not accidentally disable an
existing facility.

The initial installer derives this file from its existing environment inputs.
For an existing host, installation must not guess security policy from a
possibly drifted machine. It should either:

- translate known, successfully applied task inputs when they are available;
- write a reviewed migration candidate and leave the area unmanaged; or
- require an explicit operator choice before adopting current values.

## Rust architecture

Add a host-management library below the CLI so presentation is not used as an
API:

```text
Config loader
    -> Domain model
        -> Inspectors -> ObservedState
        -> Comparator -> Vec<Diagnostic> + ReconciliationPlan
        -> Executor   -> Adapter commands
        -> Verifier   -> ObservedState
```

Core types should include:

- `DesiredState`: validated configuration with explicit management scope.
- `ObservedState`: typed facts plus `Available`, `Unavailable`, or `Error`
  inspection status; command-output parsing errors cannot become compliant.
- `Diagnostic`: stable ID, `PASS|WARN|FAIL`, summary, evidence, and optional
  remediation ID.
- `Remediation`: ID, risk class, confirmation policy, dependencies, proposed
  actions, and verification predicate.
- `ReconciliationPlan`: deterministic ordered actions generated from a single
  desired/observed snapshot.
- `ApplyResult`: per-action outcome, post-apply verification, timestamps, and
  enough context for resumption without treating migration history as desired
  state.

Define narrow adapter traits for command execution and filesystem reads. Tests
should supply fake adapters and structured fixtures rather than mock formatted
CLI output.

### Risk and confirmation policy

Use explicit risk classes rather than deriving safety from diagnostic severity:

| Risk class | Examples | Automatic under `--safe` | Confirmation |
| --- | --- | --- | --- |
| Low | install missing package, enable a non-access service | Yes | No |
| Moderate | restart an ordinary service, replace a managed profile | No | Interactive approval |
| Access | SSH policy, UFW policy/rules, Tailscale SSH/routes | Never | Explicit per plan or target |
| Destructive | remove files/rules/data, cleanup stale mounts | Never | Explicit target and warning |

`--safe` must use an allowlist of remediation IDs or action kinds. It must not
mean "anything below a severity threshold." Non-interactive reconciliation of
Access or Destructive work exits with a distinct confirmation-required status.

## Command behavior

### `mistborn status`

Produce a concise operational summary: Mistborn version, config schema,
installed component versions, core service state, and an overall drift count.
It may report a failing exit status but never proposes or applies changes.

### `mistborn doctor`

Run all applicable inspectors and print structured diagnostics with evidence
and remediation IDs:

```text
FAIL  Plex UFW profile missing
      remediation: security/plex-firewall

WARN  fail2ban inactive
      remediation: security/fail2ban

FAIL  SSH password authentication drifted
      remediation: security/ssh
      confirmation required
```

Support a stable machine-readable form, initially `--format json`, so tests and
automation do not parse presentation text.

### `mistborn plan [REMEDIATION]`

Inspect once, show ordered changes, dependencies, risk, confirmation
requirements, and verification checks. It performs no writes, service changes,
package operations, or lock acquisition that alters durable state.

### `mistborn reconcile [REMEDIATION]`

Require root only when the selected adapter actions need it. Re-inspect before
applying to detect a stale plan, acquire a single-host reconciliation lock,
execute in dependency order, verify each bounded remediation, and atomically
record the result. A target such as `security/plex-firewall` must not rerun its
sibling SSH or base-firewall tasks.

### `mistborn doctor --fix [--safe]`

Implement as composition, not a separate repair engine:

1. Run doctor and build the same plan returned by `mistborn plan`.
2. Without `--safe`, display it and request interactive selection/approval.
3. With `--safe`, select only low-risk allowlisted actions.
4. Invoke the ordinary reconciler and print the post-apply verification.

## Delivery phases

### Phase 1: Configuration and domain skeleton

- Add typed configuration loading and validation in Rust.
- Add explicit managed/unmanaged semantics and the v1 schema.
- Add domain types for observations, diagnostics, risks, plans, and results.
- Install a sample/default configuration through the existing toolset module,
  without changing how current Bash tasks execute.
- Preserve the current state schema and installer plan behavior.

Acceptance gate:

- Configuration round-trip and rejection tests cover unknown keys, invalid
  ports/CIDRs, unsupported versions, and omitted/unmanaged sections.
- Existing hosts receive no SSH, UFW, or Tailscale mutation merely from
  installing the new binary or config support.
- Existing Rust, generated-script, status, site, and ShellCheck suites pass.

### Phase 2: Read-only status and doctor

- Move inspection logic from `assets/mistborn` into Rust adapters.
- Implement typed inspectors for packages, systemd, Docker, Runtipi, UFW,
  Plex, fail2ban, SSH, and Tailscale.
- Make `mistborn status` concise and `mistborn doctor` comprehensive.
- Add JSON output and stable remediation identifiers.
- Leave the old Bash inspection functions available for one release as a
  compatibility fallback, then remove them after parity fixtures pass.

Acceptance gate:

- Neither command performs a mutating subprocess or filesystem write.
- Fixtures cover missing commands, permission denial, malformed output,
  inactive services, compliant state, and drift.
- Unrelated UFW rules do not create failures.
- Inspection errors are distinguishable from actual drift.

### Phase 3: Planning and reconciliation framework

- Implement deterministic comparison and dependency ordering.
- Add `mistborn plan`, `mistborn reconcile`, the reconciliation lock, risk
  policy, confirmation handling, structured event log, and verification loop.
- Add `doctor --fix` and `doctor --fix --safe` only after they delegate to the
  same planner and reconciler.
- Keep Bash modules as process adapters invoked by bounded remediation IDs.

Acceptance gate:

- Read-only commands are mutation-proof in adapter tests.
- `--safe` cannot select Access or Destructive actions.
- Non-interactive risky work fails closed without an explicit confirmation.
- A failed verification is recorded as failure, not completion.
- Interrupted and partially successful plans resume without repeating verified
  actions unnecessarily.

### Phase 4: Migrate bounded domains

Move one area at a time, with its inspector, comparator, apply adapter, and
verification delivered together:

1. **UFW/Plex**: own the profile and exact scoped rules; preserve unrelated
   rules. This proves targeted reconciliation with `security/plex-firewall`.
2. **Tailscale**: inspect and reconcile SSH, exit-node advertisement, and
   auto-update separately. Treat connectivity-affecting changes as Access risk.
3. **fail2ban**: package, service enablement, and sshd jail health. Eligible
   low-risk actions may be added to the `--safe` allowlist after tests.
4. **SSH**: render a Mistborn-owned drop-in, validate with `sshd -t`, preserve
   the active session, and require explicit confirmation before reload. This is
   deliberately after the reconciliation framework has proven itself.
5. **Packages and services**: consolidate apt and systemd adapters while
   retaining official external installers as Bash-backed actions.

For each domain, remove its legacy repair path only after Rust parity tests,
rollback instructions, and an upgrade path are present.

### Phase 5: Installer contraction

- Generate the single-command installer around the Rust engine.
- Retain small, auditable shell adapters for bootstrapping the binary and
  invoking official external installers.
- Stop generating the large Bash bundle only after every migrated collection
  task has an equivalent typed remediation and verification path.

## Test and release strategy

Each phase adds unit tests for pure comparison logic, adapter contract tests
with captured fixtures, and CLI integration tests in temporary roots. Security
domains also need negative tests proving that confirmation cannot be bypassed.

Before merging a phase, run:

```text
cargo test
node --test tests/site.test.mjs
bash tests/versioning.sh
bash tests/generated.sh
bash tests/status.sh
shellcheck install.sh assets/mistborn lib/*.sh modules/*.sh tools/*.sh
git diff --check
```

Release phases independently so host upgrades remain bisectable. A config
schema change requires a migration test, a preserved backup, documentation,
website/input updates where applicable, and compatibility with the previous
released schema.

## First implementation slice

The first pull request should stop after Phase 1. It should introduce the
configuration/domain model and installation path, but must not add an apply
path or change the behavior of current security tasks. This creates the durable
contract that later inspectors and reconcilers can target without combining a
new configuration format with live access-control changes.
