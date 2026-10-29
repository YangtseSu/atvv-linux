# ADR 0004 — name and scope

## Status

Accepted, 2026-10-29.

## Context

`miremote` describes one vendor. The stack is ATVV v1.0 on Linux and must be
able to add other ATVV remotes without renaming the project.

## Decision

- Repository: `atvv-linux`
- Daemon: `atvvd`
- CLI: `atvvctl`
- Crates: `atvv-protocol`, `atvv-audio`, `atvv-input`, `atvv-daemon`
- Xiaomi RC003 profile: `miremote-rc003`

## Consequences

- The project name describes the protocol/stack, not one device.
- Xiaomi branding remains in the profile and can be reused by a future
  Xiaomi-specific frontend.
- ATVV v0.4 is out of scope until the plan changes; the project documents
  itself as ATVV v1.0+.
