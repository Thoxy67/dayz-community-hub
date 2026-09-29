# Changelog

What changed in each release, newest first. A section written here under
`## x.y.z` before `make publish` becomes that release's notes and the update's
notes; without one, the notes are the commit titles since the last release.

## Unreleased

- **A new interface.** Every screen is redrawn on a new component kit (bits-ui,
  tailwind-variants) with a side navigation, a denser server list and a ping
  shown as a signal meter. The themes and the theme editor carry over.
- **Translations** move to intlayer, one dictionary per screen, in the same
  five languages.
- **Typed calls between the interface and the app**: the commands' TypeScript
  bindings are generated from the Rust code (tauri-specta) instead of being
  written by hand.
- **Releases are built and published from one command**, `make publish`, which
  signs both platforms and updates the updater's manifest in one place (see
  docs/build.md).
