# Plugin Source Formats

veskforge targets Vencord userplugins for Vesktop. It does not install arbitrary Discord client-mod plugins. See [Discord plugin formats](discord-plugin-formats.md) for the compatibility survey behind that boundary.

## Supported

| Source | Accepted shape | Notes |
| --- | --- | --- |
| Single file | `.ts`, `.tsx`, `.js`, `.jsx` | Must be a Vencord module with a default export. |
| Folder | `index.ts`, `index.tsx`, `index.js`, or `index.jsx` | Supporting files such as components, utilities, CSS, and native modules may live beside the entrypoint. |
| GitHub repo | `https://github.com/owner/repo` | veskforge clones the repo and auto-detects one plugin folder. |

GitHub repos are accepted only when veskforge can find exactly one Vencord plugin entrypoint at the repository root or in a subfolder. If none or multiple are found, the source is rejected so the build does not fail later with a vague module-resolution error.

## Rejected

| Source | Why |
| --- | --- |
| BetterDiscord `.plugin.js` | BetterDiscord plugins use a different CommonJS/class plugin API and are not Vencord `definePlugin` modules. |
| GitHub `blob` / `tree` URLs | These are web UI URLs, not repository roots. |
| `raw.githubusercontent.com` URLs | Raw files are not Git repositories. Use a local file source for a downloaded file. |
| SSH Git URLs | veskforge intentionally uses HTTPS GitHub repo URLs for predictable validation and cloning. |
| Plain CSS themes | Themes are not Vencord plugins. |

## Evidence

- Official Vencord custom-plugin docs describe userplugins as a `.ts` / `.tsx` file or a folder containing `index.ts` / `index.tsx`.
- Upstream Vencord `scripts/build/common.mjs` scans `src/userplugins`, imports each top-level plugin entry, and uses esbuild to resolve the imported module.
- Upstream Vencord plugins commonly use `index.ts` and `index.tsx`, plus supporting files such as `style.css`, `styles.css`, `native.ts`, settings modules, and component files.
