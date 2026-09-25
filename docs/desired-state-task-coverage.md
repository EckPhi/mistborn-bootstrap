# Collection-task migration coverage

`desired-state-task-coverage.tsv` is the Phase 5 ledger for the tasks declared
in `plans/server.toml` and `plans/shell.toml`. The companion
`tests/task-coverage.sh` compares the ledger keys against those plans and fails
on an unclassified task, a stale entry, a duplicate, or an unknown disposition.

The classifications mean:

- `migrated`: a typed Rust remediation path exists for the task's behavior;
  `rust_remediations` names the stable IDs that provide that path.
- `retained Bash adapter`: Bash continues to implement the task while any
  missing typed behavior or an explicit external/bootstrap boundary remains.
- `intentionally unsupported`: the task remains in the installer but is not a
  desired-state reconciliation operation, such as interactive credentials or
  tailnet enrollment.

This ledger does not authorize removing a module or generated bundle. Even a
`migrated` task stays available through the collection installer until a
separate contraction change demonstrates state/revision compatibility,
upgrade and rollback behavior, and equivalent task-selection semantics. The
`security/tailscale-only` task remains intentionally unsupported because its
combined UFW changes and public SSH-rule deletion are not equivalent to the
separate Tailscale preference remediations.

The guard also checks that every remediation named by a `migrated` row is
present in both the Rust `RemediationId` string mapping and reconciliation
registry. This is an existence check, not proof of parity: before contraction,
review must still confirm inspector, planner, executor, verifier, and task-level
selection behavior. Tasks requiring multiple remediations remain classified as
retained until a bounded task-level bridge preserves the original scope.

Run the coverage guard with:

```sh
bash tests/task-coverage.sh
```

Update the TSV in the same change that adds, removes, or renames a collection
task. Keep the Rust remediation list aligned with the typed registry and its
inspector, planner, executor, and verifier; a package or service fact alone is
not sufficient evidence of migration.
