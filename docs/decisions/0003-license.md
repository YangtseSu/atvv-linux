# ADR 0003 — GPL-3.0-or-later

## Status

Accepted, 2026-10-29.

## Context

The project is a system component that may incorporate MIT protocol code and
may use Apache-2.0 GUI libraries. The user chose GPL-3.0-or-later.

## Decision

License the project under `GPL-3.0-or-later`.

## Consequences

- MIT, BSD-2-Clause, and Apache-2.0 components can be combined; keep their
  notices.
- GPL-3.0-or-later transitive dependencies are compatible.
- Code under `GPL-3.0-only` (for example
  [remote-mic-app](https://github.com/HD838A/remote-mic-app)) must not be copied
  into this project unless the whole work is relicensed to `GPL-3.0-only`.
  Use its documentation and behavior as reference instead. Source snapshots and
  license boundaries: [`docs/research/reference-projects.md`](../research/reference-projects.md).
- Every source file carries an SPDX identifier.
