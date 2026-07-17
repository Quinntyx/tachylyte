# Obsidian 1.12.7 parity ledger

This is an evidence ledger, not a compatibility claim. “Implemented” means the
public Rust API is exercised by `tachylyte-acceptance`; it does **not** mean the
desktop UI or every Obsidian edge case is identical. “Model-only” means a
deterministic data model/planner exists, while “Missing” means this wave has no
corresponding implementation.

## Core plugins

| Obsidian 1.12.7 core plugin | Status | Evidence |
|---|---|---|
| File explorer | Implemented | `vault_scan_read_write_rename_and_trash_are_composable` |
| Global search | Implemented | `index_search_backlinks_and_graph_share_link_resolution` |
| Quick switcher | Model-only | `tachylyte-knowledge::quick_switch`; no acceptance UI |
| Graph view | Implemented | `index_search_backlinks_and_graph_share_link_resolution` |
| Backlinks | Implemented | same test, `backlinks` public API |
| Outgoing links | Implemented | same test, `links` public API |
| Tags view | Model-only | `tachylyte-knowledge::tag_counts`; no acceptance view |
| Properties view | Model-only | `Document.properties` and `property_counts`; no acceptance view |
| Page preview | Model-only | `tachylyte-knowledge::preview`; no hover UI |
| Daily notes | Implemented | `daily_template_and_recovery_plans_apply_through_safe_adapter` |
| Templates | Implemented | same test, `render_template` |
| Note composer | Model-only | `tachylyte-workflows::compose_note`; not exercised in this fixture |
| Command palette | Model-only | `tachylyte-knowledge::rank_commands`; no desktop palette |
| Slash commands | Model-only | `tachylyte-workflows::rank_slash_commands`; no editor integration |
| Canvas | Implemented | `canvas_and_base_fixtures_round_trip_with_extensions` |
| Bookmarks | Model-only | `tachylyte-knowledge::Bookmark` roundtrip unit coverage; no acceptance UI |
| Workspaces | Implemented | `workspace_layout_roundtrip_and_feature_disable_are_observable` |
| File recovery | Implemented | `daily_template_and_recovery_plans_apply_through_safe_adapter`; retention plan applied by adapter |
| Audio recorder | Model-only | `tachylyte-workflows::audio_start` / transition tests; no capture device |
| Unique note creator | Model-only | `tachylyte-workflows::unique_note_plan`; no acceptance flow |
| Random note | Model-only | `tachylyte-knowledge::random_note`; deterministic selection only |
| Outline | Implemented | `markdown_edit_save_and_reparse_preserves_semantics` |
| Word count | Model-only | `tachylyte-workflows::word_status`; no status-bar UI |
| Slides | Model-only | `tachylyte-workflows::parse_slides`; no presentation window |
| Markdown importer | Missing | No importer/parser for foreign formats in this wave |
| PDF viewer | Missing | PDFs are scanned as files, but no viewer/rendering API |
| Sync | Model-only | `tachylyte-services::sync` version vectors and conflict policy; no remote transport |
| Publish | Model-only | `tachylyte-services::publish::diff`; no hosted publishing transport |
| Web viewer | Model-only | `auth_and_url_boundaries_remain_offline_and_redacted` tests URL policy only |
| Bases | Implemented | `canvas_and_base_fixtures_round_trip_with_extensions` (roundtrip/query model; not a full 1:1 Bases UI) |

## Major desktop capabilities

| Capability | Status | Evidence / limitation |
|---|---|---|
| Local vault open, scan, read and safe mutation | Implemented | `vault_scan_read_write_rename_and_trash_are_composable`; Linux capability-checked vault API |
| Markdown source/live-preview/reading modes | Model-only | `tachylyte-markdown::ViewMode`; acceptance verifies source parse/edit/save only |
| Markdown editing, undo and reparse | Implemented | `markdown_edit_save_and_reparse_preserves_semantics` |
| Wiki-links, embeds, headings, tags, frontmatter and tasks | Implemented | Markdown fixture plus parse assertions; embeds are API-level only here |
| Search query language and ranked snippets | Implemented | `index_search_backlinks_and_graph_share_link_resolution` |
| Backlink/outgoing-link resolution and unresolved graph nodes | Implemented | same acceptance test; graph rendering is not included |
| Canvas node geometry, edges and extension preservation | Implemented | `canvas_and_base_fixtures_round_trip_with_extensions` |
| Bases records/formulas/filter/sort | Model-only | Structured crate public APIs and its tests; acceptance checks YAML fidelity only |
| Workspace split/tab/popout/sidebar layout | Model-only | Workspace reducer APIs; acceptance checks roundtrip/feature disable, not GPUI interaction |
| Themes, appearance and CSS customization | Model-only | Workspace appearance data model; no CSS/theme loader or renderer |
| Feature toggles and disabled commands/views | Implemented | `workspace_layout_roundtrip_and_feature_disable_are_observable` |
| Commands, hotkeys and menus | Model-only | Workspace reducer models; no OS/global shortcut registration |
| Auth/session secret redaction | Implemented | `auth_and_url_boundaries_remain_offline_and_redacted` |
| URL navigation allow-list and offline boundary | Implemented | same test; no network client is intentionally present |
| Sync, Publish, update checks and telemetry transport | Model-only | Service contracts/plans only; no credentials, network, or hosted service |
| OS notifications, clipboard, drag/drop, printing and media capture | Missing | No platform adapter in this test-only crate |
| Plugin installation, execution and third-party sandbox | Missing | Core feature registry is not a JavaScript plugin runtime |

## Acceptance fixture inventory

`fixtures/Home.md` and `fixtures/Projects/River.md` provide frontmatter, tags,
tasks, headings and links. `fixtures/Planning.canvas` includes a file node,
edge, and unknown extension fields. `fixtures/Projects.base` includes a view,
sort configuration, and an unknown view key to make roundtrip fidelity visible.
