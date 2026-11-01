# atvv-linux

[English](README.md) | **简体中文**

面向 Linux 的 ATVV v1.0 用户态栈：把 BLE 语音遥控器变成 PipeWire 麦克风，并提供
evdev/uinput 按键映射和 D-Bus 控制接口。

本项目不是内核驱动、BlueZ 插件或 PipeWire 模块。BlueZ 负责配对、GATT 和 HID；
用户态守护进程负责 ATVV；PipeWire 负责音频图；evdev/uinput 负责输入重映射。

## 状态

ATVV v1.0 采集、输入重映射、守护进程和 D-Bus API、CLI、MCP 服务器以及可选的
GTK 配置应用均已实现。RC003 音频路径和 fcitx5-vinput 源路径已在真实设备和系统
服务上验证；证据见下文链接。

fcitx5-vinput 的自动设置依赖上游消费者改动。在该功能落地之前，请按名称选择
ATVV 源并手动添加声明的触发键；见
[fcitx5-vinput 指南](docs/integrations/fcitx5-vinput.zh-CN.md)。

已实现：

- `atvv-protocol`：v1.0 能力解析、控制事件、ADPCM、帧累积、设备配置文件和纯
  会话状态机。
- `atvv-audio`：原生 PipeWire `Audio/Source`，带容量受限的无锁队列、合成音测试
  和节点移除恢复。消费者链接时源会被唤醒（`node.pause-on-idle=false`）；PipeWire
  服务重启由 systemd 单元负责（ADR 0005）。
- `atvv-input`：由配置文件驱动的 evdev 发现和 `EVIOCGRAB`、RC003 手势引擎、
  uinput 虚拟键盘，以及为被独占的语音键配置合成触发键/组合键。
  `atvvctl input fake-remote` 无需硬件即可走完整条路径。
- `atvv-config`：唯一的 `daemon.toml` 模式解析器、带版本号的 JSON 配置文档、
  按按键校验操作、手势覆盖和 `exec:` 策略。实时改动通过 `atvvctl config` 或
  D-Bus `SetConfig` 方法应用。
- `atvv-bluez`：BlueZ GATT 传输（`bluer`）、适配器/设备选择、带 `StartNotify`
  回退的 `AcquireNotify`、配对代理和重连处理。
- `atvv-daemon` / `atvvd`：`Transport` trait 之后的会话编排、mock 与 BlueZ
  传输、定时器、PCM 解码、常驻 PipeWire 源、语音键边沿麦克风控制、触发键和
  `org.fcitx.Vinput` 模式、输入层，以及 `org.atvv.Daemon` 上的 JSON 视图模型。
- `atvvctl`：配置文件查看、CAPS 解码、无硬件的 RC003 会话模拟、音频源冒烟测试、
  假输入遥控器，以及 `status`、`contract`、`mic start|stop|toggle`、`reload`、
  `config get|set`、`capture`、`map test`、`pair`、`record`、`monitor` 和
  `doctor`。
- `atvv-mcp`：stdio MCP 服务器，提供 `status`、`contract`、`voice_start`、
  `voice_stop`、`voice_toggle`、`list_devices` 和 `reload`。
- `atvv-gui` / `atvv-config`：GTK4 + libadwaita 配置应用，包含可点击的遥控器
  示意图、按槽位的操作编辑器、键盘和遥控器按键学习、录音键输出选择、测试模式
  和脚本策略。界面跟随桌面语言环境：英文，`zh*` 语言环境使用简体中文。它只是
  D-Bus 客户端；见 [`docs/gui.zh-CN.md`](docs/gui.zh-CN.md)。

第一个配置文件是小米蓝牙语音遥控器 2 Pro（`miremote-rc003`，ATVV v1.0，
16 kHz ADPCM，120 字节帧）。2026-10-30 的运行（`C0:5D:39:F8:2D:4C`）验证了
真实 CAPS、音频、保活和重连；确切证据和剩余缺口（配对手势、可懂度、已安装单元
的 PipeWire 重启）见
[`docs/research/rc003-hardware-evidence.md`](docs/research/rc003-hardware-evidence.md)。

## 安装（Arch）

要求：Arch Linux、systemd 用户会话、PipeWire、BlueZ，以及（语音键集成所需的）
fcitx5-vinput。守护进程是用户服务；不要以 root 运行。可选的 `atvv-gui` 软件包
额外需要 GTK4、libadwaita 和 librsvg。

AUR 软件包从发布源码归档构建：

```sh
git clone https://aur.archlinux.org/atvv.git
cd atvv
makepkg -si

# 可选 GUI：
git clone https://aur.archlinux.org/atvv-gui.git
cd atvv-gui
makepkg -si
```

先安装 `atvv` 再安装 `atvv-gui`；AUR 助手会解析该依赖。GUI 应用是可选的；
守护进程和 CLI 不依赖 GTK。

先配对遥控器，再启用用户服务：

```sh
atvvctl pair C0:5D:39:F8:2D:4C
systemctl --user daemon-reload
systemctl --user enable --now atvvd
atvvctl doctor
```

`atvvd` 通过 PipeWire 提供 `atvv-miremote-rc003`。在 fcitx5-vinput 中按名称选择它
（见 [`docs/integrations/fcitx5-vinput.zh-CN.md`](docs/integrations/fcitx5-vinput.zh-CN.md)）；
不要把它设为系统默认设备。

