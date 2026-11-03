# 配置

[English](configuration.md) | **简体中文**

`daemon.toml` 是 `atvvd` 的唯一配置文件。它由 `atvv-config` crate 解析和校验；
守护进程、`atvvctl` 和 GUI 都不会再实现第二个解析器。GUI（`atvv-config`，见
[gui.zh-CN.md](gui.zh-CN.md)）通过 `GetConfig`/`SetConfig` 编辑文档，从不直接写
这个文件。文件是可选的：缺失时使用设备配置文件默认值。

位置：`$XDG_CONFIG_HOME/atvv/daemon.toml`，或 `~/.config/atvv/daemon.toml`。

## 模式

```toml
schema_version = 3
active_device = "C0:5D:39:F8:2D:4C"  # 由 `atvvctl pair` 写入
adapter = "hci0"                    # 可选的 BlueZ 适配器
gain_db = 0                         # -24..=24
input = true                        # evdev/uinput 路径
preopen_mic = false                 # 就绪后保持麦克风会话打开
vinput = "trigger"                  # trigger | dbus | off
trigger_key = "DICTATE"             # 按键或组合键；none 表示禁用
allow_exec = false                  # 启用 exec: 操作

[devices."C0:5D:39:F8:2D:4C"]
label = "客厅"                      # 可选
profile = "miremote-rc003"
layout = "editor"                   # default 或某个用户布局 id

[devices."74:46:B3:22:2D:D5"]
profile = "google-chromecast"
layout = "default"

[recording]
press = "key:KEY_F13"               # 可选的边缘操作；none 表示禁用
release = "none"

[layouts.editor]
name = "Editor"
description = "编辑器和浏览器快捷键"

[layouts.editor.gesture]
long_press_ms = 700

[layouts.editor.buttons.up]
single = "key:KEY_F13"

[layouts.editor.buttons.ok]
long = "combo:ctrl+alt+v"
```

