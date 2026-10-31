# fcitx5-vinput

[English](fcitx5-vinput.md) | **简体中文**

针对 fcitx5-vinput `2.4.2` 测试（上游快照
`xifan2333/fcitx5-vinput@d9a409d`）。集成通过 PipeWire 源和合成语音键完成；
D-Bus 是可选回退。没有链接或复制任何 vinput 代码。

## 架构

```text
RC003 语音键 ──HID──> evdev（已独占）──> atvv-input
                                              ├─ 合成 Dictate ──> fcitx 输入上下文
                                              └─ 语音边沿 ──────> atvvd
ATVV 音频 ──GATT──> atvvd ──> Audio/Source atvv-miremote-rc003 ──> fcitx5-vinput
```

`atvvd` 拥有会话，并在语音键按下时打开 `MIC_OPEN`。fcitx5-vinput 拥有录音、
ASR 和文本提交。源是常驻的，遥控器空闲时输出静音。

ATVV 是提供方；vinput 是消费者（ADR 0007）。集成接口是 PipeWire 源加上标准
`Dictate` 按键。`atvvd` 不写 vinput 配置，也不在触发路径中调用
`org.fcitx.Vinput`。源选择和触发绑定属于 vinput。vinput 也不需要调用 `atvvd`：
源是常驻的，实体按键边沿会打开麦克风；软件启动无法产生帧。

## 设置

1. 在用户会话中运行守护进程：

   ```sh
   systemctl --user start atvvd     # 步骤 09 安装单元之后
   # 或：atvvd --device C0:5D:39:F8:2D:4C --verbose
   ```

2. 确认源存在：

   ```sh
   pw-cli info atvv-miremote-rc003
   atvvctl doctor
   ```

3. 在 vinput 中按名称选择它。守护进程在每次录音开始时重新读取采集设备，因此
   下一次录音会使用它；无需重启守护进程：

   ```sh
   vinput device list
   vinput device use atvv-miremote-rc003
   vinput device list             # atvv-miremote-rc003 标记为活动
   ```

4. 把 ATVV 触发加入 vinput，同时不替换用户的快捷键：

   ```ini
   # ~/.config/fcitx5/conf/vinput.conf
   [TriggerKey]
   0=<用户现有的触发键>
   1=Dictate
   ```

   默认安装的现有条目是 `0=Alt_R`，因此 `Dictate` 是 `1=Dictate`。自定义列表
   使用其下一个空闲索引；不要复制其他机器上的更高索引。如果 `1` 已被占用
   （例如被 `Assistant` 占用），`Dictate` 使用下一个空闲索引；索引是按用户
   独立的。`atvvd` 默认 `trigger_key = "DICTATE"`，发出听写消费者按键。编辑此
   文件前停止 fcitx5：插件会在退出时重写其内存中的配置，因此 fcitx5 运行时所做
   的编辑可能被覆盖。编辑后重启 fcitx5。

   如果 IME 无法绑定听写键，`VOICECOMMAND` 和 `ASSISTANT` 是支持的回退。

   源把声明的按键公布为 `atvv.recording_keys`；上游 vinput 请求是在用户选择
   ATVV 源时自动追加映射后的集合。在该功能落地之前，上述手动条目是受支持的
   设置。见 [`provider-contract.zh-CN.md`](provider-contract.zh-CN.md)。

不要把 ATVV 源设为系统默认设备。fcitx 以 16 kHz 单声道 S16LE 录音，与源速率
一致。

## 触发模式

配置 `$XDG_CONFIG_HOME/atvv/daemon.toml`（或
`~/.config/atvv/daemon.toml`）。`trigger` 是默认值。

```toml
# "trigger": 从被独占的语音键合成 trigger_key（默认）
# "dbus":    调用 org.fcitx.Vinput StartRecording/StopRecording
# "off":     不集成 vinput；atvvctl/MCP 仍可控制会话
vinput = "trigger"

# DICTATE 是听写消费者按键（KEY_DICTATE）。保留用户现有的 vinput
# [TriggerKey] 条目，并在下一个空闲索引添加 Dictate（默认安装：1=Dictate）。
trigger_key = "DICTATE"
```