按键映射、手势时序、`exec:` 启用方式和实时 `SetConfig` 路径记录在
[`docs/configuration.zh-CN.md`](docs/configuration.zh-CN.md)。

软件包安装 `/usr/bin/{atvvd,atvvctl,atvv-mcp}`；
`/usr/lib/systemd/user/atvvd.service`；
`/usr/lib/udev/rules.d/70-atvv-linux.rules`（对 RC003 事件节点和
`/dev/uinput` 使用 `uaccess`）；`/usr/lib/modules-load.d/atvv-linux.conf`
（`uinput`）；man 手册；以及 bash/zsh/fish 补全。不安装系统级 D-Bus 策略：
守护进程使用会话总线。

## 故障排查

| 症状 | 检查 |
|---|---|
| `cannot connect to the session D-Bus` | `systemctl --user status atvvd`；`systemctl --user enable --now atvvd` |
| `The name is not activatable` | `atvvd` 未运行；检查 `systemctl --user status atvvd`。`0.1.0-2` 在遥控器断开时保持守护进程运行并重试链接。 |
| `no device address` | `atvvctl pair <MAC>` 会写入 `~/.config/atvv/daemon.toml` |
| 音频源缺失 | `pw-cli info atvv-miremote-rc003`；`atvvctl doctor` |
| `uinput` 不可写或按键未被独占 | `/dev/uinput` 存在且可写；`uinput` 模块已加载；安装 udev 规则后重新登录 |
| 遥控器未连接或缺少 CAPS | `bluetoothctl info <MAC>`；`atvvctl doctor`；`atvvctl monitor` |
| 语音键没有触发 fcitx | 守护进程默认 `trigger_key = "DICTATE"`。保留用户的 `[TriggerKey]` 条目并添加 `1=Dictate`；编辑前停止 fcitx5，编辑后重启。整段注释掉的 `[TriggerKey]` 会禁用触发。`F24` 是回退键。 |
| F5 传到了当前应用 | RC003 节点未被独占；检查 `atvvctl status` 中的 `input.grabbed`，并关闭其他独占程序 |
| PipeWire 重启后没有音频源 | `PartOf=pipewire.service` 会重启 `atvvd`；检查 `atvvctl status` 和 `pw-cli info` |
| 音频源存在但 fcitx 未列出 | 音频源出现后重启 `vinput-daemon` |

设备支持和真实设备记录见
[`docs/device-support.zh-CN.md`](docs/device-support.zh-CN.md) 和
[`docs/research/rc003-hardware-evidence.md`](docs/research/rc003-hardware-evidence.md)。

## 构建与测试

```sh
cargo test --workspace
cargo run -q -p atvv-cli -- profile show miremote-rc003
cargo run -q -p atvv-cli -- caps decode 0B010002000078
cargo run -q -p atvv-cli -- simulate miremote-rc003
cargo run -q -p atvv-cli -- audio test --seconds 3
cargo run -q -p atvv-cli -- input fake-remote
dbus-run-session -- cargo run -q -p atvv-daemon -- --mock
cargo run -q -p atvv-cli -- status
cargo run -q -p atvv-cli -- config get --json
cargo run -q -p atvv-cli -- config set --button up --trigger single --action key:KEY_F13
cargo run -q -p atvv-cli -- map test --button up --trigger single
cargo run -q -p atvv-cli -- capture --timeout 10
cargo run -q -p atvv-cli -- doctor
cargo run -q -p atvv-cli -- pair C0:5D:39:F8:2D:4C
cargo run -q -p atvv-daemon -- --device C0:5D:39:F8:2D:4C --verbose
cargo run -q -p atvv-cli -- record --wav /tmp/rc003.wav --seconds 10
cargo run -q -p atvv-cli -- monitor
cargo run -q -p atvv-mcp
cargo run -q -p atvv-gui
```

## 目录结构

```text
crates/atvv-protocol/   协议、ADPCM、帧、设备配置文件、会话状态
crates/atvv-config/     daemon.toml 模式、校验、JSON 配置文档
crates/atvv-audio/      原生 PipeWire Audio/Source 和 PCM 队列
crates/atvv-input/      evdev 发现/独占、手势、uinput 键盘
crates/atvv-bluez/      BlueZ GATT 传输和配对代理
crates/atvv-daemon/     atvvd 会话守护进程、mock 传输、D-Bus API
crates/atvv-cli/        atvvctl 诊断和 D-Bus 控制
crates/atvv-mcp/        面向 agent 的 stdio MCP 服务器
crates/atvv-gui/        GTK4/libadwaita 配置应用（atvv-config）
packaging/              systemd 用户单元、udev 规则、GUI 桌面元数据
docs/plans/             规范实现计划和验收标准
docs/decisions/         架构决策
docs/research/          领域调研、协议事实、参考快照
docs/integrations/      fcitx5-vinput 和编码 agent 配置
docs/handoff.md         下一次会话的当前状态
```

## 范围

范围内：ATVV v1.0+、Linux、BlueZ、PipeWire、evdev/uinput、D-Bus、
fcitx5-vinput、编码 agent 集成、设备配置文件、打包。

暂不在范围内：ATVV v0.4、ASR 引擎、非官方云协议、非 ATVV 传输、
内核/BlueZ/PipeWire 补丁、macOS/Windows。

## 致谢

本项目在开发过程中参考了若干开源项目。来源、固定快照、许可证和复用边界列在
[`docs/research/reference-projects.md`](docs/research/reference-projects.md)。

## 许可证

GPL-3.0-or-later。见 `LICENSE`。
