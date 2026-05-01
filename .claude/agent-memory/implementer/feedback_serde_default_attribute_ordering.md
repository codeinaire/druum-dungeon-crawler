---
name: serde attribute ordering — #[serde(default)] must follow #[derive(...)]
description: Placing #[serde(default)] before #[derive(...)] triggers legacy_derive_helpers warning (will become hard error); correct order is derive first, then serde attribute
type: feedback
---

In Rust, proc-macro helper attributes like `#[serde(default)]` must appear AFTER the `#[derive(...)]` that introduces the helper, not before it.

**Why:** Placing `#[serde(default)]` before `#[derive(Serialize, Deserialize)]` triggers compiler warning `legacy_derive_helpers` (E0777), noting "this was previously accepted by the compiler but is being phased out; it will become a hard error in a future release." The serde attribute is a *derive helper* and can only be interpreted after the derive macro that registers it is seen.

**How to apply:** Always write:
```rust
#[derive(Serialize, Deserialize, Default, Debug, Clone)]
#[serde(default)]
pub struct MyStruct { ... }
```
Never:
```rust
#[serde(default)]   // WRONG — before the derive
#[derive(Serialize, Deserialize)]
pub struct MyStruct { ... }
```
This applies to all serde container attributes: `#[serde(rename_all = "...")]`, `#[serde(tag = "...")]`, etc.