`trigger_key` 接受单个按键（`DICTATE`、`VOICECOMMAND`、`ASSISTANT`）或 `+`
组合键（`ctrl+rightctrl`）。修饰键按顺序按下，并按相反顺序释放。目标 IME 和
任何合成器快捷键必须使用相同的按键或组合键。

这些名称是分层专用的，不能互换：

| 功能 | evdev / 守护进程 | fcitx5-vinput |
|---|---|---|
| 语音命令 | `VOICECOMMAND`（`KEY_VOICECOMMAND`，`0x246`） | `VoiceCommand` |
| 助手 | `ASSISTANT`（`KEY_ASSISTANT`，`0x247`） | `Assistant` |
| 听写 | `DICTATE`（`KEY_DICTATE`，`0x24a`） | `Dictate` |

`DICTATE` 是默认活动按键；`VOICECOMMAND` 和 `ASSISTANT` 是支持的回退。在消费
者中绑定声明的集合，并发出一个活动按键；不要同时发出三个。

fcitx 原则上接受裸修饰键，但 2026-10-30 从 ATVV 虚拟键盘发出的合成 `Alt_R`
未能触发 fcitx5-vinput 2.4.2。`Control+Control_R` 组合键可以工作。

`vinput`、`trigger_key`、手势时序和按键映射会实时应用：当触发路径变化时，
`ReloadConfig` 或 `atvvctl config set` 只重启输入运行时。`atvvctl config get`
显示活动值。

### 触发键模式（默认）

语音键被独占，因此 F5 无法到达当前应用。按住会发出 `DICTATE` 按下；释放会发出
`DICTATE` 抬起。vinput 插件看到与物理键盘相同的按键路径，并在活动输入上下文中
启动/停止录音。

- 保留用户现有的 vinput 触发，并添加 ATVV 按键：
  ```ini
  # ~/.config/fcitx5/conf/vinput.conf
  [TriggerKey]
  0=<用户现有的触发键>
  1=Dictate
  ```
  默认安装的现有条目是 `0=Alt_R`，因此新条目是 `1=Dictate`；自定义列表使用其
  下一个空闲索引。整段注释掉的 `[TriggerKey]` 表示没有触发。2026-10-30 的真实
  RC003 运行使用 `Dictate` 和 `trigger_key = "DICTATE"`（fcitx `Key(Dictate)`，
  vinput `target.object=atvv-miremote-rc003`，`rate=16000 channels=1 fmt=259`）。
  `Control+Control_R` 组合键配合 `trigger_key = "ctrl+rightctrl"` 此前已经验证。
- 按住说话。RC003 由实体按键门控麦克风数据：仅在按键按住时音频流动。短按会为
  fcitx 产生普通按键（可能切换其录音状态），但遥控器在按键释放后立即停止发送
  音频。在此硬件上，点按切换无法维持录音；请使用按住说话。
- 按下后大约 0.3 秒再说话：`MIC_OPEN` 到达设备，首批帧在 `AUDIO_START` 之后
  到达。
- 如果按住期间设备节点消失（链接丢失），输入运行时会发出触发释放，守护进程
  停止录音。

无需合成器即可验证：聚焦一个使用 fcitx 的编辑器，按住语音键，说话，释放。
`atvvctl status` 必须显示 `streaming` 且 `audio.frames` 增长；fcitx 提交转写
文本。浏览器不得重新加载（不能泄漏 F5）。

### D-Bus 模式（显式回退）

这不是零配置路径；`trigger` 仍然是默认值。仅当触发键与应用绑定冲突，或合成器/
IME 路径看不到虚拟键盘时使用。

```toml
vinput = "dbus"
```

守护进程直接调用 `org.fcitx.Vinput`：

- 语音按下时 `StartRecording()`（无参数，无返回值）。
- 释放时 `StopRecording(s scene_id)`；外部路径传入空场景 id，与 vinput 自己的
  CLI 一致，因为 start 不暴露场景 id。

ATVV 麦克风仍然由语音键边沿打开。如果 `vinput-daemon` 未运行，调用会快速失败
（名称所有权检查），守护进程保持 ATVV 会话，错误会报告在 `GetStatus` 的
`vinput.last_error` 中，并由 `atvvctl doctor` 显示。此模式下不会合成触发键，
因此同一按键不能同时绑定在两个地方。

