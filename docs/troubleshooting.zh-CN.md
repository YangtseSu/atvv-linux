# 故障排查

[English](troubleshooting.md) | **简体中文**

当遥控器、音频源或消费者不工作时，从这里开始。先运行检查，它会按层报告：

```sh
atvvctl doctor
```

正常时每行都显示 `ok`。GUI 的**入门检查…**对话框从 `GetStatus`/`GetConfig`
显示同样的四层，并可复制其中的命令。两种检查都不会启动服务、打开蓝牙或写入
消费者配置。

## `atvvctl` 无法连接守护进程

症状：

```text
atvvctl: org.freedesktop.DBus.Error.ServiceUnknown: The name org.atvv.Daemon was not provided by any .service files
```

`atvvctl doctor` 会报告 `daemon fail` 并跳过音频源检查。检查用户单元和日志：

```sh
systemctl --user status atvvd --no-pager
systemctl --user enable --now atvvd
journalctl --user -u atvvd -n 50 --no-pager
```

预期：`Active: active (running)`，以及一条带传输和集成模式的启动日志：

```text
atvvd: bluez transport for C0:5D:39:F8:2D:4C on org.atvv.Daemon /org/atvv/Daemon; vinput=trigger trigger=DICTATE
```

如果单元在运行但没有 D-Bus 名称，GUI 会停留在**守护进程不可用**页面；等单元
报告 active 后重试。如果日志停在 `no device address`，继续下一节。

## 未配置遥控器，或遥控器已断开

症状：`atvvctl doctor` 报告 `daemon fail` 或 `device fail`；GUI 入门检查的
**设备**未通过；`atvvctl status` 返回 `"connected": false`。断开时 GUI 的
电量显示为 `last known`。

```sh
atvvctl device list
atvvctl pair C0:5D:39:F8:2D:4C
bluetoothctl info C0:5D:39:F8:2D:4C
atvvctl device use C0:5D:39:F8:2D:4C
```

`atvvctl device list` 用 `*` 标记当前设备：

```text
active: C0:5D:39:F8:2D:4C
* C0:5D:39:F8:2D:4C 小米蓝牙语音遥控器  miremote-rc003  custom  connected 60%
```

`bluetoothctl` 应显示 `Paired: yes`、`Trusted: yes`、`Connected: yes`。
守护进程会自动重试断开的链路；按一下遥控器按键唤醒它，并用
`atvvctl monitor` 观察。`atvvctl pair` 会把地址写入 `daemon.toml`（除非传入
`--no-save`）。使用 `atvvctl device use <MAC>` 可在不重启守护进程的情况下切换
已知设备。

## PipeWire 中缺少音频源

症状：`atvvctl doctor` 报告 `source fail` 或 `pipewire fail`；GUI 入门检查的
**音频源**未通过；消费者列不出该源。

```sh
atvvctl status
pw-cli info atvv-miremote-rc003
pw-cli list-objects | grep atvv
```

预期的节点信息：

```text
type: PipeWire:Interface:Node/3
node.name = "atvv-miremote-rc003"
media.class = "Audio/Source"
audio.rate = "16000"
atvv.contract_version = "1"
atvv.profile = "miremote-rc003"
atvv.recording_keys = "DICTATE,VOICECOMMAND,ASSISTANT"
```

`atvvctl status` 显示 `"audio": {"available": true, ...}`。如果守护进程正常但
节点消失，重启守护进程，它会重新创建音频源：

```sh
systemctl --user restart atvvd
atvvctl doctor
```

## 音频源存在，但消费者不录音

音频源是常驻的，但只有按住遥控器语音键时才会传输数据。消费者链接到音频源
并不会打开麦克风。

```sh
atvvctl contract --json
pw-link -l
```

`atvvctl contract --json` 打印源、声明的按键、当前按键和一行消费者说明。
`pw-link -l` 只列出当前链接，因此结果为空表示没有消费者连接。在消费者中选择
`atvv-<profile>`，并绑定一个声明的录音键。fcitx5-vinput 不会自动绑定：

- 保留用户已有的 `[TriggerKey]` 条目，并添加 `1=Dictate`（默认的
  `0=Alt_R` 可以保留）；
- 编辑前停止 fcitx5，编辑后完整启动；`fcitx5-remote -r` 可能仍保留旧的
  触发列表；
- 整段注释掉的 `[TriggerKey]` 会禁用触发。

