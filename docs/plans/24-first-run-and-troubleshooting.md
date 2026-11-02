# Step 24 — First-run checklist and recovery docs

Status: ⬜ not-started
Depends on: 12, 14, 19, 23
Touches: `crates/atvv-gui/`, `README.md`, `README.zh-CN.md`, `docs/`

## Goal

Reduce first-run and recovery friction without adding per-application
detection or consumer configuration:

1. a generic **Getting started** checklist in `atvv-gui` that shows which layer
   is missing: daemon, device, source, or input/trigger;
2. a symptom-based troubleshooting page in English and Simplified Chinese;
3. a short **Known limitations** section in both READMEs.

The checklist is read-only and derived from existing `GetStatus`, `GetConfig`,
and `contract` data. It does not start services, open Bluetooth, write
`daemon.toml`, or inspect another application's configuration.

## Evidence

- The GUI currently has a daemon-unavailable page and a device-information
  dialog, but no guided sequence that tells a new user which layer is missing
  (`crates/atvv-gui/src/app.rs`, `docs/gui.md`).
- The README has a troubleshooting table, but no single page with the exact
  commands, expected output, and log signatures for the common failures
  (`README.md`).
- The provider contract already exposes the source name, format, recording
  keys, active key, input readiness, and the consumer recipe
  (`crates/atvv-daemon/src/status.rs`,
  `docs/integrations/provider-contract.md`).
- `docs/research/sayall-linux-comparison.md` §3.4 and §3.5 record the
  behavior-only reference: a live four-step checklist and a symptom-based
  troubleshooting page. Do not copy its GPL-3.0-only code or Vokie-specific
  checks.

## Design

### Getting started checklist

- Add **Getting started…** to the header menu. It opens an `AdwDialog` that can
  be reopened at any time.
- Auto-open the dialog once at startup only when the loaded document has no
  `active_device` (no remote configured yet). If the daemon or config cannot
  be loaded, keep the existing unavailable state and do not auto-open. A
  configured but disconnected device does not auto-open; the menu item is
  still available.
- Four check rows, each with a state dot, a short explanation, and a copyable
  command or link:
  1. **Daemon** — `GetStatus` succeeded. Else show
     `systemctl --user status atvvd` and `atvvctl doctor`.
  2. **Device** — `active_device` is set, `status.device.address` matches it,
     and `status.connected` is true. Else show `atvvctl pair <MAC>`.
  3. **Source** — `status.audio.available` is true and
     `status.contract.source` is non-empty. Else show `atvvctl status` and
     `pw-cli info atvv-<profile>`.
  4. **Input and trigger** — `status.input.enabled`,
     `status.input.keyboard`, and `status.contract.active_key` is `Some`.
     Else show `atvvctl status` and the `uinput` hint.
- Below the checks, add one informational row: **Consumer**. Show
  `status.contract.recipe` and a link to `docs/integrations/` on GitHub. Do
  not mark it green by guessing whether a consumer is configured; the provider
  cannot know that.
- Refresh the checklist every 1 s while the dialog is visible; stop the timer
  when it closes. Do not add a background timer.
- Keep it read-only: no `systemctl` calls, no Bluetooth calls, no config
  writes, no per-application detection. Copy buttons only copy the command.
- Put the row-state computation in a small pure function with unit tests so
  the UI test does not have to infer it from widgets.
- Add all labels and explanations to `crates/atvv-gui/src/i18n.rs` in English
  and Simplified Chinese.

### Troubleshooting page

- Add `docs/troubleshooting.md` and `docs/troubleshooting.zh-CN.md`; each pair
  links to the other at the top. English stays authoritative.
- Structure each section as: symptom, checks in order, exact commands,
  expected output or log signature, and the next step. Cover:
  - daemon not running or D-Bus name not activatable;
  - no device address or device disconnected;
  - source missing from `pw-cli`/consumer;
  - source present but the consumer does not record (selection, trigger key,
    vinput warm stream);
  - trigger key conflict or F5 reaching the focused application;
  - `/dev/uinput` permission or keyboard grab failure;
  - after a PipeWire service restart;
  - remote reconnect and stale `last known` facts;
  - no level or clipping; how to set gain with the new control from step 23;
  - where the logs are: `journalctl --user -u atvvd`, `atvvctl monitor`,
    `atvvctl doctor`, `atvvctl status`, `atvvctl contract --json`.
