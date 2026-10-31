# IM 应用

[English](im-applications.md) | **简体中文**

状态：通用指南。消息客户端通过自己的麦克风设置使用 ATVV 源。没有 ATVV 专用
集成、按客户端快捷键或按应用诊断。

典型客户端包括 Telegram Desktop、WeChat 和 Matrix 客户端。它们的菜单、快捷键
和录音模式由这些客户端拥有，本文不记录。

## 设置

1. 确认源存在：

   ```sh
   pw-cli info atvv-miremote-rc003
   atvvctl contract
   ```

2. 在客户端的音频或语音消息设置中按名称选择 ATVV 源
   （`atvv-miremote-rc003`）。如果客户端没有源选择器，修复应属于该客户端或其
   桌面门户权限；ATVV 不会更改系统默认设备。
3. 录音时按住遥控器语音键，释放即停止。遥控器按键只门控麦克风，不发送消息。
   使用客户端自己的发送控件。

源是常驻的，遥控器空闲时静音。只录到静音的录音通常意味着语音键没有被按住。

## 沙盒客户端

Flatpak 或门户沙盒客户端只能看到桌面门户暴露的设备。如果其选择器中没有 ATVV
源，请在桌面设置中授予麦克风访问权限，或使用能够选择 PipeWire 源的构建。不要
把 ATVV 源设为系统默认设备作为变通。

## 验证

```sh
atvvctl status          # 契约、输入状态、音频计数
pw-cat --record --target atvv-miremote-rc003 /tmp/atvv.wav
```

在按住语音键之前，录音是静音。

## 相关

- [提供方契约](provider-contract.zh-CN.md) — 声明的录音键和控制端点。
- [浏览器 AI 聊天页面](browser-ai-chat.zh-CN.md) — 浏览器麦克风选择器。
