# Druum Asset Layout

Each top-level subfolder corresponds to one typed RON asset family registered via `bevy_common_assets::ron::RonAssetPlugin` in `src/plugins/loading/mod.rs`.

| Folder | Extension | Type | Registered via |
|--------|-----------|------|------------|
| `dungeons/` | `.dungeon.ron` | `crate::data::DungeonFloor` | `RonAssetPlugin::<DungeonFloor>::new(&["dungeon.ron"])` |
| `items/` | `.items.ron` | `crate::data::ItemDb` | `RonAssetPlugin::<ItemDb>::new(&["items.ron"])` |
| `enemies/` | `.enemies.ron` | `crate::data::EnemyDb` | `RonAssetPlugin::<EnemyDb>::new(&["enemies.ron"])` |
| `classes/` | `.classes.ron` | `crate::data::ClassTable` | `RonAssetPlugin::<ClassTable>::new(&["classes.ron"])` |
| `spells/` | `.spells.ron` | `crate::data::SpellTable` | `RonAssetPlugin::<SpellTable>::new(&["spells.ron"])` |

Each multi-dot extension dispatches to a unique `Asset` type. Plain `.ron` is **not** registered to any type — using a bare `.ron` extension on a file under one of these folders would produce a `"No AssetLoader found"` warning at runtime.

## Hot-reload

When running `cargo run --features dev`, edits to any file under `assets/` trigger a re-load via `bevy/file_watcher`. Two pieces are required for hot-reload to work:

1. `bevy/file_watcher` listed under the `dev` Cargo feature in `Cargo.toml`.
2. `AssetPlugin { watch_for_changes_override: Some(cfg!(feature = "dev")), ..default() }` set in `src/main.rs`.

Edits while in `GameState::Loading` re-poll the asset collection. Edits during gameplay are picked up by `AssetEvent<T>::Modified` — note this is a `Message` in Bevy 0.18, not an `Event`, so reading it directly requires `MessageReader<AssetEvent<T>>`.

## Adding a new asset family

1. Define the struct in `src/data/<name>.rs` with `#[derive(Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone)]`.
2. Add `RonAssetPlugin::<NewType>::new(&["new.ron"])` to `LoadingPlugin::build` in `src/plugins/loading/mod.rs`.
3. Add a field to `DungeonAssets` with `#[asset(path = "...")]`.
4. Add at least one placeholder RON file under `assets/<folder>/`.
5. Add a round-trip test in `src/data/<name>.rs` (pattern: see `src/data/dungeon.rs`).
6. Re-export the new type from `src/data/mod.rs` (`pub use <name>::NewType;`).

## Security note

Bevy's `AssetPlugin::default()` sets `unapproved_path_mode = UnapprovedPathMode::Forbid`, which blocks loads from outside this `assets/` folder. **Do not change this default.** Loading RON from user-supplied paths (e.g. mod files, save imports) opens path-traversal and parser-DoS risks.

## Trust model

The RON files in this directory are fixed at build time and shipped with the binary. They are trusted input. If future features (e.g. dungeon editor, mod support) load RON from outside `assets/`, they must add validation: depth/size limits on the parser, allow-list checks on paths, and an explicit re-evaluation of the trust boundary.
