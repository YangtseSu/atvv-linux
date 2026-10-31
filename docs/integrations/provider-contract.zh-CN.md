# 提供方契约

[English](provider-contract.md) | **简体中文**

状态：源元数据和通用契约输出已于 2026-10-31 实现；这是当前指南。

## 目标

让 ATVV 易于使用，而无需链接我们的 crate 或阅读源码。遥控器适用于：

- fcitx5-vinput 和其他听写前端；
- 通过 `getUserMedia` 采集麦克风的浏览器页面；
- Claude Code、Codex 和 OMP 等编码 agent；
- Telegram 和 WeChat 等 IM 应用；
- `pw-cat` 等通用录音器。

提供方暴露标准边界。消费者拥有自己的配置。按按键映射和手势时序是本地
`daemon.toml` 配置；它们不属于提供方契约。

```text
RC003 语音键 ──HID──> atvv-input ──uinput 录音键──> 消费者快捷键
ATVV 音频    ──GATT─> atvvd ──PipeWire Audio/Source──────> 消费者录音器
```

`atvvd` 不会选择消费者的源、写它的配置，或在主路径中调用它的守护进程。麦克风
由实体语音键门控；主机侧“启动”无法产生帧。

## 音频契约

- 节点：`Audio/Source`，名称 `atvv-<profile>`（例如 `atvv-miremote-rc003`）。
- 格式：16 kHz、单声道、S16LE。`CAPS_RESP` 是 ATVV 编解码器的权威来源；
  PipeWire 节点始终呈现 16 kHz 单声道。
- 生命周期：源在守护进程生命周期内常驻，遥控器空闲时输出静音。
- 门控：RC003 麦克风仅在实体语音键按住时发送帧。打开 PipeWire 流不会打开
  麦克风。
- 不做系统默认：不要把 ATVV 源设为系统默认设备。在消费者中按名称选择它。

通用检查：

```sh
pw-cli info atvv-miremote-rc003
pw-cat --record --target atvv-miremote-rc003 /tmp/atvv.wav
```

## 录音键契约

提供方声明可用于开始录音的按键。消费者绑定声明的集合；提供方每次按下只发出一个
活动按键。

| 规范名称 | evdev | fcitx5 名称 | 角色 |
|---|---|---:|---|
| `DICTATE` | `KEY_DICTATE` `0x24a` | `Dictate` | 默认活动按键 |
| `VOICECOMMAND` | `KEY_VOICECOMMAND` `0x246` | `VoiceCommand` | 支持的回退 |
| `ASSISTANT` | `KEY_ASSISTANT` `0x247` | `Assistant` | 支持的回退 |

- 声明的集合：上述三个名称，按此顺序。
- 活动按键：默认 `DICTATE`；`trigger_key` 可以为特定消费者选择
  `VOICECOMMAND`、`ASSISTANT` 或原始按键/组合键。
- 绑定规则：把整个声明集合加入消费者的快捷键列表，同时保留用户现有快捷键。
  按下遥控器会发出活动按键；在活动按键改变之前，其他绑定不会生效。
- 不要同时发出三个按键。消费者可能把多余按键解析为组合键或触发重复操作。

### 发现

源通过 PipeWire 属性公布契约：

```text
atvv.contract_version = 1
atvv.profile          = miremote-rc003
atvv.recording_keys   = DICTATE,VOICECOMMAND,ASSISTANT
```

能够读取属性的消费者应在用户选择源时自动绑定声明的按键。无法自动绑定的消费者
必须告知用户把应用快捷键设置为声明的按键之一。提供方不会尝试发现任意应用的
快捷键。

## 控制与状态

- 会话总线上的 `org.atvv.Daemon`：状态 JSON、`StartVoice`、`StopVoice`、
  `ToggleVoice`、`ReloadConfig`、`ListDevices`。
- `atvvctl`：人类可读和 JSON 诊断；没有按应用命令。
- `atvvctl contract [--json]`：消费者在源上发现的提供方契约（配置文件、源、
  媒体类、格式、声明的按键、活动按键、输入/键盘就绪状态、D-Bus/MCP 端点和
  通用配方）。
- `atvvctl status`：完整守护进程视图；其 `contract` 对象是同一份契约数据。
- `atvv-mcp`：面向编码 agent 的 stdio MCP（`status`、`contract`、`voice_*`、
  `list_devices`、`reload`）。MCP 是控制/状态，不是音频传输。

## 消费者配方

### fcitx5-vinput

1. 选择源：`vinput device use atvv-miremote-rc003`。
2. 绑定声明的按键。默认安装的 `[TriggerKey]` 包含 `0=Alt_R`；把 `Dictate`
   追加到下一个空闲索引：

   ```ini
   [TriggerKey]
   0=Alt_R
   1=Dictate
   ```

   自定义列表使用其下一个空闲索引；永远不要硬编码其他配置中的索引。编辑
   `vinput.conf` 前停止 fcitx5，因为插件会在退出时重写该文件。
3. 上游功能请求是：当源声明 `atvv.recording_keys` 时，在
   `vinput device use` 时自动追加映射后的 fcitx 名称，并保留现有条目。实现路径：
   `vinput device use` 已经枚举源，因此可以读取该属性，并使用 fcitx 的
   `org.fcitx.Fcitx.Controller1` 在 `fcitx://config/addon/vinput` 上调用
   `GetConfig`/`SetConfig` 实时追加条目。如果 fcitx 或属性不可用，则打印确切的
   条目。上述手动条目是受支持的回退。

可选的 `vinput = "dbus"` 模式是虚拟键盘无法到达输入法的环境中的回退；它不是
默认值。

### 浏览器 AI 聊天页面

见 [browser-ai-chat.zh-CN.md](browser-ai-chat.zh-CN.md)。

### 编码 agent

使用 `atvv-mcp` 进行状态和会话控制。转写文本通过 IME/听写前端到达当前编辑器；
MCP 不流式传输音频。如果 agent 需要原始音频，可以在用户按住按键时用 `pw-cat`
从 PipeWire 源录制，然后运行自己的 ASR。

### IM 应用

见 [im-applications.zh-CN.md](im-applications.zh-CN.md)。

### 通用录音器

```sh
pw-cat --record --target atvv-miremote-rc003 /tmp/atvv.wav
```

## 诊断

仅通用诊断：

- `atvvctl status` — 守护进程、源、输入和 `contract` 对象。
- `atvvctl doctor` — 提供方侧检查（D-Bus、BlueZ、PipeWire、uinput、配置），
  包括实时源是否公布 `atvv.*` 属性。
- `atvvctl contract [--json]` — 运行中守护进程报告的提供方契约。
- `atvv-mcp contract` — 面向编码 agent 的同一对象。

按消费者的设置属于上述文档。如果消费者看不到源或其快捷键不匹配，修复应属于该
消费者的配置或提供方契约，而不是应用专用分支。

## 版本化

`atvv.contract_version` 从 `1` 开始。允许增加属性；删除或改变已声明按键或源属性
的含义需要 ADR 和版本号提升。
