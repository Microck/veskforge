<p align="center">
  <picture>
    <source srcset="design/assets/veskforge-logo-dark.svg" media="(prefers-color-scheme: dark)">
    <source srcset="design/assets/veskforge-logo-light.svg" media="(prefers-color-scheme: light)">
    <img src="design/assets/veskforge-logo-light.svg" alt="veskforge" width="240">
  </picture>
</p>

<p align="center">
  <a href="https://github.com/Microck/veskforge/releases"><img src="https://img.shields.io/github/v/release/Microck/veskforge?display_name=tag&style=flat-square&label=release&color=000000" alt="release badge"></a>
  <a href="https://github.com/Microck/veskforge/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/Microck/veskforge/ci.yml?branch=main&style=flat-square&label=ci&color=000000" alt="ci badge"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-mit-000000?style=flat-square" alt="license badge"></a>
</p>

---

`veskforge` is an unofficial desktop build manager for Vesktop users who want custom Vencord plugins without hand-running the source workflow every time. it manages a Vencord checkout, installs enabled plugins into `src/userplugins`, builds Vencord, validates the desktop `dist`, and points Vesktop at that build through Vesktop's supported `vencordDir` state setting.

the main path is intentionally boring: add a local plugin or Git URL, build Vencord, apply the validated `dist`, then fully restart Vesktop. veskforge does not patch Vesktop binaries and does not try to runtime-inject plugins into Vencord bundles.

## why

Vencord custom plugins are compile-time plugins. Vesktop loads a built Vencord desktop bundle, so the stable workflow is to build a custom Vencord `dist` and configure Vesktop to use it.

- keeps to Vesktop's supported custom Vencord location instead of patching installed app files
- supports local plugin files, local plugin folders, and Git plugin sources
- stores plugin state in one manifest and recreates `src/userplugins` from that manifest before each build
- validates required Vencord desktop artifacts before touching Vesktop state
- preserves unrelated Vesktop `state.json` fields when applying `vencordDir`
- defaults updates to manual apply, with an explicit auto-rebuild preference stored for later automation

## requirements

- **windows or linux** for the v1 target platforms
- **git**, **node**, and **pnpm** available on `PATH`
- **Vesktop** installed normally
- **trusted custom plugins** only; veskforge does not sandbox plugin code
- **Rust toolchain** only if building veskforge from source

## quickstart

for normal use, install the latest build from [releases](https://github.com/Microck/veskforge/releases):

| platform | artifact |
| --- | --- |
| windows | `veskforge_*_x64-setup.exe` |
| linux | `.deb`, `.rpm`, or `.AppImage` |

to run from source:

```bash
pnpm install --trust-policy-exclude semver@6.3.1
pnpm tauri dev
```

the `--trust-policy-exclude` flag is currently needed in this environment because pnpm blocks `semver@6.3.1` as a transitive trust-policy downgrade under `@babel/core`.

to create a linux deb package:

```bash
pnpm tauri build --bundles deb
```

the generated package is written under:

```bash
src-tauri/target/release/bundle/deb/
```

## workflow

| step | behavior |
| --- | --- |
| add plugin | register a local `.ts` / `.tsx` file, local folder with `index.ts(x)`, or Git URL |
| build | clone or update `Vendicated/Vencord`, recreate `src/userplugins`, run `pnpm install --frozen-lockfile`, then `pnpm build` |
| validate | require `package.json`, `vencordDesktopMain.js`, `vencordDesktopPreload.js`, `vencordDesktopRenderer.js`, `vencordDesktopRenderer.css` |
| apply | write the validated `dist` path to Vesktop `state.json` as `vencordDir` |
| restart | fully restart Vesktop so it loads the custom Vencord build |

## plugin sources

| source | expected shape |
| --- | --- |
| local file | `.ts` or `.tsx`; copied into a generated plugin folder as `index.ts` |
| local folder | folder containing `index.ts` or `index.tsx` |
| Git URL | `https://`, `ssh://`, or `git@` URL, with optional branch, tag, or commit ref |

veskforge recreates the managed Vencord `src/userplugins` folder from the manifest on each build. disabled plugins stay in the manifest but are not materialized into the next build.

## paths

veskforge stores its own state in the platform app data directory. the managed Vencord checkout lives under that app data directory at:

```text
workspace/Vencord
```

Vesktop state detection checks common locations such as:

| platform | candidate |
| --- | --- |
| linux | `~/.config/vesktop/state.json` |
| linux flatpak | `~/.var/app/dev.vencord.Vesktop/config/vesktop/state.json` |
| windows | `%APPDATA%\\vesktop\\state.json` |

you can also paste a `state.json` path in the app, or set `VESKTOP_STATE_FILE` before launching veskforge.

## update model

veskforge defaults to manual updates. the app can check whether the managed Vencord checkout differs from `origin/main`; rebuilding is an explicit action unless auto rebuild is enabled in settings.

this is intentional. Vencord and Discord internals can change, and a plugin that built yesterday may fail after an upstream update.

## design assets

logo files:

- `design/assets/veskforge-logo-light.svg`
- `design/assets/veskforge-logo-dark.svg`

see [design notes](design/README.md) for the UI direction.

## building from source

```bash
pnpm install --trust-policy-exclude semver@6.3.1
pnpm build
cd src-tauri
cargo check
```

build a desktop binary and deb package:

```bash
pnpm tauri build --bundles deb
```

build all configured linux bundles:

```bash
pnpm tauri build
```

windows `.exe` installers are built by GitHub Actions on `windows-latest` using the same `pnpm tauri build` command. CI uploads the NSIS installer from `src-tauri/target/release/bundle/nsis/*.exe`, and tagged releases attach that installer to the GitHub release.

## verification

current local verification:

| check | status |
| --- | --- |
| frontend build | `pnpm build` passes |
| rust backend | `cargo check` passes |
| rust tests | `cargo test` passes |
| linux bundles | `pnpm tauri build` passes for deb, rpm, and AppImage |
| windows installer | configured in GitHub Actions with `windows-latest` and NSIS `.exe` artifact upload |
| release binary smoke test | starts under `xvfb-run` and stays alive until timeout |
| rendered UI smoke test | checked with `agent-browser` at `http://localhost:1420/` |

## license

[mit license](LICENSE)
