# Step 20 — First release and external packaging

Status: 🚧 in-progress
Depends on: 19
Touches: `.github/workflows/`, `README.md`, `docs/`, `packaging/`

## Goal

Publish the first tagged release and move Arch recipes out of the project
repository. The repository keeps the source, systemd/udev/desktop assets, and
the release workflow; AUR package repositories and the personal VCS packaging
repository own the build recipes.

## Decisions

- First public version is `v0.1.0`, matching the workspace version and the
  existing release package series. The tag is the release source of truth.
- `.github/workflows/release.yml` runs only on `v*` tags. It builds the four
  binaries on `x86_64` and `aarch64`, packages each as
  `atvv-<version>-<arch>.tar.gz` with the systemd/udev/desktop assets and
  docs, and publishes the source archive plus checksums. CI still owns tests;
  the release job does not run them.
- The stable AUR packages are one split pkgbase `atvv` with
  `pkgname=('atvv' 'atvv-gui')`, built from the release tarball. One Cargo
  invocation compiles the shared graph once; `atvv-gui` depends on `atvv`.
- The VCS package is one split pkgbase `atvv-git` with
  `pkgname=('atvv-git' 'atvv-gui-git')` in the personal `arch-pkgs`
  repository; it is not an AUR package.
- Both pkgbases declare `x86_64` and `aarch64`. The aarch64 build is not run
  on this host.
- The repository no longer carries a `PKGBUILD` or packaging README.
  `packaging/` keeps only runtime assets consumed by the external recipes.

## Deliverables

- ✅ Tag-triggered release workflow building x86_64/aarch64 binaries and
  publishing them with the source archive and checksums.
- ✅ `v0.1.0` tag and source archive generated locally; the GitHub release is
  pending the tag push.
- ✅ AUR package repository `atvv` with split packages `atvv` and `atvv-gui`.
- ✅ `arch-pkgs` recipe `atvv-git` with split packages `atvv-git` and `atvv-gui-git`.
- ✅ Remove the in-repo PKGBUILD and packaging READMEs.
- ✅ Update README, GUI docs, AGENTS, plan index, and handoff.

## Verification

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
xvfb-run -a cargo test -p atvv-gui --test ui --test i18n
git tag -l v0.1.0
sha256sum atvv-linux-0.1.0.tar
makepkg --printsrcinfo | diff - .SRCINFO   # in each AUR package
makepkg -f                                  # in each AUR package, with SRCDEST staged
namcap PKGBUILD
```

## Results

- `git tag -l v0.1.0` → `v0.1.0`. The release job produces
  `atvv-linux-0.1.0.tar` and its SHA-256; the AUR PKGBUILD pins the checksum.
  The same archive from a shallow tag checkout has the same checksum.
- The binary jobs package `atvv-0.1.0-x86_64.tar.gz` and
  `atvv-0.1.0-aarch64.tar.gz` with the four binaries, systemd/udev/desktop
  assets, README, license, and third-party notices.
- One split pkgbase `atvv` builds both packages from one Cargo invocation.
  With the existing `$srcdir` (shared base artifacts), the split rebuild took
  86 s, Cargo 1m20s. Clean separate builds had measured 67 s + 61 s; the split
  avoids the second compilation of the shared graph. Sizes are unchanged:
  `atvv` 6.4 MiB / 18.1 MB installed; `atvv-gui` 1.3 MiB / 3.34 MB installed.
- The split build compiles `atvv-audio`, `atvv-daemon`, and `atvv-gui` in one
  graph; the shared crates are not compiled twice. No `check()` compilation is
  done; CI owns the test suite.
- `makepkg --printsrcinfo` matches `.SRCINFO`; the split repository is
  committed. `namcap PKGBUILD` is clean.
- `arch-pkgs` split pkgbase passes `namcap PKGBUILD`; `pkgver()` on the local
  tag yields `0.1.0.r0.g5569389`. A full clone build waits for the tag push.
- Package warnings are the expected implicit glibc/libgcc/libpipewire, the
  runtime-service `bluez`/`pipewire` notes, and the GUI's `librsvg` (SVG icon
  loader) and `atvv` (daemon dependency) notes.

## Exit criteria

- ⬜ Pushing `v0.1.0` creates a GitHub release with the tarball and checksum.
- ✅ The AUR PKGBUILDs build `atvv` and `atvv-gui` from the release archive.
- ⬜ The VCS PKGBUILDs track the GitHub repository and build the same binaries.
  The source URL and `pkgver()` are verified; the clone build waits for the
  tag push.
- ✅ No PKGBUILD or packaging recipe remains in `packaging/`.
- ✅ Install and GUI docs point at the AUR packages, not the repository checkout.

## Risks

- The source archive is uncompressed so the checksum does not depend on the
  runner's gzip/zstd version. It is larger but deterministic.
- The release binaries are built on Ubuntu runners and link the runner's
  glibc/GTK/PipeWire; they are not a distro package. CI and the release job
  use the same dependency set.
- The split build compiles the GUI even when only `atvv` is installed. This
  keeps the shared dependency graph at one compile, at the cost of GTK build
  dependencies for a base-only build.
- `atvv-gui` must be rebuilt whenever `atvv` changes; the exact-version
  dependency makes a stale GUI fail instead of silently mismatching.
- `aarch64` is declared but was not built on this x86_64 host.
- AUR names `atvv` and `atvv-gui` were unoccupied when checked on 2026-10-31.

## Progress log

- 2026-10-31 — release workflow, external packaging layout, and removal of the
  in-repo PKGBUILD planned by user direction.
- 2026-10-31 — release workflow added; in-repo PKGBUILD and packaging READMEs
  removed; README, GUI docs, AGENTS, plan index, and handoff updated.
- 2026-10-31 — `v0.1.0` tagged; source archive and checksum generated. The
  stable AUR packages were consolidated into one split pkgbase `atvv`
  (`atvv`, `atvv-gui`); the VCS recipes were consolidated into one split
  pkgbase `atvv-git` (`atvv-git`, `atvv-gui-git`) in `arch-pkgs`. Both declare
  `x86_64` and `aarch64`. The split AUR build completed; release push pending
  approval.
