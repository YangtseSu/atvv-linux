# 编码 agent

[English](coding-agents.md) | **简体中文**

ATVV 栈为编码 agent 提供两样东西，并明确不提供一样：

- **控制**：`atvv-mcp` 通过 stdio MCP 暴露守护进程状态和语音命令。
- **文本**：fcitx5-vinput 把转写结果提交到当前应用。
- **不提供**：通过 MCP 流式传输音频或转写文本。MCP 是请求/响应模式。除非所选
  ASR 后端上传音频，否则音频不会离开本机。

## MCP 服务器

`atvv-mcp` 在 stdio 上使用换行分隔的 JSON-RPC 2.0，支持 `2024-11-05` 到
`2025-11-25` 的握手版本（`initialize`、`tools/list`、`tools/call`）。它延迟
连接 `org.atvv.Daemon`，因此可以在守护进程之前启动。

向 agent 注册该二进制：

```sh
claude mcp add atvv -- /usr/bin/atvv-mcp
```

或在 MCP JSON 配置中：

```json
{
  "mcpServers": {
    "atvv": {
      "command": "/usr/bin/atvv-mcp",
      "args": []
    }
  }
}
```

### 工具

| 工具 | 参数 | 结果 |
|---|---|---|
| `status` | 无 | 守护进程状态 JSON（`state`、`connected`、`streaming`、`device`、`caps`、`audio`、`input`、`vinput`、`contract`、`config`、`last_error`） |
| `contract` | 无 | 提供方契约 JSON（配置文件、源、格式、声明的录音键、活动按键、输入/键盘状态、端点、配方） |
| `voice_start` | 无 | 打开遥控器麦克风（`MIC_OPEN`） |
| `voice_stop` | 无 | 关闭遥控器麦克风（`MIC_CLOSE`） |
| `voice_toggle` | 无 | 空闲时启动，打开中/流式传输时停止 |
| `list_devices` | 无 | 已配置的设备地址 |
| `reload` | 无 | 重新读取 `daemon.toml`；实时应用增益、输入、手势时序、触发键、vinput 模式和按按键映射 |

后端失败会以 `isError: true` 和错误文本返回，因此 agent 可以报告“守护进程未
运行”，而不是挂起。未知工具返回 JSON-RPC `-32602` 错误。

### 契约

当 agent 需要了解采集接口时，在启动时调用一次 `contract`。它返回与
`atvvctl contract --json` 相同的对象：

```json
{
  "version": "1",
  "profile": "miremote-rc003",
  "source": "atvv-miremote-rc003",
  "media_class": "Audio/Source",
  "sample_format": "s16le",
  "sample_rate": 16000,
  "channels": 1,
  "recording_keys": ["DICTATE", "VOICECOMMAND", "ASSISTANT"],
  "active_key": "DICTATE",
  "input_ready": true,
  "keyboard_ready": true,
  "dbus": {
    "name": "org.atvv.Daemon",
    "path": "/org/atvv/Daemon",
    "interface": "org.atvv.Daemon"
  },
  "mcp": {"command": "atvv-mcp", "transport": "stdio"},
  "recipe": "Select atvv-miremote-rc003 as the capture source; bind DICTATE, VOICECOMMAND, ASSISTANT or set the consumer shortcut to the active key."
}
```

`active_key` 是触发模式下配置的按键。`input_ready` 和 `keyboard_ready` 报告它
当前是否可以发出；遥控器断开时按键仍然配置，但当前无法发出。agent 不需要选择源
或绑定按键；fcitx5-vinput 负责这些。契约用于发现和诊断。

在 RC003 上，`voice_start` 会打开会话，但实体语音键仍然门控音频帧。免提操作请
使用 fcitx5-vinput 的触发模式；MCP 用于状态和编排，不用于采集语音。

## 传递文本

转写文本由 fcitx5-vinput 生成，并通过活动输入上下文提交。这是主要路径：对着
当前编辑器或终端说话，agent 收到的是普通文本输入。

当 agent 必须把文本推送到另一个终端时，优先使用终端自己的控制通道，而不是合成
键盘输入：

| 界面 | 命令 |
|---|---|
| tmux | `tmux send-keys -t <session>:<window>.<pane> -- "text" Enter` |
| WezTerm | `wezterm cli send-text --pane-id <id> -- "text"` |
| kitty | `kitty @ send-text --match id:<id> -- "text"` |
| 任何有剪贴板的界面 | `wl-copy <<< "text"`，然后由用户粘贴 |

准则：

- 指定具体的窗格/窗口；不要依赖焦点。
- 对于代码、引号和非 ASCII 文本，优先粘贴而不是逐字符输入。`send-keys` 会解释
  某些参数；使用 `--` 和单个带引号的参数。
- 不要把 `wtype` 作为核心路径。它依赖合成器对虚拟键盘协议的支持，会注入当前
  焦点，并可能输入到错误的窗口。仅将其保留为可选回退。
- 未经用户明确请求，绝不要把机密粘贴到窗格或剪贴板。

## 示例流程

```text
1. 用户按住 RC003 语音键并说：
   "run the workspace tests and summarize failures"
2. fcitx5-vinput 从 atvv-miremote-rc003 录制、转写，并把文本提交到 agent 的
   终端。
3. agent 运行请求的命令。
4. agent 可以调用 tools/call status，在请求下一次听写前确认遥控器链接仍然
   就绪。
```

MCP `status` 也可用于诊断：`vinput.mode` 区分触发和 D-Bus 集成，
`input.keyboard` 报告虚拟键盘是否可用，`audio.frames`/`audio.samples` 证明解码
后的帧已到达源。
