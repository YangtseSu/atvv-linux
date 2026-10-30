# Arch package

`PKGBUILD` builds a split package: `atvv-linux` contains the daemon, CLI, and
MCP binaries, the user unit, udev rule, modules-load file, man pages, and
completions; `atvv-linux-gui` contains the optional `atvv-config` app plus its
desktop entry, AppStream metadata, and icon.

## Build the release tarball

`packaging/arch` is listed in `.gitattributes` with `export-ignore`, so the
source tarball never contains this directory or its checksum. Generate it from
a committed tree:

```sh
./make-source.sh
```

The script prints the SHA-256. Set that value in `sha256sums` before publishing;
the archive is deterministic for a given tree, and changes to this directory do
not change it.

## Build and install

```sh
cd packaging/arch
makepkg
sudo pacman -U atvv-linux-0.1.0-7-x86_64.pkg.tar.zst
# optional GUI:
sudo pacman -U atvv-linux-gui-0.1.0-7-x86_64.pkg.tar.zst
```

`makepkg -si` installs both packages. The packages do not enable a user
service. After installing:

```sh
systemctl --user daemon-reload
systemctl --user enable --now atvvd
```

`atvvd` reads `~/.config/atvv/daemon.toml`. Pair first or start with an explicit
address:

```sh
atvvctl pair C0:5D:39:F8:2D:4C
systemctl --user restart atvvd
```

## D-Bus policy

No system-wide D-Bus policy is installed. `atvvd` owns `org.atvv.Daemon` on the
session bus, which is allowed by the default session-bus policy. The daemon is
not a system service and must not be started by the system manager: BlueZ is
reached through the system bus, but audio and input belong to the logged-in
session.

## Verification

```sh
systemd-analyze verify packaging/systemd/atvvd.service
udevadm verify packaging/udev/70-atvv-linux.rules
```
