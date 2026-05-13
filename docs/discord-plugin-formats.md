# Discord Plugin Formats

veskforge is intentionally narrow: it manages Vencord userplugins for Vesktop. The Discord modding ecosystem has several incompatible plugin formats that can all look like "Discord plugins" from a user's point of view.

## Format Survey

| Ecosystem | Typical shape | Runtime/API | veskforge support |
| --- | --- | --- | --- |
| Vencord / Vesktop custom userplugins | single `.ts` / `.tsx` source file, or folder with `index.ts` / `index.tsx`; practical source modules can also be `.js` / `.jsx` when they use ESM and Vencord APIs | compiled into Vencord through `src/userplugins` and bundled by Vencord's build | supported |
| Equicord-style userplugins | folder with `index.ts` / `index.tsx`, often targeting Vencord-compatible APIs plus Equicord-only APIs | Equicord/Vencord fork build system | rejected unless the plugin is also valid against upstream Vencord |
| BetterDiscord | single `*.plugin.js` file with JSDoc metadata and `module.exports` class/function | loaded at runtime by BetterDiscord with `BdApi` | rejected |
| Replugged | folder/package with `manifest.json` plus built JS entrypoint; plugins may be distributed as packages/archives | Replugged loader and plugin APIs | rejected |
| Powercord | folder with JS entrypoint and `manifest.json` / `powercord_manifest.json`; project is effectively legacy/EOL | Powercord loader and APIs | rejected |
| shelter | plugin package with `plugin.json` and JS/TS/Solid source built for shelter | shelter loader and APIs | rejected |
| GooseMod | module folder with `goosemodModule.json` metadata and an ESM main file exporting GooseMod handlers | GooseMod module store/loader | rejected |
| Vendetta / Bunny / Revenge / Pyoncord | mobile plugin package with `manifest.json` and a `main` entrypoint, commonly TypeScript/JavaScript for Discord's React Native mobile app | mobile Vendetta-family loaders and APIs | rejected |
| Enmity | mobile plugin package distributed through Enmity repositories and targeting Enmity's mobile API | Enmity iOS/Android client mod loader | rejected |
| Aliucord | Android plugin source in Kotlin/Java, built through Gradle into a zipped/dex plugin artifact | Aliucord Android runtime | rejected |
| Themes/CSS | `.theme.css`, `.css`, or mod-specific theme manifests | styling systems, not Vencord plugin modules | rejected |

## Compatibility Rule

veskforge accepts only Vencord-compatible source modules because Vesktop loads a Vencord desktop bundle through the `vencordDir` setting. It does not run BetterDiscord, Replugged, Powercord, shelter, GooseMod, Vendetta-family, Enmity, Aliucord, or theme loaders.

Accepted sources must resolve to exactly one Vencord plugin entrypoint:

- local file: `.ts`, `.tsx`, `.js`, or `.jsx`
- local folder: `index.ts`, `index.tsx`, `index.js`, or `index.jsx`
- GitHub repository root: `https://github.com/owner/repo`

The entrypoint must contain a default export. This conservative check rejects CommonJS BetterDiscord plugins such as `module.exports = class Plugin {}` before they can produce a vague Vencord build error.

## GitHub Source Handling

GitHub inputs are repository roots only. veskforge rejects `blob`, `tree`, raw file, and SSH URLs because those inputs cannot be reliably cloned and inspected.

When a GitHub repository is added, veskforge clones or fetches it immediately and searches for one Vencord plugin folder. The source is accepted only if discovery finds exactly one compatible entrypoint. No match and multiple matches are both hard errors with user-facing messages.

## Sources Checked

- Vencord docs: custom plugins are installed under `src/userplugins` as `.ts` / `.tsx` files or folders with `index.ts` / `index.tsx`.
- Vencord source: the build imports plugin entries from `src/userplugins`; esbuild resolves JS/JSX modules when the source is valid ESM.
- Equicord docs: userplugins use Vencord-like `src/userplugins` folders with `index.ts` / `index.tsx`, but Equicord-specific plugins can depend on fork-only APIs.
- BetterDiscord docs: plugins are JavaScript `*.plugin.js` files exported through `module.exports` and use BetterDiscord's plugin lifecycle/API.
- Replugged developer guide: plugins use a `manifest.json` that points at the addon's entrypoint.
- Powercord examples and docs: plugins use JS entrypoints plus manifest files such as `manifest.json` / `powercord_manifest.json`; Powercord itself is marked EOL upstream.
- shelter docs: plugins use a `plugin.json` metadata file and shelter-specific APIs.
- GooseMod module docs: modules use `goosemodModule.json` metadata and an exported ESM object with GooseMod handlers.
- Vendetta-family plugin docs: mobile plugins use a manifest with a `main` entrypoint and run against Vendetta/Bunny/Revenge/Pyoncord mobile APIs.
- Enmity repositories/docs: mobile plugins are distributed through Enmity plugin repositories and target Enmity's mobile API, not Vencord.
- Aliucord docs/templates: plugins are Android Java/Kotlin projects built through Aliucord tooling, not desktop Vencord modules.
