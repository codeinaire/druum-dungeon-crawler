---
name: Bevy 0.18 Reflect derive support for common type shapes
description: Verified that #[derive(Reflect)] auto-derives FromReflect/TypePath/GetTypeRegistration and recursively works on enums (unit/tuple/struct variants), Vec<Vec<T>>, Option<T>, primitives, and tuples — no #[reflect(...)] attributes needed for druum's data shapes
type: reference
---

For Druum's RON-loaded asset types (Feature #4+), `#[derive(Reflect)]` Just Works on the common Rust shapes. Verified directly against `/Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_reflect-0.18.1/`:

**What auto-derives via `#[derive(Reflect)]`:**
- `Reflect` itself (the supertrait)
- `PartialReflect` (supertrait of Reflect)
- `Struct`, `TupleStruct`, or `Enum` depending on the Rust shape
- `TypePath` (which Asset requires; lib.rs:449 explicitly says "[`TypePath`] is largely used for diagnostic purposes, and should almost always be implemented by deriving [`Reflect`] on your type")
- `Typed`
- `GetTypeRegistration`
- `FromReflect` (auto-derived unless suppressed via `#[reflect(from_reflect = false)]`; see `lib.rs:267-268`)

**Verified working shapes:**
- Enum with mixed variant kinds (`A`, `B(usize, i32)`, `C { foo: f32, bar: bool }`) — see `enums/mod.rs:11-87` test
- `Vec<T>` — blanket impl at `impls/alloc/vec.rs:10-20` (`impl_reflect_for_veclike!` + `impl_type_path!`)
- `Vec<Vec<T>>` — recursive: `T = Vec<U>` reflects when `U` reflects
- `Option<T>` — `impls/core/option.rs`
- Primitives (`u32`, `f32`, `bool`, etc.) — opaque types, see `impls/core/primitives.rs`
- `String` — `impls/alloc/string.rs`
- Tuples like `(u32, u32, Direction)` — `tuple.rs`

**Asset trait requirement:** `bevy_asset-0.18.1/src/lib.rs:456` defines `pub trait Asset: VisitAssetDependencies + TypePath + Send + Sync + 'static {}`. `#[derive(Asset)]` provides `VisitAssetDependencies`; `#[derive(Reflect)]` provides `TypePath`. Together they meet the trait bounds.

**How to apply:**
- For RON-loaded asset types in Druum: use `#[derive(Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone)]` on the top-level type, and `#[derive(Reflect, Serialize, Deserialize, Default, Debug, Clone)]` on contained types (no Asset needed unless they need their own Handle).
- Add `PartialEq` when an integration test needs to assert struct equality (Eq blocked by `f32`).
- DO NOT add `TypePath` derive separately — it's already provided by Reflect, and double-derives cause compile errors.
- Be aware `FromReflect` is auto-derived; if a future feature suppresses it via `#[reflect(from_reflect = false)]`, that breaks dynamic deserialization for that type.

This was verified against Feature #4 research (2026-05-01); applies as-is unless Bevy 0.19+ changes the Reflect derive macros (check `bevy_reflect_derive` source at upgrade time).
