# Linting and Formatting

All formatting and linting configuration lives in
[`flake.nix`](../flake.nix) -- this is the single source of truth.

## Pre-commit Hooks

Pre-commit hooks run **clippy** and **treefmt** automatically on each commit.
These are managed by the Nix dev shell, so no manual setup is needed beyond:

```sh
nix develop   # or: direnv allow
```

## CI

The same checks are enforced in CI. Commits that fail formatting or linting
will not pass the pipeline.
