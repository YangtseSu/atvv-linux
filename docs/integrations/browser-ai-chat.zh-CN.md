# 浏览器 AI 聊天页面

[English](browser-ai-chat.md) | **简体中文**

浏览器页面通过标准 `getUserMedia` 使用 ATVV 源。没有 ATVV 专用浏览器 API、
扩展或云端点。页面拥有自己的录音按钮；遥控器拥有麦克风门控。

## 前提条件

- `atvvd` 在用户会话中运行。
- 遥控器已配对，源已常驻：

  ```sh
  pw-cli info atvv-miremote-rc003
  atvvctl contract --json
  ```

- 页面通过 HTTPS 或 `localhost` 提供；`getUserMedia` 需要安全上下文。

## 选择源

ATVV 源不是系统默认设备。在页面的麦克风选择器中按名称选择它，或在授予权限后
选择它的 `deviceId`：

```js
const devices = await navigator.mediaDevices.enumerateDevices();
const atvv = devices.find(
  (device) =>
    device.kind === "audioinput" &&
    (device.label.includes("atvv-") || device.label.includes("ATVV")),
);
const stream = await navigator.mediaDevices.getUserMedia({
  audio: {
    deviceId: atvv ? { exact: atvv.deviceId } : undefined,
    channelCount: 1,
    sampleRate: 16000,
  },
});
```

在用户授予麦克风权限之前，标签为空；如果页面需要预选源，请在第一次
`getUserMedia` 调用后再次枚举。

## 录音

1. 按下遥控器的语音键并保持按住。
2. 开始页面录音。
3. 释放语音键停止；守护进程关闭麦克风。

源是常驻的，遥控器空闲时输出静音。流已连接但录到静音，通常意味着语音键没有被
按住：RC003 麦克风由实体按键门控，而不是由主机流门控。

页面可以把 16 kHz 单声道源重采样到自己的速率；这是正常的。不要向守护进程请求
其他速率。

## 验证

```sh
atvvctl contract        # 源、声明的按键、活动按键
atvvctl status          # 契约加实时音频和输入状态
pw-cat --record --target atvv-miremote-rc003 /tmp/atvv.wav
```

在 `pw-cat` 运行时按住语音键。不按键时 WAV 是静音。

## 故障排查

| 症状 | 原因 | 操作 |
|---|---|---|
| 选择器中没有 `atvv-*` 设备 | PipeWire 或守护进程未运行，或浏览器看不到该节点 | 检查 `atvvctl doctor`；源存在后重启浏览器 |
| 已选择设备但电平一直不动 | 语音键未按住 | 在整个录音期间按住按键 |
| 权限错误 | 页面不是安全上下文 | 使用 HTTPS 或 `localhost` |
| 启动时麦克风错误 | 没有记住 `deviceId` | 再次选择源；持久化由页面负责 |

## 相关

- [提供方契约](provider-contract.zh-CN.md) — 源元数据、声明的按键和控制端点。
- [通用录音器](provider-contract.zh-CN.md#通用录音器) — 浏览器之外的
  `pw-cat` 检查。
