# Arch package

`PKGBUILD` builds a split VCS package: `atvv-linux-git` contains the daemon,
CLI, and MCP binaries, the user unit, udev rule, modules-load file, man pages,
and completions; `atvv-linux-gui-git` contains the optional `atvv-config` app
plus its desktop entry, AppStream metadata, and icon.

Both packages are built from this checkout's HEAD. `source` clones the local
repository, so uncommitted files are not part of the build; commit first.
`pkgver` is `0.1.0.r<commit-count>.g<short-hash>`.

`makepkg` compiles the release binaries once and does not run the test suite;
CI owns fmt, clippy, and the workspace/integration tests. The VCS
`src/atvv-linux-git/target/` directory is kept between builds, so a rebuild
only compiles changed crates.

## Build and install

```sh
cd packaging/arch
makepkg
sudo pacman -U atvv-linux-git-*.pkg.tar.zst
# optional GUI:
sudo pacman -U atvv-linux-gui-git-*.pkg.tar.zst
```

`makepkg -si` installs both packages. The `-git` packages provide and replace
`atvv-linux` and `atvv-linux-gui`; pacman removes the release packages when
they are replaced. The packages do not enable a user service. After
installing:

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
