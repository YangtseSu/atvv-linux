# Step 19 — GUI localization and bilingual user docs

Status: ✅ done
Depends on: 14
Touches: `crates/atvv-gui/`, `packaging/gui/`, `packaging/arch/`, `docs/`,
`README.md`, `AGENTS.md`

## Goal

Localize the GTK configuration app into English and Simplified Chinese, and
publish the user-facing documentation in both languages. English stays the
primary language everywhere else: code, comments, commits, plans, research,
decisions, acceptance records, CLI output, and man pages.

## Decisions

- The GUI ships exactly two languages: English (`en`) and Simplified Chinese
  (`zh-Hans`). English is the source language and the fallback for any other
  or unsupported locale.
- The app follows the desktop locale (`LANGUAGE`, `LC_ALL`, `LC_MESSAGES`,
  `LANG`, as resolved by GLib). There is no in-app language switch. A `zh*`
  locale selects Simplified Chinese; Traditional Chinese locales fall back to
  Simplified because no `zh-Hant` catalog exists.
- Translations live in one Rust catalog (`crates/atvv-gui/src/i18n.rs`) keyed
  by the English string. No `.po`/`.mo` files, gettext runtime, or packaging
  locale directories are added. Every catalog lookup falls back to English, so
  a missing entry degrades to the source text instead of an empty label.
- Technical diagnostics from the daemon and from `atvv-config` validation stay
  English. The GUI localizes its own wrappers, prompts, labels, and status
  text; protocol values such as `single`, `none`, `KEY_F5`, and `DICTATE` are
  never translated.
- User-facing documents get a `.zh-CN.md` sibling with a language switch at
  the top. The English file stays authoritative. Development records
  (`docs/plans`, `docs/research`, `docs/decisions`, `docs/acceptance`,
  `docs/handoff.md`) remain English-only. Device data and quoted output may
  stay in the device's language.
- Desktop entry and AppStream metadata carry `zh_CN` / `zh-Hans-CN` name,
  summary, comment, and description entries.

## Deliverables

- ✅ `atvv-gui` i18n module: language enum, GLib locale detection, `tr()` and
  `tr_args()` helpers, and the English/Chinese catalog.
- ✅ All window chrome, banners, rows, buttons, tooltips, accessible names,
  dialogs, toasts, error wrappers, status text, hardware labels, button
  labels, action/dictation/gesture labels, and diagram text localized.
- ✅ Headless GUI test for the Chinese window; existing English smoke test
  stays deterministic (English unless explicitly switched).
- ✅ Localized desktop entry and AppStream metadata.
- ✅ Language switch links and Simplified Chinese versions for `README.md`,
  `docs/configuration.md`, `docs/gui.md`, `docs/device-support.md`, all
  `docs/integrations/*.md`, and `packaging/arch/README.md`.
- ✅ PKGBUILD installs the Chinese README, device-support, and GUI documents
  beside the English files.
- ✅ `AGENTS.md` records the language rule for future sessions.
- ✅ Step index and handoff updated in the same commit.

## Verification

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
xvfb-run -a cargo test -p atvv-gui --test ui
xvfb-run -a cargo test -p atvv-gui --test i18n
LANG=zh_CN.UTF-8 LANGUAGE=zh_CN xvfb-run -a \
  cargo test -p atvv-gui --test i18n -- --ignored
```

The Chinese test switches the catalog before building the window and asserts
localized title, battery text, hardware labels, and buttons. `Language` locale
mapping is unit-tested with `en_*`, `C`, `de_*`, `zh_CN.UTF-8`, `zh-Hans`, and
`zh_TW` inputs.

## Results

- `cargo fmt --all -- --check` → clean.
- `cargo clippy --workspace --all-targets -- -D warnings` → clean.
- `cargo test --workspace` → 171 passed, 5 ignored. The two new tests are the
  Chinese window and the ignored locale-detection test.
- `xvfb-run -a cargo test -p atvv-gui --test ui --test i18n` → English and
  Chinese windows passed.
- `LANG=zh_CN.UTF-8 LANGUAGE=zh_CN xvfb-run -a cargo test -p atvv-gui --test
  i18n -- --ignored` → `locale_selects_chinese` passed, proving GLib locale
  detection reaches the catalog.
- `desktop-file-validate packaging/gui/org.atvv.Config.desktop` → clean.
- `appstreamcli validate --no-net packaging/gui/org.atvv.Config.metainfo.xml`
  → success (one detail).
- `reuse lint` → compliant, 151 files.
- Local markdown link check across all tracked Markdown → no broken file
  targets.
- The msgid audit matched 127 catalog entries and 127 call sites with no
  missing or unused entries.

## Exit criteria

- ✅ With an English locale the GUI text is unchanged from step 14.
- ✅ With a Chinese locale every app-owned string is Chinese; protocol values,
  key names, addresses, and daemon diagnostics remain unchanged.
- ✅ A missing catalog entry shows the English source string, never a blank.
- ✅ Desktop search shows a Chinese name and comment under a Chinese locale.
- ✅ Every translated document links to its English counterpart and back.
- ✅ No Chinese text appears outside the GUI catalog and its tests, the Chinese
  documents, and quoted device data.

## Risks

- Selection indexes and key names are not translated; only labels change.
- The locale is read once at startup. Changing the desktop language requires
  restarting `atvv-config`; there is no live switch.
- Translations can drift when English strings change. The msgid-keyed catalog
  makes drift visible as an English fallback at runtime, and `tr_args`
  templates keep placeholders identical across languages.

## Progress log

- 2026-10-31 — step planned by user direction: GUI i18n (English primary,
  English and Simplified Chinese only), everything else English, user-facing
  docs bilingual with mutual links.
- 2026-10-31 — README status now describes project state and verified paths
  instead of the author's installed package version and running unit;
  developer-machine state remains in `docs/handoff.md`.
- 2026-10-31 — `atvv-gui` gained a msgid-keyed catalog (`i18n.rs`), GLib locale
  detection, `tr`/`tr_args`, and full coverage of window, editor, dialogs,
  status text, and diagram labels. Desktop entry and AppStream metadata carry
  Chinese translations. The English and Chinese window tests, the locale test,
  workspace checks, and REUSE pass.
- 2026-10-31 — README, configuration, GUI, device support, all integration
  guides, and the Arch package README have `.zh-CN.md` siblings with language
  switches. The PKGBUILD installs the Chinese README, device-support, and GUI
  documents.
