# Arch 软件包

[English](README.md) | **简体中文**

`PKGBUILD` 构建一个拆分的 VCS 软件包：`atvv-linux-git` 包含守护进程、CLI 和
MCP 二进制、用户单元、udev 规则、modules-load 文件、man 手册和补全；
`atvv-linux-gui-git` 包含可选的 `atvv-config` 应用及其桌面条目、AppStream
元数据和图标。

两个软件包都从当前检出的 HEAD 构建。`source` 克隆本地仓库，因此未提交文件不会
进入构建；请先提交。`pkgver` 是 `0.1.0.r<commit-count>.g<short-hash>`。

`makepkg` 只编译一次发布二进制，不运行测试套件；CI 负责 fmt、clippy 以及
工作区/集成测试。VCS 的 `src/atvv-linux-git/target/` 目录在构建之间保留，因此
重新构建只编译变化的 crate。

## 构建与安装

```sh
cd packaging/arch
makepkg
sudo pacman -U atvv-linux-git-*.pkg.tar.zst
# 可选 GUI：
sudo pacman -U atvv-linux-gui-git-*.pkg.tar.zst
```

`makepkg -si` 会安装两个软件包。`-git` 软件包提供并替换 `atvv-linux` 和
`atvv-linux-gui`；被替换时 pacman 会移除发布包。软件包不会启用用户服务。安装后：

```sh
systemctl --user daemon-reload
systemctl --user enable --now atvvd
```

`atvvd` 读取 `~/.config/atvv/daemon.toml`。先配对，或使用显式地址启动：

```sh
atvvctl pair C0:5D:39:F8:2D:4C
systemctl --user restart atvvd
```

## D-Bus 策略

不安装系统级 D-Bus 策略。`atvvd` 在会话总线上拥有 `org.atvv.Daemon`，默认会话
总线策略允许这样做。守护进程不是系统服务，不得由系统管理器启动：BlueZ 通过
系统总线访问，但音频和输入属于已登录会话。

## 验证

```sh
systemd-analyze verify packaging/systemd/atvvd.service
udevadm verify packaging/udev/70-atvv-linux.rules
```
