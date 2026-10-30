# fcitx5-vinput upstream API: zero-config gaps

Status: research snapshot 2026-10-30. This document records the source facts
behind step 10's upstream PR list. It is evidence, not a plan.

- Source: <https://github.com/xifan2333/fcitx5-vinput>
- Pinned snapshot: `d9a409d` (the version tested in steps 08/11)
- Current `main` at review: `bd7dd4365694f11b906948f025f1a653384d37ae`
  (2026-10-30T08:27:18+08:00)
- Installed package: `2.4.2`; fcitx5 `5.1.23`
- License: GPL-3.0. Integrate over D-Bus; do not link, vendor, or copy code.

## 1. Daemon D-Bus surface

`src/common/dbus/dbus_interface.h` and
`src/daemon/runtime/dbus_service.cpp` define:

```text
Bus name:   org.fcitx.Vinput
Object:     /org/fcitx/Vinput
Interface:  org.fcitx.Vinput.Service
Methods:    StartRecording() -> ()
            StartCommandRecording(s) -> ()
            StopRecording(s scene_id) -> s
            CancelOperation(b) -> ()
            GetStatus() -> s
            GetAsrBackendState() -> sssssbbas
            ReloadAsrBackend() -> ()
            StartAdapter(s) -> ()
            StopAdapter(s) -> ()
Signals:    StatusChanged(s), RecognitionResult(s), RecognitionPartial(s)
```

There is no capture-device getter or setter.

## 2. Capture device

- Core config field: `global.capture_device`, serialized as
  `capture_device` in `~/.config/vinput/config.json`
  (`src/common/config/core_config_types.h`, `core_config_json.cpp`).
- `vinput device use <name>` writes the field with `SaveCoreConfig` and prints
  `"Capture device set to '%s'. Restart daemon to apply changes."`
  (`src/cli/config/device_actions.cpp:82-89`).
- The daemon re-reads the config at every `StartRecordingInternal` and calls
  `capture_->SetTargetObject(runtime_settings.global.captureDevice)`
  (`src/daemon/runtime/daemon_runtime_controller.cpp:426-433`). `2.4.2` does
  the same. `SetTargetObject` destroys an idle reusable stream, so the next
  `BeginRecording` reconnects to the new target
  (`src/daemon/audio/audio_capture.cpp`).
- Therefore the "restart daemon" sentence is stale: the next recording already
  uses the new device. The restart is not required by the source.
- `ResolveCaptureTarget` treats `""` and `"default"` as the system default. A
  known source name is used directly. An unknown non-`.monitor` name is still
  passed as `target.object`; there is no fallback to the system default when a
  configured named source is absent
  (`src/common/audio/pipewire_device.cpp:227-264`).
- Implication: a third-party controller cannot select a source without
  editing vinput's config. A D-Bus API needs to distinguish "unset" (`""`)
  from an explicit `"default"` choice, or auto-selection cannot honor the
  user's explicit choice.

## 3. Trigger keys are addon-owned

- `[TriggerKey]` is a `fcitx::KeyList` in `VinputConfig`, loaded from
  `~/.config/fcitx5/conf/vinput.conf`
  (`src/common/config/vinput_config.cpp:106`,
  `src/addon/core/vinput.cpp:274-279`).
- The vinput daemon and `org.fcitx.Vinput` do not own this list. A daemon
  method cannot append to it without a second config source or a write to
  another application's configuration.
- `VinputEngine::setConfig` loads the raw config, calls `applySettings()`
  (which reassigns `trigger_keys_`), and saves
  (`src/addon/core/vinput.cpp:279-310`). It applies live, without a daemon
  restart.
- fcitx5 exposes the official path to that method:
  `org.fcitx.Fcitx5`, `/controller`, `org.fcitx.Fcitx.Controller1`:
  - `GetConfig("fcitx://config/addon/vinput") -> v`
  - `SetConfig("fcitx://config/addon/vinput", v) -> ()`
  - `ReloadAddonConfig("vinput") -> ()`
  These exist in fcitx5 5.1.23
  (`src/modules/dbus/dbusmodule.cpp` in fcitx5). `SetConfig` reaches the
  addon's `setConfig`, so it persists and applies live. The file stays owned by
  fcitx; there is no fcitx-rewrite race.
- ADR 0001 removes the addon's notifier D-Bus object and drives addon input
  state strictly from `StatusChanged`
  (`docs/adr/0001-direct-desktop-notifications-and-error-abolition.md`).
  A new addon-side trigger object would go against that direction; a CLI
  using fcitx's controller would not.