提供方只发出一个当前按键：默认 `DICTATE`，或配置的 `trigger_key`。见
[`fcitx5-vinput.zh-CN.md`](integrations/fcitx5-vinput.zh-CN.md) 和
[`provider-contract.zh-CN.md`](integrations/provider-contract.zh-CN.md)。GUI
的录音键对话框和 `trigger_key` 配置值会改变该输出；不要在消费者中为同一个键
再添加第二个触发绑定。

## 触发键没有到达，或 F5 传到了当前应用

遥控器的 evdev 节点必须被独占，且 `uinput` 可写，才能发出合成按键。

```sh
atvvctl status
atvvctl doctor
ls -l /dev/uinput
```

`atvvctl status` 中预期：

```text
"input": {"available": true, "enabled": true, "grabbed": ["/dev/input/event26"], "keyboard": true, ...}
```

`atvvctl doctor` 打印 `uinput ok /dev/uinput is writable`。如果节点未被独占，
关闭占用它的其他程序（另一个输入守护进程或调试器）并重启 `atvvd`。如果
`/dev/uinput` 不可写，检查 udev 的 `uaccess` ACL，并在安装软件包后重新登录：

```sh
getfacl /dev/uinput
```

`F5` 传到当前应用说明独占失败；按同样的方式检查。键盘或当前按键不可用时，
GUI 入门检查会把**输入与触发键**标为未通过。

## PipeWire 服务重启之后

打包的用户单元是 `PartOf=pipewire.service` 且 `Restart=always`，因此 systemd
会随 PipeWire 一起重启 `atvvd`。守护进程不会自行重启 PipeWire。

```sh
systemctl --user status atvvd --no-pager
atvvctl doctor
pw-cli info atvv-miremote-rc003
```

如果单元报告 active 后节点仍然缺失，重启一次守护进程
（`systemctl --user restart atvvd`），并在日志中查找
`atvv-audio: PipeWire source restart:` 及其后的 PipeWire 错误。

## 遥控器重连与过期的设备信息

断连后守护进程保持会话并重试链路。上次连接的电量、固件和硬件信息会继续显示，
并标记为 `last known`；`connected` 为 false 时它们不是实时值。

```sh
atvvctl monitor
bluetoothctl info C0:5D:39:F8:2D:4C
```

唤醒遥控器，等待 `atvvctl monitor` 中的 `state=` 变化。如果遥控器一直断开，
先给它断电再上电，然后检查 `bluetoothctl` 和日志。过期的 `last_error` 只是
提示；当前状态以 `atvvctl doctor` 为准。

## 没有电平，或信号削波

GUI 的**菜单 → 麦克风…**对话框显示增益后的峰值和增益滑块。相同的值也在
`atvvctl status` 中：

```text
"audio": {"level": 0, "gain_db": 0, "available": true, "streaming": false, ...}
```

`level` 是满量程的千分比（`1000` = 0 dBFS），在帧之间衰减，并且是在增益后
测量。`streaming` 为 true 时电平一直为 `0`，说明到达音频源的是静音；检查实体
语音键和遥控器链路。电平反复达到 `1000` 说明削波：在 GUI 中降低增益，或运行：

```sh
atvvctl config set --gain-db -6
```

增益在 ADPCM 解码后、送入音频源前应用。默认 `0 dB`；允许范围是
`-24..=24 dB`（见 `MAX_GAIN_DB`）。该值随其余配置一起保存并实时生效。

## 日志与诊断

```sh
journalctl --user -u atvvd -f
atvvctl monitor
atvvctl doctor
atvvctl status
atvvctl contract --json
```

- `atvvd: bluez transport for <MAC> on org.atvv.Daemon ...` 是正常的启动行；
  `vinput=trigger trigger=DICTATE` 标明当前集成方式。
- `atvvd: transport error: ...` 和 `atvvd: write failed: ...` 在
  `--verbose` 时出现；相同文本也在 `atvvctl status` 的 `last_error` 中。
- `atvv-audio: PipeWire source restart: ...` 表示音频图变化后源已被重新创建。
- `atvvctl doctor` 按顺序检查注册表、守护进程、音频源、PipeWire 节点、契约
  属性、硬件信息、BlueZ、设备、ATVV 服务、uinput、vinput 和配置文件。

不要在这里添加按应用的检查。提供方看不到消费者的配置；下一步是集成指南：
[`docs/integrations/`](integrations/README.zh-CN.md)。
