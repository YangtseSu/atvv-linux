# ADR 0007: ATVV provides interfaces; consumers own their configuration

Status: accepted 2026-10-31

## Context

`atvv-linux` is the lower layer: it owns the ATVV session, the resident
PipeWire `Audio/Source`, and the evdev/uinput mapping of the remote's voice
key. fcitx5-vinput is an upper-layer consumer: it selects a capture source,
binds a trigger key, records, and commits text.

Step 10 originally proposed the opposite direction: `atvvd` would call
`org.fcitx.Vinput.SetCaptureDevice` to make vinput select the ATVV source, and
D-Bus mode would become the zero-config default. That turns a provider into a
controller of one specific consumer and duplicates a policy vinput already
owns.

The RC003 microphone is gated by the physical voice key. A host `MIC_OPEN`
without the key yields `AUDIO_START` and then a frame timeout, so a
vinput-to-ATVV "start recording" call would not produce audio either. The key
edge is the control signal; both the microphone gate and the synthetic
`Dictate` key originate there.

## Decision

1. The public interface is standard:
   - a resident PipeWire `Audio/Source` named `atvv-<profile>`;
   - a uinput voice key, `DICTATE` (`KEY_DICTATE`) by default;
   - the `org.atvv.Daemon` status API, used for diagnostics; the primary path
     needs only the source and the key.
2. The consumer owns its configuration. vinput selects the source and binds
   the trigger key. `atvvd` does not write vinput's `config.json` or
   `vinput.conf`, and it does not call `org.fcitx.Vinput` in the primary path.
3. `vinput = "trigger"` remains the default. `vinput = "dbus"` remains an
   explicit fallback for environments where the virtual keyboard cannot reach
   the input method; it is never the zero-config path.
4. The source declares the recording contract as properties:
   `atvv.contract_version=1`, `atvv.profile=<id>`, and
   `atvv.recording_keys=DICTATE,VOICECOMMAND,ASSISTANT`. Consumers bind the
   declared set and preserve the user's own shortcuts; the provider emits one
   active key. Device-selection policy and trigger binding belong in the
   consumer.
5. Diagnostics are generic, not per-application: `atvvctl status`, `doctor`,
   and a planned `contract` output. There is no `atvvctl vinput` and no
   WeChat/Telegram/Codex/OMP-specific check. Per-consumer recipes live under
   `docs/integrations/`.

## Consequences

- Drop the step-10 deliverables that push `SetCaptureDevice` from `atvvd` and
  the `vinput_device = "auto" | "none"` policy from the ATVV daemon.
- The one-time consumer setup is `vinput device use atvv-miremote-rc003` plus
  `[TriggerKey] 1=Dictate` on a default install (`0=Alt_R`). A customized list
  uses its next free index. That is correct layering, not a workaround.
- True zero-config now depends on a vinput-side auto-selection feature. Until
  it exists, the plan must say "one-time consumer configuration", not
  "zero-config".
- The generic `GetCaptureDevice`/`SetCaptureDevice` API and the addon focus
  fallback are optional for step 10. They are useful control APIs for other
  consumers, but ATVV does not call them in the primary path.
- `AddTriggerKey` on `org.fcitx.Vinput` remains out of scope.

## Alternatives rejected

- **ATVV sets the PipeWire default source during a press.** It would affect
  every application on the system, not just vinput; the plan already forbids
  making the ATVV source the system default.
- **ATVV writes vinput's config.** It overrides a deliberate consumer choice
  and breaks the ownership boundary.
- **vinput calls ATVV to start the mic.** The hardware gates the microphone on
  the physical key; a software start cannot produce frames.
