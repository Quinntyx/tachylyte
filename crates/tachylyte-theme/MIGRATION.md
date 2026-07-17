# Theme migration guide

This guide maps the remaining literal colors to the compiled semantic palette
in `tachylyte-theme`. The examples are representative current values, not a
request to preserve the old dark-only appearance.

The light constants were checked against the supplied Obsidian 1.12.7 source at
`/home/henry/Documents/src/tachylyte/main/app.asar.unpacked/obsidian.asar.unpacked/app.css`
(the `--accent-*`, `--color-base-*`, semantic color, background, text, and
interactive mappings around lines 2818–2903). In particular, `base00..100` are
`#ffffff`, `#fcfcfc`, `#fafafa`, `#f6f6f6`, `#e3e3e3`, `#e0e0e0`, `#d4d4d4`,
`#bdbdbd`, `#ababab`, `#707070`, `#5c5c5c`, and `#222222`; packed values are
converted to GPUI's normalized `Hsla`, not interpreted as CSS strings.

## Token mapping

| Surface | Existing literals (representative) | Use these semantic tokens |
| --- | --- | --- |
| `crates/tachylyte-app/src/lib.rs` shell | `0xf7f7f7ff` / `0x1e1e1eff` app background; `0xffffffff` / `0x252526ff` panels; `0x242424ff` / `0xd4d4d4ff` foreground | `background.app` (or `editor.background` for the document area), `sidebar.background` and `sidebar.foreground`, `text.normal` |
| `crates/tachylyte-app/src/lib.rs` file rows and command palette | `0x3b3b3bff` hover; `0x808080ff` palette border | `sidebar.hover` / `interactive.hover`, `modal.background`, `modal.border`, and `borders.default` |
| `crates/tachylyte-editor-ui/src/lib.rs` editor | `0x20242bff` text cells; `0x355070ff` selection; `0x5f6b7aff` cursor | `editor.foreground`, `background.selection` (or `editor.line_highlight` where appropriate), `editor.cursor`; line numbers should use `text.muted` |
| `crates/tachylyte-structured-ui/src/bases.rs` Bases | `0x202124ff` view background; `0x30343bff` toolbar; `0x292d35ff` row; `0x3e5c76ff` active row; white text | `background.app`, `titlebar.background`/`titlebar.foreground`, `sidebar.background`, `interactive.selected` or `sidebar.active`, and `text.on_accent` |
| `crates/tachylyte-structured-ui/src/canvas.rs` canvas | `0x202124ff` viewport; `0x2a2d32ff` grid; `0x3b4252ff` node; `0x88c0d0ff` selected node; `0x8fbcbbff` edges; `0x687080ff` node border | `background.app`, `borders.subtle`, `surface`/`sidebar.background`, `interactive.selected` or `accent`, `accent`, `borders.default`, and `text.on_accent` |
| Navigation surfaces (`tachylyte-app` shell and `crates/tachylyte-workspace/src/lib.rs`) | panel/sidebar literals above; workspace accent `"#7c3aed"` | `sidebar.*` surface tokens, `accent`, `accent_hover`, `accent_active`, `text.link`, and `borders.focus` |

## Migration notes

* Obtain one palette from `AppearanceSettings::tokens(system_is_dark)` (or
  `ThemeKind::tokens`) and pass token values into GPUI styling; do not recreate
  light/dark branches at each call site.
* Prefer the surface group matching the semantic role (`sidebar`, `modal`,
  `editor`, `titlebar`, `status`, or `launcher`) over `base00`–`base100`.
  Base scales remain useful for genuinely unclassified primitives.
* Theme use is native Rust/GPUI styling. There is no CSS runtime, stylesheet
  loader, or CSS variable bridge involved; migration means replacing literals
  with compiled `Palette` fields.
* Preserve alpha for overlays by using `background.selection` or the relevant
  `interactive` token rather than converting translucent colors to opaque RGB.
