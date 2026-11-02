# 设备支持

[English](device-support.md) | **简体中文**

“支持”意味着：已编译的 `DeviceProfile`、本仓库记录的真实设备运行，以及安装该
配置文件的软件包。合成测试和 mock 传输只能证明代码路径，不能证明设备。

## 支持矩阵

| 设备 | USB/HID ID | 设备配置文件 | ATVV | 音频 | 状态 | 证据 |
|---|---|---|---|---|---|---|
| Xiaomi Bluetooth Remote 2 Pro / RC003 | `2717:32b8` | `miremote-rc003` | v1.0 | ADPCM 16 kHz，120 B 帧 | 2026-10-30 已验证 | [`rc003-hardware-evidence.md`](research/rc003-hardware-evidence.md) |
| Google Chromecast Remote / A3 | `18d1:9450` | `google-chromecast` | v1.0 | ADPCM 16 kHz，240 B 帧 | 2026-11-02 部分验证：ATVV 音频、流 ID 关闭/保活、配对、重连和 HID 按键映射已通过；双设备切换待完成 | [`chromecast-hardware-evidence.md`](research/chromecast-hardware-evidence.md) |
| Google Chromecast Voice Remote / DIS A0，固件 26.7 | `18d1:9450` | `google-chromecast` | v1.0 | ADPCM 16 kHz，160 B 帧（上游记录） | 未在 Linux 上运行；共用同一配置文件，帧长来自 `CAPS_RESP`，使用流 ID 关闭/保活 | [`zolo1978/google-remote-voice-mac@e57c0a4`](https://github.com/zolo1978/google-remote-voice-mac/tree/e57c0a47cece2014734f5bc74f4b5a81bc4e54f9) |
| 小米 RC 系列，其他固件 | 厂商 `2717`，名称 `MI RC*` | 无 | 未知 | 未知 | 不支持；需要配置文件和运行记录 | — |
| 没有配置文件的 ATVV v1.0 遥控器 | 未知 | 无 | v1.0 | 取决于配置文件 | 不支持；请添加 `DeviceProfile` | — |
| ATVV v0.4 遥控器 | 未知 | 无 | v0.4 | — | 超出范围 | — |
| `test-fake-remote` | `1209:0002`（uinput） | 仅测试 | 模拟 | 模拟 | 仅测试，永不进入 `PROFILES` | [`05-input-uinput.md`](plans/05-input-uinput.md) |

软件包验收：[`docs/acceptance/rc003-2026-10-30.md`](acceptance/rc003-2026-10-30.md)。

## RC003 配置文件固定的事实

- 直接 `AUDIO_START`，没有 `START_SEARCH`。
- 短 `MIC_OPEN` `[0C 00]`、裸 `MIC_CLOSE` `[0D]`、裸 `MIC_EXTEND` `[0E]`。
- 无头部 120 字节帧，高半字节在前，16 kHz，0 dB 增益。
- 停止后丢弃 300 ms，且 `AUDIO_STOP` 回声不得导致第二次 `MIC_CLOSE`。
- 按键以 HID 键盘形式到达；语音键是 `KEY_F5`，被独占，因此不会传到当前应用。
  主页是 `KEY_HOME`，应用/菜单键是 `KEY_COMPOSE`，在实测固件上 TV 是
  `KEY_GRAVE`。HID 描述符声明的是 `KEY_HOME` 和 `KEY_COMPOSE`，不是
  `KEY_HOMEPAGE`/`KEY_MENU`。

## Chromecast 配置文件固定的事实

- 直接 `AUDIO_START`，没有 `START_SEARCH`。
- 短 `MIC_OPEN` `[0C 00]`、流 ID `MIC_CLOSE` `[0D <stream>]`、流 ID
  `MIC_EXTEND` `[0E <stream>]`。A3 同时接受裸形式和流 ID 形式；流 ID 形式与
  A0 固件一致。
- 帧长来自 `CAPS_RESP`：A3 为 240 B，文档记录的 A0 固件为 160 B。配置文件中的
  240 B 只是回退值。帧无头部，高半字节在前，16 kHz，0 dB 增益。
- `CAPS_RESP` 为 10 字节（`0b 01 00 02 03 00 f0 01 00 1a`），解析器忽略末尾的
  `01 00 1a`。
- HID 使用者控制：电源是 `KEY_COFFEE`，OK 是 `KEY_SELECT`，主页是
  `KEY_HOMEPAGE`，返回是 `KEY_BACK`，Input 是 `KEY_TV`，静音是 `KEY_MUTE`。
  YouTube 和 Netflix 是 `KEY_CAMERA_ACCESS_DISABLE` 与
  `KEY_CAMERA_ACCESS_TOGGLE`。三者都可映射，默认会重新发出各自的物理按键。
  助手键不产生 evdev 事件，而是直接发送 ATVV `AUDIO_START`；守护进程从会话
  开始/停止合成消费者触发键。

## 添加设备

1. 在 `crates/atvv-protocol/src/profile.rs` 中添加 `DeviceProfile`，包含 id、
   名称前缀、编解码器、帧大小、命令变体、时序和按键。
2. 仅当厂商/产品 ID 与现有配置文件不同时，添加匹配的 udev 规则（`.rules` 中
   已有 RC003、Chromecast 和 `uinput` 的 `uaccess` 规则）。
3. 先用 mock/模拟会话运行该配置文件，再用真实设备运行。
4. 在 `docs/research/` 中记录运行，包括日期、地址、CAPS 字节、音频统计、保活
   和重连结果。
5. 只有在记录运行之后才更新此矩阵。
