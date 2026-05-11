---
name: init_asset::<T>() requires AssetPlugin — add to MinimalPlugins test apps
description: App::init_asset::<T>() panics at runtime unless AssetPlugin was added; MinimalPlugins omits it
type: feedback
---

`App::init_asset::<T>()` calls into Bevy's asset system internals and will panic at app.update() if `AssetPlugin` was not added first.

**Fix:** When a test app needs `init_asset::<SomeType>()`, add `AssetPlugin::default()` to the plugin list:

```rust
app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default(), StatesPlugin));
app.init_asset::<TownServices>();  // now safe
```

**Why:** `MinimalPlugins` omits the asset pipeline to keep tests lightweight. `AssetPlugin` bootstraps the `Assets<T>` resource storage that `init_asset` writes into.

**How to apply:** Any test that calls `init_asset::<T>()` OR inserts resources into `Assets<T>` (e.g., `app.world_mut().resource_mut::<Assets<TownServices>>().add(...)`) requires `AssetPlugin`. Add it defensively whenever the test touches asset storage.
