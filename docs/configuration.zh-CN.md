# 配置

[English](configuration.md) | **简体中文**

`daemon.toml` 是 `atvvd` 的唯一配置文件。它由 `atvv-config` crate 解析和校验；
守护进程、`atvvctl` 和 GUI 都不会再实现第二个解析器。GUI（`atvv-config`，见
[gui.zh-CN.md](gui.zh-CN.md)）通过 `GetConfig`/`SetConfig` 编辑文档，从不直接写
这个文件。文件是可选的：缺失时使用设备配置文件默认值。

位置：`$XDG_CONFIG_HOME/atvv/daemon.toml`，或 `~/.config/atvv/daemon.toml`。

## 模式

```toml
schema_version = 1
profile = "miremote-rc003"          # 必须与构建中的设备配置文件一致
device = "C0:5D:39:F8:2D:4C"        # 由 `atvvctl pair` 写入
adapter = "hci0"                    # 可选的 BlueZ 适配器
gain_db = 0                         # -24..=24
input = true                        # evdev/uinput 路径
vinput = "trigger"                  # trigger | dbus | off
trigger_key = "DICTATE"             # 按键或组合键；none 表示禁用
allow_exec = false                  # 启用 exec: 操作

[gesture]
double_click_ms = 300
long_press_ms = 550
repeat_start_ms = 350
stable_release_ms = 600
suppression_ms = 180
repeat_interval_ms = 100
back_repeat_interval_ms = 50

[buttons.up]
single = "key:KEY_F13"

[buttons.ok]
long = "combo:ctrl+alt+v"
```

| 字段 | 默认值 | 说明 |
|---|---|---|
| `schema_version` | `1` | 未知版本会被拒绝。 |
| `profile` | 当前设备配置文件 | 不同的 id 会被拒绝；按键映射只属于一个设备配置文件。 |
| `device` | 无 | 蓝牙地址；`atvvctl pair` 会写入。更改在下次启动时生效。 |
| `adapter` | BlueZ 默认 | 适配器名称，例如 `hci0`。 |
| `gain_db` | 设备配置文件值 | 限制在 ±24 dB；没有 AGC。 |
| `input` | `true` | 为 false 时禁用发现、独占和 uinput。 |
| `vinput` | `trigger` | `trigger` 合成 `trigger_key`；`dbus` 调用 `org.fcitx.Vinput`；`off` 禁用集成。 |
| `trigger_key` | `DICTATE` | 单个按键或组合键，例如 `F18` 或 `ctrl+rightctrl`。`none` 禁用合成。 |
| `allow_exec` | `false` | 任何 `exec:` 操作都需要它。 |
| `[gesture]` | 设备配置文件时序 | 每个字段覆盖一个设备配置文件值；边界会被校验。 |
| `[buttons.<button>]` | 设备配置文件默认值 | 按槽位的操作覆盖。 |

十二个可映射按键是 `power`、`ok`、`back`、`up`、`down`、`left`、`right`、
`home`、`menu`、`tv`、`volume_up` 和 `volume_down`。语音键不可映射：它控制
ATVV 麦克风和 vinput 触发。

槽位是 `single`、`double` 和 `long`。缺失的槽位表示“使用设备配置文件默认值”，
而不是“未映射”。默认映射会为每个逻辑按键重新发出原始按键，`power` 除外；
它默认为 `none`，以免与 logind 竞争。

## 操作语法

```text
none
key:<name>
combo:<mod>+<mod>+<name>
exec:<command>
dictation:start|stop|toggle
```

按键名称使用与输入运行时相同的解析器：内核名称（`KEY_F13`）、别名（`ctrl`、
`alt`、`super`、`enter`、`esc`、`dictate`）以及合成的 `KEY_<NAME>` 形式都可接受。
组合键先按下修饰键，再按相反顺序释放。一个按键在组合键中只能出现一次。

`none` 会吞掉按键。`dictation:start|stop|toggle` 只驱动 ATVV 会话，不发出按键。

校验会拒绝未知按键、语音键、未知按键名称、组合键中重复的按键、格式错误的操作、
超出范围的增益或手势值，以及 `allow_exec = false` 时的 `exec:` 操作。错误信息会
指出按键、槽位和值。

## `exec:` 策略

`exec:` 是通用逃生口，不是脚本语言。除非 `allow_exec = true`，否则它被禁用。
守护进程以用户身份通过 `sh -c` 执行它，stdin/stdout/stderr 重定向到
`/dev/null`，一次只运行一条命令，超时 5 秒。以 root 运行的守护进程会直接拒绝
`exec:`。失败和超时会报告在 `atvvctl status`（`last_error`）和守护进程日志中。

编辑、读取或预览 `exec:` 操作永远不会运行它。只有分发的遥控器按键会运行它，
并且仅在 `allow_exec` 开启时。

## 应用更改

`atvvctl config set` 通过 D-Bus 发送整个文档。守护进程在应用任何内容之前校验
每个字段，然后：

1. 应用增益、输入、vinput 模式、触发键、手势时序和操作映射；
2. 当映射、时序、触发键或 vinput 模式变化时重启输入运行时，因为 uinput
   按键注册依赖映射。重启失败会恢复之前的运行时并报告错误；
3. 原子地写入文件。写入失败会回滚运行时。

无效文档不会触碰活动映射、运行时或文件。`atvvctl reload` 通过同一路径重新读取
文件，并返回校验或应用错误，而不是报告成功。

按键映射是本地配置。它不属于 PipeWire 源上公布的提供方契约；消费者绑定声明的
录音键，而不是按键映射。

## `atvvctl`

```sh
atvvctl config get                 # 人类可读的有效文档
atvvctl config get --json          # 精确的 GetConfig 负载
atvvctl config set --gain-db 6
atvvctl config set --input false
atvvctl config set --vinput trigger --trigger-key DICTATE
atvvctl config set --allow-exec
atvvctl config set --button up --trigger single --action key:KEY_F13
atvvctl config set --button ok --trigger long --action combo:ctrl+alt+v
atvvctl config set --button up --trigger single --action none
atvvctl config set --button up --trigger single --action default
atvvctl reload
atvvctl map test --button up --trigger single
atvvctl map test --action "combo:ctrl+alt+v"
atvvctl capture --timeout 30    # 一次按下；打印逻辑按键和槽位
```

`atvvctl map test` 解析活动文档或单个操作字符串。它从不执行 `exec:`；只校验并
打印按键程序。

`atvvctl capture` 启动捕获模式，等待一次逻辑按键按下，然后结束。捕获会报告输入
运行时识别出的逻辑按键和手势槽位。它本身不独占 evdev，也不暴露原始按键码。

## D-Bus API

`org.atvv.Daemon`，路径 `/org/atvv/Daemon`：

| 成员 | 签名 | 说明 |
|---|---|---|
| `GetConfig` | `() -> s` | JSON `ConfigDocument`。 |
| `SetConfig` | `(s) -> ()` | 校验、应用并持久化。无效输入返回 `org.freedesktop.DBus.Error.InvalidArgs` 和字段错误。 |
| `BeginCapture` | `() -> ()` | 为调用连接启动捕获。 |
| `EndCapture` | `() -> ()` | 停止捕获。 |
| `ButtonEvent` | 信号 `(s, s, b)` | 捕获活动时的 `button`、`trigger`、`pressed`。 |

当调用 `BeginCapture` 的连接离开总线时，捕获模式会自动结束。负载由
`schema_version` 版本化；客户端必须把完整文档发回 `SetConfig`。