- Link to the integration guides and the hardware evidence instead of copying
  their content. Do not add per-application diagnostics.
- Add a **Known limitations** section to `README.md` and
  `README.zh-CN.md` with links:
  - RC003 microphone data is gated by the physical voice key;
  - allow roughly 0.3 s after pressing before speaking (until a measured
    keep-open result changes it; see B03);
  - fcitx5-vinput auto-selection and declared-key binding are upstream-owned
    and still pending; manual configuration is documented;
  - the Chromecast A0 Linux run, HID press map, and two-device switch remain
    pending as recorded in the handoff;
  - ATVV does not perform ASR; the consumer owns recognition and text commit;
  - default gain is 0 dB and clipping is possible; set gain with the GUI or
    `atvvctl config set --gain-db`.
- Link the README troubleshooting row to the new page and add the page to
  `docs/integrations/README.md` as the recovery entry point.

## Deliverables

- ⬜ Startup checklist data function with four checks plus the consumer
  informational row; unit tests for every combination.
- ⬜ **Getting started…** menu item and dialog with a 1 s refresh only while
  visible; auto-open only when `active_device` is absent.
- ⬜ Copyable commands for daemon, pair, source, and input/trigger failures;
  no service, Bluetooth, or config side effects.
- ⬜ English and Simplified Chinese UI strings for the dialog.
- ⬜ `docs/troubleshooting.md` and `.zh-CN.md` with the symptom/command/log
  sections above; bilingual links.
- ⬜ README and `.zh-CN.md` **Known limitations** sections with evidence
  links.
- ⬜ `docs/integrations/README.md` and `.zh-CN.md` link to the troubleshooting
  page.
- ⬜ `docs/research/sayall-linux-comparison.md` §6 links this step.

## Verification

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
xvfb-run -a cargo test -p atvv-gui --test ui
reuse lint
```

Manual smoke (no hardware required):

```text
dbus-run-session -- bash -c '
  cargo run -q -p atvv-daemon -- --mock &
  sleep 1
  cargo run -q -p atvv-gui
'
```

- With an empty config, the checklist auto-opens and marks the missing device;
- with a configured mock device, it does not auto-open;
- the menu item reopens it;
- stopping the mock daemon makes the daemon check fail on the next refresh;
- copy buttons place the exact commands on the clipboard.

Run the commands shown in the troubleshooting page on this host, with the
installed daemon, and record the observed output. Do not claim a hardware run
that was not performed.

Check relative links with a one-off script (the same check used for the
research docs); do not commit the script.

## Exit criteria

- ⬜ The checklist has four independently computed checks and one consumer
  informational row; no per-application detection.
- ⬜ Auto-open happens only when no device is configured; the dialog is always
  reachable from the menu.
- ⬜ The checklist is read-only: no `systemctl`, Bluetooth, or config writes.
- ⬜ The GUI strings exist in English and Simplified Chinese.
- ⬜ `docs/troubleshooting.md` and its Chinese sibling exist, link to each
  other, and are linked from the README and the integrations index.
- ⬜ The README Known limitations sections are present in both languages and
  point at the evidence.
- ⬜ The troubleshooting commands were run and their observed output is
  recorded in the step or a linked evidence file.
- ⬜ `reuse lint` and the relative-link check pass.

## Risks

- A checklist that guesses consumer state would become per-application
  diagnostics. Keep the consumer row informational.
- Auto-opening on every launch with no device can annoy a user who is only
  inspecting the GUI. The condition is narrow (no `active_device`) and the
  dialog is dismissible; do not add persistence until it is a measured
  problem.
- Copying commands is not the same as running them. Do not add action buttons
  that start services; that would make the GUI a second control plane.
- The troubleshooting page can drift from the integration docs. Link to them
  and keep only the generic recovery path here.
- A known-limitations section can turn into a second device-support matrix.
  Keep it short and link to the evidence.

## Progress log

- 2026-11-02 — plan created from
  `docs/research/sayall-linux-comparison.md` §3.4, §3.5, and §6.

## Next action

Implement the pure checklist state function and its unit tests, then the
**Getting started…** dialog with the 1 s refresh and the auto-open condition.
The bilingual troubleshooting page and README sections follow.
