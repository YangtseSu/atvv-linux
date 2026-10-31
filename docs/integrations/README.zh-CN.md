# 集成

[English](README.md) | **简体中文**

守护进程拥有 ATVV 会话和常驻 PipeWire 源。其他所有组件都通过两个边界之一
接入：

- **音频**：`Audio/Source` 节点 `atvv-miremote-rc003`，16 kHz 单声道 s16le。
  可使用 fcitx5-vinput、ASR 适配器或录音器。守护进程从不执行 ASR。
- **控制**：会话总线上的 `org.atvv.Daemon`、`atvvctl` 或 `atvv-mcp`。

| 集成 | 文档 | 边界 |
|---|---|---|
| 提供方契约 | [provider-contract](provider-contract.zh-CN.md) | 源元数据 + 录音键 + 通用控制 |
| fcitx5-vinput | [fcitx5-vinput](fcitx5-vinput.zh-CN.md) | 源 + 声明的录音键 |
| 编码 agent | [coding-agents](coding-agents.zh-CN.md) | MCP 工具 + agent 文本传递 |
| 浏览器 AI 聊天 | [browser-ai-chat](browser-ai-chat.zh-CN.md) | `getUserMedia` 设备选择 |
| IM 应用 | [im-applications](im-applications.zh-CN.md) | 应用内选择源 |
| CLI | [`atvvctl`](../../README.zh-CN.md) | `org.atvv.Daemon`、`contract`、`status`、`doctor` |

提供方在源上声明契约（`atvv.contract_version`、`atvv.profile`、
`atvv.recording_keys`）。消费者拥有自己的配置，并应绑定声明的按键；提供方不会
添加按应用的诊断或按应用的控制路径。

消费者链接到源并不会打开麦克风。在 RC003 上，实体语音键控制麦克风数据，因此
守护进程在按键按下时打开 `MIC_OPEN`，在释放时关闭。热的 PipeWire 流只表示
消费者已准备好接收音频。

实时检查是事件驱动且可重复的。守护进程持续运行；你坐在机器前时按下按键即可。
不要构建倒计时或“N 秒内按下”的定时测试。