## 4. External start and input-context focus

- External `StartRecording` goes directly to the daemon. The addon learns
  about the session from `StatusChanged` and presents/commits in an input
  context.
- `resolveFrontendInputContext` resolves in this order:
  `session_->ic`, `status_ic_`, the caller's fallback, `last_active_ic_`
  (`src/addon/core/vinput.cpp:332-344`).
- `last_active_ic_` is set on `InputContextCreated` and on every key event,
  but not on focus-in of an already existing context. A mouse-driven focus
  change before the next key press can leave it stale.
- `fcitx::Instance::mostRecentInputContext()` is available in 5.1.23
  (`src/lib/fcitx/instance.h`) and is fcitx's authoritative "most recent
  focused" context. Using it as the fallback fixes external start without
  changing the trigger-key path.
- Implication: D-Bus mode is viable as the zero-config path, but this focus
  fallback should land first. The trigger-key path does not have the problem
  because the key event carries the context.

## 5. Scene id

- `StopRecording(s scene_id)` stores the id in the recognition order
  (`daemon_runtime_controller.cpp:911`).
- `scene::Resolve` returns the named scene, otherwise the configured
  `activeSceneId`, otherwise an empty definition
  (`src/common/scene/postprocess_scene.cpp:111-120`; used by
  `recognition_pipeline.cpp:82`).
- An empty scene id is therefore valid: it means "the active scene". External
  callers (the ATVV daemon, the CLI) pass `""` and get the active scene.
- `StartRecording()` returns nothing; no scene id is exposed. The caller does
  not need one for dictation. Adding a return value is not required; the
  contract should be documented instead.

## 6. Upstream process

- Functional changes follow issue -> draft PR -> single-item commits -> ready
  PR -> squash merge; diffs should stay under 300 lines
  (`.agents/skills/vinput-dev/references/issue-pr-workflow.md`).
- External PRs are not treated as feature requests; open an issue first
  (`docs/agents/issue-tracker.md`).

## 7. PR implications

Required:

1. **Capture-device API**: `GetCaptureDevice() -> s` (raw configured value) and
   `SetCaptureDevice(s) -> ()` (persist + apply to the next recording;
   validate against the source list). The CLI should use it when the daemon is
   running and stop claiming a restart is needed.
2. **Addon focus fallback**: use `instance_->mostRecentInputContext()` before
   `last_active_ic_` in `resolveFrontendInputContext`, so external D-Bus start
   does not depend on a key event.

Optional / deferred:

3. **Trigger-key CLI** via fcitx's `Controller1` `GetConfig`/`SetConfig`.
   Needed only if trigger mode must be zero-config. D-Bus mode needs no
   trigger key.

Do not file:

- `AddTriggerKey`/`SetTriggerKey` on `org.fcitx.Vinput`: the daemon does not
  own the addon's `[TriggerKey]`; it would create a second config source.
- An addon-side trigger D-Bus object: contradicts ADR 0001.
- `StartRecording() -> s`: empty scene id already resolves to the active
  scene; a docs sentence is enough.

## Sources

- <https://github.com/xifan2333/fcitx5-vinput/blob/bd7dd4365694f11b906948f025f1a653384d37ae/src/common/dbus/dbus_interface.h>
- <https://github.com/xifan2333/fcitx5-vinput/blob/bd7dd4365694f11b906948f025f1a653384d37ae/src/cli/config/device_actions.cpp>
- <https://github.com/xifan2333/fcitx5-vinput/blob/bd7dd4365694f11b906948f025f1a653384d37ae/src/daemon/runtime/daemon_runtime_controller.cpp>
- <https://github.com/xifan2333/fcitx5-vinput/blob/bd7dd4365694f11b906948f025f1a653384d37ae/src/common/config/vinput_config.cpp>
- <https://github.com/xifan2333/fcitx5-vinput/blob/bd7dd4365694f11b906948f025f1a653384d37ae/src/addon/core/vinput.cpp>
- <https://github.com/xifan2333/fcitx5-vinput/blob/bd7dd4365694f11b906948f025f1a653384d37ae/docs/adr/0001-direct-desktop-notifications-and-error-abolition.md>
- <https://github.com/fcitx/fcitx5/blob/5.1.23/src/modules/dbus/dbusmodule.cpp>
- <https://github.com/fcitx/fcitx5/blob/5.1.23/src/lib/fcitx/instance.h>
