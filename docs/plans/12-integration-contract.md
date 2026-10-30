# Step 12 — provider contract and consumer integration

Status: 🚧 in-progress
Depends on: 10 (vinput consumer recipe); can start earlier
Touches: `crates/atvv-audio/src/source.rs`, `crates/atvv-cli/`,
`crates/atvv-mcp/`, `docs/integrations/`, `docs/plans/10-vinput-zero-config.md`

## Goal

Make ATVV easy to consume without linking our crates or reading our source.
Consumers include fcitx5-vinput, browser AI chat pages, coding agents (Claude
Code, Codex, OMP), and IM applications (Telegram, WeChat). The provider
exposes a stable audio source, a declared recording-key set, and generic
control/status. Consumers own their configuration. The provider does not add
per-application branches or per-application diagnostics.

## Current state

- The source advertises `atvv.contract_version=1`, `atvv.profile`, and
  `atvv.recording_keys=DICTATE,VOICECOMMAND,ASSISTANT` as PipeWire
  properties. Smoke check:
  `pw-cli info atvv-test` shows all three.
- The provider contract is documented in
  [`../integrations/provider-contract.md`](../integrations/provider-contract.md).
- `atvvctl status`/`doctor` do not yet print the declared keys or the active
  key. There is no `atvvctl contract` command yet.
- `atvv-mcp` exposes `status`/`voice_*` but not the contract.
- Consumer docs cover fcitx5-vinput and coding agents; browser and IM recipes
  are only in the provider-contract document.

## Deliverables

- ✅ Source contract metadata: `atvv.contract_version`, `atvv.profile`, and
  `atvv.recording_keys`; unit test and `pw-cli` smoke check.
- ✅ Provider-contract document with the target-consumer recipes and the
  anti-drift rules.
- ⬜ `atvvctl contract`: print the profile, source name, media class, format,
  declared recording keys, active key, input/keyboard state, D-Bus/MCP
  endpoints, and one consumer recipe line. Human-readable and `--json`.
- ⬜ `atvvctl status`/`doctor`: include the declared keys and active key so a
  generic diagnostic does not need application-specific code.
- ⬜ `atvv-mcp`: add a `contract` tool returning the same JSON, and keep
  `status` backward compatible.
- ⬜ Consumer docs: browser AI chat and IM recipes as first-class documents or
  sections; coding-agent doc updated with the contract and the declared keys.
- ⬜ Tests: contract metadata serialization; `atvvctl contract` output;
  MCP `contract` tool; no writes to another application's configuration.
- ⬜ Real check: `pw-cli info` exposes the contract on the real source; a
  generic `pw-cat` recording works; fcitx5-vinput auto-binds after the upstream
  feature (or the docs path is verified manually).

## Design notes

- The declared set is `DICTATE`, `VOICECOMMAND`, `ASSISTANT` in that order.
  `DICTATE` is the default active key; the other two are fallbacks. `F24` is
  not part of the contract.
- Consumers bind the declared set and preserve the user's own shortcuts. The
  provider emits one active key; never emit all three simultaneously (chord
  parsing and duplicate actions).
- Automatic adaptation to an arbitrary application's shortcut is not possible
  from the provider. The user either sets the consumer shortcut to a declared
  key or sets `trigger_key` to the consumer's shortcut. Document both.
- No per-application diagnostics. No `atvvctl vinput`, WeChat, Telegram,
  Codex, or OMP commands. Generic contract output plus per-consumer docs.
- Contract changes are versioned with `atvv.contract_version`; removing or
  changing a key requires an ADR.

## Verification

```text
cargo test -p atvv-audio
cargo test -p atvv-cli
cargo test -p atvv-mcp
cargo test --workspace
pw-cli info atvv-miremote-rc003
cargo run -q -p atvv-cli -- contract
cargo run -q -p atvv-cli -- status
```

## Exit criteria

- ⬜ `pw-cli info` shows the three contract properties on the resident source.
- ⬜ `atvvctl contract` reports source, format, declared keys, active key, and
  the generic consumer instructions without reading any consumer config.
- ⬜ `atvv-mcp` exposes the same contract to coding agents.
- ⬜ Browser and IM recipes are documented; per-application diagnostics are
  explicitly absent.
- ⬜ The fcitx5-vinput recipe uses the declared keys and the default-install
  index (`1=Dictate`), with the historical `2=` explained.
- ⬜ No provider code writes another application's configuration.

## Risks

- A consumer may not expose source properties through its audio stack (for
  example a sandboxed browser portal). The recipe falls back to selecting the
  source by name.
- A consumer may use a fixed shortcut that is not in the declared set. The
  supported fix is to change the consumer shortcut or `trigger_key`; automatic
  detection is out of scope.
- Adding a custom property is not a standard PipeWire contract; consumers that
  do not understand it must ignore it safely. Keep the value a plain
  comma-separated list and document the version.
- The source is created before the daemon loop; an initial PipeWire failure is
  not retried in-process. That remains a separate reliability gap.

## Progress log

- 2026-10-31 — step created from the consumer-integration design goal. Source
  metadata and the provider-contract document are implemented; generic
  `atvvctl`/MCP contract output, consumer recipes, and real checks remain.
