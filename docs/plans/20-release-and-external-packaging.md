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
- `.github/workflows/release.yml` runs only on `v*` tags. It builds one
  uncompressed `git archive` tarball (`atvv-linux-<version>.tar`), writes its
  SHA-256, and creates the GitHub release with generated notes. No build matrix,
  cache, or third-party action is added; CI already owns tests.
- The stable AUR packages are `atvv` and `atvv-gui` (separate pkgbases), built
  from the release tarball. `atvv-gui` depends on `atvv`.
- The VCS recipes are `atvv-git` and `atvv-gui-git` in the personal
  `arch-pkgs` repository; they are not AUR packages.
- The repository no longer carries a `PKGBUILD` or packaging README.
  `packaging/` keeps only runtime assets consumed by the external recipes.

## Deliverables

- ⬜ Tag-triggered release workflow publishing the source archive and checksum.
- ⬜ `v0.1.0` tag and release archive.
- ⬜ AUR package repositories `atvv` and `atvv-gui`.
- ⬜ `arch-pkgs` recipes `atvv-git` and `atvv-gui-git`.
- ⬜ Remove the in-repo PKGBUILD and packaging READMEs.
- ⬜ Update README, GUI docs, AGENTS, plan index, and handoff.

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

## Exit criteria

- ⬜ Pushing `v0.1.0` creates a GitHub release with the tarball and checksum.
- ⬜ The AUR PKGBUILDs build `atvv` and `atvv-gui` from the release archive.
- ⬜ The VCS PKGBUILDs track the GitHub repository and build the same binaries.
- ⬜ No PKGBUILD or packaging recipe remains in `packaging/`.
- ⬜ Install and GUI docs point at the AUR packages, not the repository checkout.

## Risks

- The release archive is uncompressed so the checksum does not depend on the
  runner's gzip/zstd version. It is larger but deterministic.
- `atvv-gui` must be rebuilt whenever `atvv` changes; the exact-version
  dependency makes a stale GUI fail instead of silently mismatching.
- AUR names `atvv` and `atvv-gui` were unoccupied when checked on 2026-10-31.

## Progress log

- 2026-10-31 — release workflow, external packaging layout, and removal of the
  in-repo PKGBUILD planned by user direction.