当触发键与应用绑定冲突，或合成器/IME 路径看不到虚拟键盘时，使用 D-Bus 模式。
外部 `StartRecording` 从插件最后一次按键事件解析输入上下文，因此下一次按键前
仅用鼠标改变焦点可能指向过时上下文；触发键模式没有这个问题。焦点回退
（`mostRecentInputContext`）只是此回退模式的可选上游工作；见
[`docs/research/vinput-upstream-api.md`](../research/vinput-upstream-api.md)。

## 无截止时间的验证

守护进程是事件驱动的。运行它，在你方便坐在机器前时执行检查；没有倒计时，也没
有会错过的窗口。

```sh
atvvd --verbose
vinput device use atvv-miremote-rc003
vinput device list                    # atvv-miremote-rc003 是活动设备
atvvctl monitor                       # 打印语音开始/停止
vinput daemon status                  # 按住按键时显示 "recording"
```

聚焦一个使用 fcitx 的编辑器，按住语音键，说话，释放。按下期间 `atvvctl status`
应显示 `streaming` 且 `audio.frames` 增长。可以按需重复；检查不会过期。

步骤 08 期间使用了自动化版本：mock 传输守护进程加上真实会话服务、聚焦的
`zenity --entry`，以及 `ydotool` 发送配置的触发组合键。vinput-daemon 日志显示
`target.object=atvv-miremote-rc003`，并协商
`rate=16000 channels=1 fmt=259`。详情见
[`docs/research/rc003-hardware-evidence.md`](../research/rc003-hardware-evidence.md)。

触发必须匹配 vinput 的 `[TriggerKey]`
（`~/.config/fcitx5/conf/vinput.conf`）。保留用户现有条目并添加 ATVV 按键；
整段注释掉的 `[TriggerKey]` 会禁用触发。编辑前停止 fcitx5，然后重新启动；
插件会在退出时重写此文件，因此 fcitx5 运行时的编辑可能被覆盖。

```ini
[TriggerKey]
0=Control+Control_R   # 或用户现有的触发键
1=Dictate
```

```toml
# ~/.config/atvv/daemon.toml
trigger_key = "DICTATE"
```

更改后重启 `fcitx5` 和 `atvvd`。`VOICECOMMAND` 和 `ASSISTANT` 是支持的回退；
`Control+Control_R` 组合键配合 `trigger_key = "ctrl+rightctrl"` 是另一条已验证
路径。GNOME 自定义快捷键无法绑定裸修饰键。

## 为什么不从源链接打开麦克风？

fcitx5-vinput 可能保持一个热的、非活动的采集流。消费者存在不等于麦克风请求。
在链接时打开遥控器麦克风会让 RC003 麦克风无限期活动并耗尽电池。守护进程没有
消费者连接回调；`MIC_OPEN`/`MIC_CLOSE` 只来自语音键边沿、`atvvctl mic`、D-Bus
或 MCP。

## 故障排查

| 症状 | 检查 |
|---|---|
| 源缺失 | `atvvctl status`、`pw-cli info atvv-miremote-rc003`、`atvvctl doctor` |
| 设备不在 `vinput device list` 中 | 源必须存在于 PipeWire；守护进程在每次录音时重新读取 `global.capture_device`，因此无需重启守护进程 |
| 触发没有反应 | `atvvctl doctor` 的 `vinput` 和 `uinput` 行；`input = false` 会禁用按键路径。确保 `[TriggerKey]` 包含 `Dictate`（或其他配置的按键）且 `trigger_key` 匹配；注释掉整段会禁用触发。 |
| 录音但静音 | 按住按键；`atvvctl status` 必须显示 `streaming`；按键门控 RC003 帧 |
| F5 仍然泄漏 | RC003 evdev 节点未被独占；检查状态中的 `input.grabbed` |
| `vinput` 行显示 dbus 模式不可用 | 启动 `vinput-daemon` 或切换到 `vinput = "trigger"` |

## 范围之外

ASR 引擎、云凭据和文本注入属于 vinput 或 agent 的职责。守护进程不转写音频。