| 字段 | 默认值 | 说明 |
|---|---|---|
| `schema_version` | `3` | 版本 1 和 2 在加载时迁移；更高版本会被拒绝。 |
| `active_device` | 无 | 活动设备的蓝牙地址；`atvvctl pair` 会写入并设为活动设备。 |
| `[devices.<address>]` | 无 | 已知设备：可选 `label`、必填 `profile` 和 `layout`。地址会规范为大写。 |
| `adapter` | BlueZ 默认 | 适配器名称，例如 `hci0`。 |
| `gain_db` | 设备配置文件值 | 限制在 ±24 dB；没有 AGC。`atvv-config` 的**菜单 → 麦克风…**显示实时电平和增益。 |
| `input` | `true` | 为 false 时禁用发现、独占和 uinput。 |
| `preopen_mic` | `false` | 在 `ready` 时打开 ATVV 麦克风并保持待命，停止后重新打开。消除设备的首帧启动延迟，代价是常驻 BLE 会话。见[预打开麦克风](#预打开麦克风)。 |
| `vinput` | `trigger` | `trigger` 合成 `trigger_key`；`dbus` 调用 `org.fcitx.Vinput`；`off` 禁用集成。触发键与应用快捷键冲突时，`dbus` 是显式回退。修改 vinput 的 `[TriggerKey]` 后必须完全重启 fcitx5，`fcitx5-remote -r` 不够。 |
| `trigger_key` | `DICTATE` | 单个按键或组合键，例如 `F18` 或 `ctrl+rightctrl`。`none` 禁用合成。 |
| `allow_exec` | `false` | 任何 `exec:` 操作都需要它。 |
| `[recording]` | 无 | 录音键按下和松开边缘的可选 `press`、`release` 操作。语法与按键操作相同，但不接受 `dictation:*`；见[录音键操作](#录音键操作)。 |
| `[layouts.<id>]` | 无 | 用户布局：`name`、可选的 `description`、`[gesture]` 和 `[buttons.<button>]`。 |

只有活动设备拥有 GATT 会话、PipeWire 源、HID 独占和映射板。
`atvvctl device use <address>` 无需重启守护进程即可切换已知设备；旧设备会先断开、
释放独占，再连接新设备。每个设备的布局保存在其设备条目中，切换时会重新加载该
设备的活动布局。已知设备在守护进程重启后仍然存在。
`atvvctl device rename <address> <label>` 设置或清除可选标签；空标签表示清除。

可映射按键是活动设备的实体按键。每个配置文件都有 `power`、`ok`、`back`、
`up`、`down`、`left`、`right`、`home`、`tv`、`volume_up` 和 `volume_down`；
RC003 另有 `menu`，Chromecast 另有 `mute`、`youtube` 和 `netflix`。映射板只显示
活动设备的按键。语音键不可映射：它控制 ATVV 麦克风和 vinput 触发。
`google-chromecast` 配置文件的 OK 是 `KEY_SELECT`，主页是 `KEY_HOMEPAGE`，
Input 是 `KEY_TV`，静音是 `KEY_MUTE`，YouTube 和 Netflix 使用摄像头访问按键。
助手键直接发送 ATVV `AUDIO_START`，守护进程从会话开始/停止合成配置的触发键。

槽位是 `single`、`double` 和 `long`。缺失的槽位表示“使用设备配置文件默认值”，
而不是“未映射”。默认映射会为配置文件拥有的每个按键重新发出其物理主按键，
`power` 除外；它默认为 `none`，以免与 logind 竞争。

## 布局

布局是一组命名的按键操作，加上手势时序覆盖。设备配置文件仍然是硬件/固件标识；
布局是用户的按键映射。`default` 由设备配置文件合成，从不存储，并且是**模板**：
在映射板上编辑它后点击“应用”，会把更改另存为新的用户布局，模板本身保持不变。

布局必须有名称（不区分大小写地唯一）、可选描述、可选的按键覆盖和可选手势覆盖。
布局 id 匹配 `[a-z0-9][a-z0-9_-]{0,31}`；`default` 以及预设 id `mpv`、`slides`、
`omp` 为保留 id。最多允许 32 个用户布局；名称 1–40 个字符，描述最多 120 个字符。

全局字段（`adapter`、`gain_db`、`input`、`preopen_mic`、`vinput`、
`trigger_key`、`allow_exec`、`recording`）不属于布局。切换布局永远不会改变 exec
策略、活动设备或录音键。`default` 不能被存储、重命名、删除或重置，也不会出现在
`layouts` 中。切换布局时会同步更新活动设备的 `layout` 条目。

### 预打开麦克风

默认情况下，守护进程在按下语音键时发送 `MIC_OPEN`。设置
`preopen_mic = true` 后，它在设备进入 `ready` 时发送 `MIC_OPEN`，每 10 秒发送
`MIC_EXTEND` 保持会话，并在设备停止会话约 200 ms 后重新打开。该会话处于待命
状态而非录音：`atvvctl status` 保持 `state = "ready"`、`streaming = false`，
同时 `mic_armed = true`；消费者只在语音键边沿收到
`VoiceStarted`/`VoiceStopped`。帧超时不会关闭空闲的待命会话。

RC003 仍由实体按键门控麦克风数据，因此这不是常开麦克风。代价是常驻 BLE 会话和
每次说话一次重新打开，换取更低的首帧延迟。2026-11-03 在 RC003
（`C0:5D:39:F8:2D:4C`）上测得：从按下语音键到首个解码帧的中位数从 **157 ms**
降到输入边沿到达时首帧已经排队（中位数改善 **169 ms**）。这是守护进程侧的延迟，
不是消费者端的端到端延迟。默认值为 `false`；见
[硬件证据](research/rc003-hardware-evidence.md#keep-open-mic-ab-2026-11-03)。

对于语音键只发送 `AUDIO_START` 的设备（`google-chromecast` 配置文件），该设置
不生效：预打开会抑制合成其消费者触发键的会话边沿。

手势覆盖作用于整个活动布局，因此切换布局可能改变双击/长按行为。
`atvvctl layout show` 会打印实际生效的时序。

### 预设

三个内置预设是用户布局的只读模板：

| id | 名称 | 验证版本 | 用途 |
|---|---|---|---|
| `mpv` | mpv | mpv 0.41.0 | Space 暂停，`f` 切换全屏，方向键跳转，`m` 静音，`q` 退出 |
| `slides` | Slides | LibreOffice Impress 26.8.1、WPS Office 12.1.2 | PageUp/PageDown 翻页，B 黑屏，F5 开始放映，Esc 结束 |
| `omp` | omp | omp 18.14.2 | Enter 提交，Esc 中断，Ctrl+R 搜索历史，Ctrl+O 展开工具输出，Shift+Tab 切换思考等级，Alt+R 重试 |

每个预设都在单次按下槽位映射其声明的按键，不含 `none`、`exec:` 或
`dictation:`。在 RC003 上是全部十二个可操作按键；把预设应用到缺少某个按键的
设备（例如没有 `menu` 的 Chromecast）时会丢弃该槽位。双击和长按保持设备配置
文件默认值。映射按各应用的默认键位验证；应用内自定义（mpv 的 `input.conf`、
omp 的 `~/.omp/agent/keybindings.yml`、omp 的 Vim 模式）可能改变这些按键。

预设不是活动状态。应用预设会创建一个用户布局；除非传入 `--activate`，活动布局
保持不变，`default` 永远不会被修改。预设 id 是保留 id，因此用默认名称应用 `mpv`
会创建 id 为 `mpv-layout` 的布局。用
`layout reset <id> --preset <预设>` 把用户布局重置为预设，布局名称和描述保持不变。
GUI 通过“从预设新建…”和“重置为预设…”提供同一目录。ATVV 不会检测前台应用或自动
切换布局；生成的布局可以像其他布局一样编辑。

### 迁移

包含顶层 `[buttons]` 或 `[gesture]` 表的版本 1 文件在加载时迁移为名为 "Custom"
的用户布局 `custom`；没有覆盖的版本 1 文件使用 `default` 模板。版本 2 文件不得
包含顶层表。下一次保存会写入版本 3：单个 `device` 字段变成带文件和布局信息的
`[devices.<address>]` 条目，`active_device` 指向它，旧的顶层 `profile`、`device`
和 `active_layout` 键被删除。版本 3 文件完整控制设备注册表。`SetConfig` 只接受
版本 3，因此过期的客户端会明确失败，而不是悄悄丢弃布局。

## 操作语法

```text
none
key:<name>
combo:<mod>+<mod>+<name>
exec:<command>
dictation:start|stop|toggle
pointer:click:left|right|middle|side|extra|forward|back|task
pointer:move:up|down|left|right:<units>
```

按键名称使用与输入运行时相同的解析器：内核名称（`KEY_F13`、截屏键
`KEY_SYSRQ`）、别名（`ctrl`、`alt`、`super`、`enter`、`esc`、`dictate`、
`printscreen`）以及合成的 `KEY_<NAME>` 形式都可接受。`print` 是旧的
`KEY_PRINT`，不是截屏键；`KEY_SYSRQ` 也可写作 `sysrq` 或 `prtsc`。组合键先按下
修饰键，再按相反顺序释放。一个按键在组合键中只能出现一次。

`none` 会吞掉按键。`dictation:start|stop|toggle` 只驱动 ATVV 会话，不发出按键。

指针操作使用独立的 `atvv-virtual-pointer` uinput 设备，具备 `REL_X`/`REL_Y`
和鼠标按键。`pointer:click:<button>` 按下并释放一个鼠标键；
`pointer:move:<direction>:<units>` 按 1–255 个相对计数移动，向上和向左为负值。
只有映射中包含指针操作时才会创建该设备。相对计数不是屏幕像素：桌面的指针加速
和速度设置会生效。原始 `BTN_*` 名称会被拒绝；请使用
`pointer:click:<button>`，让按键由指针设备发出，而不是键盘设备。

一个方向键可以承载完整的指针手势：

```toml
[buttons.up]
single = "pointer:move:up:1"
double = "pointer:move:up:10"
long = "pointer:move:up:50"
```

鼠标左键、右键、中键、侧键、扩展键、前进键、后退键和任务键都可以这样映射：

```toml
[buttons.ok]
single = "pointer:click:left"

[buttons.menu]
single = "pointer:click:right"
```

校验会拒绝未知按键、语音键、未知按键名称、组合键中重复的按键、格式错误的操作、
超出 1–255 的指针单位、原始 `BTN_*` 按键、超出范围的增益或手势值，以及
`allow_exec = false` 时的 `exec:` 操作。错误信息会指出按键、槽位和值。

## 录音键操作

实体录音键（RC003 上是 `KEY_F5`）不属于布局。它的按下和松开边缘打开、关闭 ATVV
麦克风，并可在同一边缘运行两个可选操作：

```toml
[recording]
press = "key:KEY_F13"
release = "none"
```

操作语法与按键操作相同；`dictation:*` 会被拒绝，因为该键本身已经负责开始和停止
听写。`none`、省略字段或整个 `[recording]` 表缺失都表示禁用该边缘。

- 麦克风边缘先执行，操作随后执行。按下操作每次按下运行一次，松开操作每次松开
  运行一次。
- `key`、`combo` 和 `pointer` 操作是点按：按键或单击立即按下并释放。按住录音键
  期间不会一直保持该操作。
- `exec:` 遵循 [`exec:` 策略](#exec-策略)：需要 `allow_exec = true`，一次一条
  命令，超时 5 秒，关闭 stdio。松开操作可能排在按下操作之后等待。
- 测试模式会报告两个边缘，但不会运行任何操作。录音键对话框中的**运行一次**会
  显式执行其中一个操作。
- `trigger_key` 与这些操作相互独立。如果两者发出同一个键，两个都会发送；要只保留
  操作，请设置 `trigger_key = "none"`。
- 这些操作是本地配置。提供者契约（`atvv.recording_keys`、`active_key` 和源元数据）
  保持不变。

## `exec:` 策略

`exec:` 是通用逃生口，不是脚本语言。除非 `allow_exec = true`，否则它被禁用。
守护进程以用户身份通过 `sh -c` 执行它，stdin/stdout/stderr 重定向到
`/dev/null`，一次只运行一条命令，超时 5 秒。以 root 运行的守护进程会直接拒绝
`exec:`。失败和超时会报告在 `atvvctl status`（`last_error`）和守护进程日志中。

编辑、读取或预览 `exec:` 操作永远不会运行它。只有分发的遥控器按键会运行它，
并且仅在 `allow_exec` 开启时。

## 应用更改

`atvvctl config set` 通过 D-Bus 发送整个文档。守护进程在应用任何内容之前校验
每个字段（包括每个已存储的布局），然后：

1. 应用增益、输入、预打开模式、vinput 模式、触发键、录音键操作、手势时序和活动
   布局的操作映射；
2. 当映射、时序、触发键或 vinput 模式变化时重启输入运行时，因为 uinput
   按键注册依赖映射。重启失败会恢复之前的运行时并报告错误；仅录音键操作变化
   不会重启输入运行时；
3. 原子地写入文件。写入失败会回滚运行时。

无效文档不会触碰活动映射、运行时或文件。`atvvctl layout use` 通过同一路径应用
所选布局并持久化 `active_layout`；运行时应用或写入失败时，之前的布局保持活动。
`atvvctl reload` 通过同一路径重新读取文件，并返回校验或应用错误，而不是报告成功。

按键映射是本地配置。它不属于 PipeWire 源上公布的提供方契约；消费者绑定声明的
录音键，而不是按键映射。

## `atvvctl`

```sh
atvvctl config get                 # 人类可读的有效文档
atvvctl config get --json          # 精确的 GetConfig 负载
atvvctl config set --gain-db 6
atvvctl config set --input false
atvvctl config set --preopen-mic true
atvvctl config set --vinput trigger --trigger-key DICTATE
atvvctl config set --allow-exec
atvvctl config set --recording-press key:KEY_F13 --recording-release none
atvvctl layout list
atvvctl layout show editor
atvvctl layout use editor
atvvctl layout new editor --name Editor --from default
atvvctl layout rename editor "Editor layout"
atvvctl layout delete editor
atvvctl layout reset editor
atvvctl layout reset editor --preset omp
atvvctl layout preset list
atvvctl layout preset show omp
atvvctl layout preset apply mpv
atvvctl layout preset apply omp --as agent --activate
atvvctl config set --layout editor --button up --trigger single --action key:KEY_F13
atvvctl config set --button up --trigger single --action key:KEY_F13
atvvctl config set --button ok --trigger long --action combo:ctrl+alt+v
atvvctl config set --button ok --trigger single --action pointer:click:left
atvvctl config set --button up --trigger single --action pointer:move:up:1
atvvctl config set --button up --trigger double --action pointer:move:up:10
atvvctl config set --button up --trigger long --action pointer:move:up:50
atvvctl config set --button up --trigger single --action none
atvvctl config set --button up --trigger single --action default
atvvctl reload
atvvctl map test --button up --trigger single
atvvctl map test --layout editor --button up --trigger single
atvvctl map test --action "combo:ctrl+alt+v"
atvvctl map test --action "pointer:click:left"
atvvctl capture --timeout 30    # 一次按下；打印逻辑按键和槽位
```

`atvvctl layout use` 发送 `SetLayout`。`layout new` 和 `layout rename` 不会切换
活动布局；`layout preset apply` 除非传入 `--activate`，也不会切换。`layout delete`
拒绝活动布局，`layout delete`、`layout rename` 和 `layout reset` 拒绝 `default`。
活动布局为 `default` 时，`config set --button` 会拒绝并提示使用
`atvvctl layout new <id> --from default`；使用 `--layout <id>`
可编辑其他用户布局。

`atvvctl map test` 默认解析活动布局，也可用 `--layout <id>` 或单个操作字符串。
它从不执行 `exec:`；只校验并打印按键程序。指针操作报告 `kind: pointer`，没有按键程序。`atvvctl status`
在 `input.pointer` 中报告指针设备是否活动；纯键盘映射为 false。

`atvvctl status` 打印完整的 `StatusView` JSON。`audio.level` 是增益后信号的
衰减峰值，单位为千分比（`0..=1000`）；空闲时为零，随状态发布更新，并与 GUI
电平条一致。`audio.gain_db` 是当前应用的增益。

`atvvctl capture` 启动捕获模式，等待一次逻辑按键按下，然后结束。捕获会报告输入
运行时识别出的逻辑按键和手势槽位。它本身不独占 evdev，也不暴露原始按键码。

## D-Bus API

`org.atvv.Daemon`，路径 `/org/atvv/Daemon`：

| 成员 | 签名 | 说明 |
|---|---|---|
| `GetConfig` | `() -> s` | JSON `ConfigDocument`。 |
| `SetConfig` | `(s) -> ()` | 校验版本 2、应用并持久化。无效输入返回 `org.freedesktop.DBus.Error.InvalidArgs` 和字段错误。 |
| `SetLayout` | `(s) -> ()` | 按 id 切换活动布局、应用并持久化。 |
| `BeginCapture` | `() -> ()` | 为调用连接启动捕获。 |
| `EndCapture` | `() -> ()` | 停止捕获。 |
| `LayoutChanged` | 信号 `(s)` | 新的活动布局 id。 |
| `ButtonEvent` | 信号 `(s, s, b)` | 捕获活动时的 `button`、`trigger`、`pressed`。 |

`GetStatus` 通过 `layout.id` 和 `layout.name` 报告活动布局。当调用 `BeginCapture`
的连接离开总线时，捕获模式会自动结束。负载由 `schema_version` 版本化；客户端
必须把完整的版本 2 文档（包括所有布局）发回 `SetConfig`。
