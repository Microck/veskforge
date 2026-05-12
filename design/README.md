# veskforge design notes

Use generated bitmap assets as the visual starting point instead of designing the interface from a blank page.

- Light logo source: `assets/veskforge-logo-light.svg`
- Dark logo source: `assets/veskforge-logo-dark.svg`
- UI draft source: `assets/veskforge-ui-draft.png`
- Generator: `egaki`
- Model: `gpt-image-2`

Before replacing these assets, verify Egaki auth with:

```sh
egaki login --show
egaki models
```

The UI should stay practical and dense: left navigation, plugin list, build controls, apply settings, and a visible log panel. Avoid a landing page or decorative marketing layout.
