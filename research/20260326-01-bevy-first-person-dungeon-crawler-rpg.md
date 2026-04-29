# First-Person Dungeon Crawler RPG in Bevy - Research

**Researched:** 2026-03-26
**Domain:** Rust game development / Bevy engine / First-person grid-based dungeon crawler RPG
**Confidence:** MEDIUM -- Bevy ecosystem moves fast; plugin compatibility is verified at time of writing but may shift within months.

## Summary

Building a first-person grid-based dungeon crawler RPG (in the tradition of Wizardry, Etrian Odyssey, Legend of Grimrock, and similar titles) in Bevy is feasible but demands deliberate architectural choices. Bevy 0.18.1 (released 2026-03-04) is the current stable release, offering a mature ECS, a capable 3D renderer with PBR, fog, and atmosphere systems, and a growing UI toolkit. The engine's plugin architecture maps well to the modular subsystems a DRPG needs (dungeon, combat, party, inventory, UI), and its data-driven ECS is a natural fit for RPG stat systems, status effects, and turn-based action queues.

The primary tension is between Bevy's power and its instability: the engine ships breaking changes roughly every 3 months, the built-in UI system is still primitive for the menu-heavy screens this genre requires, and there is no official editor. These are manageable risks for a project that pins to a specific Bevy version and uses `bevy_egui` for complex UI, but they set the ceiling on iteration speed compared to Unity or Godot.

Three rendering approaches are viable -- pure 2D sprite-layered views, 3D dungeon geometry with billboard enemy sprites, or full 3D. The 3D-with-billboards hybrid (Option B) is recommended: it gives the atmospheric dungeon corridors and lighting that modern players expect while keeping content creation tractable (2D art for enemies and NPCs rather than 3D models). Bevy's PBR renderer, fog systems, and `bevy_sprite3d` make this approach well-supported.

**Primary recommendation:** Target Bevy 0.18.x, use a 3D dungeon geometry + billboard sprite hybrid renderer, structure the project as a plugin-per-subsystem architecture, use `bevy_egui` for all menu/status/inventory UI, and represent dungeons as custom RON-serialized grid data with per-cell wall bitmasks.

---

## Genre Analysis: What Makes a Dungeon Crawler

### Core Gameplay Loop

```
Town Hub (rest, shop, save)
    |
    v
Dungeon Exploration (grid movement, mapping, resource management)
    |
    v
Encounters (random battles, FOEs, boss fights)
    |
    v
Turn-Based Combat (party actions, front/back row, skills, items)
    |
    v
Rewards (XP, loot, progression)
    |
    v
Return to Town or Push Deeper
```

### Essential Mechanical Systems

| System | Description | Key Examples |
|--------|-------------|--------------|
| **Grid Movement** | Tile-by-tile, 4-directional facing, 90-degree turns | All games in this genre |
| **Party Management** | 5-6 characters, front/back row, class/role assignments | Wizardry (6), Etrian Odyssey (5), Grimrock (4) |
| **Turn-Based Combat** | Action selection per character, initiative ordering, status effects | Wizardry, Etrian Odyssey, Undernauts |
| **Character Creation** | Race, class, stat allocation, skill trees | Wizardry, Etrian Odyssey |
| **Dungeon Design** | Multi-floor, secret walls, traps, doors, teleporters, spinners, dark zones, anti-magic zones | Wizardry (spinners, dark zones), Etrian Odyssey (FOEs), Grimrock (puzzles) |
| **Auto-mapping** | Automatic map reveal as player explores; classic mode hides map | Etrian Odyssey (draw-your-own), Wizardry (auto-map) |
| **Town/Hub** | Shops, inn (rest/heal), temple (revive), guild (manage party) | All games |
| **Encounter Types** | Random encounters (step-based), FOEs/visible enemies, scripted boss fights | Etrian Odyssey (FOE = Field On Enemy, visible mini-bosses that move on the map) |
| **Loot & Equipment** | Weapons, armor, accessories, consumables, crafting | All games; varies in depth |

### What Differentiates Modern Takes

| Feature | Classic (Wizardry 1981) | Modern (Etrian Odyssey, Undernauts, 2024 Wizardry Remake) |
|---------|------------------------|----------------------------------------------------------|
| Mapping | Player draws on paper | Auto-map with fog-of-war; Etrian Odyssey: draw-on-touchscreen |
| Visuals | Wire-frame or simple sprites | Full 3D environments or high-quality 2D art |
| QoL | Punishing, no safety nets | Auto-battle, fast-forward, quick-save |
| FOEs | All encounters random | Visible powerful enemies on map that move when you move |
| Party Building | Simple class system | Complex subclass trees, skill synergies, team compositions |
| Story | Minimal | Narrative-driven; voiced characters |
| Difficulty | Brutal, permadeath | Multiple difficulty options; permadeath optional |

### Wall Representation: Thin (Razor) vs Thick Walls

Two fundamental approaches to dungeon grid data:

1. **Razor/Thin Walls (Wizardry, Etrian Odyssey style):** Walls exist *between* cells, not as cells themselves. Each cell stores a 4-bit bitmask indicating which sides have walls (N/S/E/W). The player always occupies a cell center. This is the standard for blobber-style dungeon crawlers.

2. **Thick Walls (Grimrock style):** Walls occupy physical grid cells. The playable space is the cells not marked as walls. Better for real-time movement but uses more grid space.

**Recommendation for this project:** Razor walls (thin walls). This is the canonical representation for Wizardry-style games and maps directly to a compact data structure.

---

## Standard Stack

### Core

| Library | Version | Purpose | License | Maintained? | Why Standard |
|---------|---------|---------|---------|-------------|--------------|
| [bevy](https://crates.io/crates/bevy) | 0.18.1 | Game engine: ECS, renderer, audio, assets, state management | MIT/Apache-2.0 | Active (45K stars, 174 contributors for 0.18, Bevy Foundation) | Dominant Rust game engine; most ecosystem support |
| [bevy_egui](https://crates.io/crates/bevy_egui) | 0.39.x | Immediate-mode UI for menus, inventory, character sheets, combat UI | MIT/Apache-2.0 | Active (0.39.0 released 2026-01-14 for Bevy 0.18) | Best option for complex game UI in Bevy; egui is mature and flexible |
| [leafwing-input-manager](https://crates.io/crates/leafwing-input-manager) | 0.18.x | Input mapping: keyboard, gamepad, mouse to game actions | MIT/Apache-2.0 | Active (0.18.0 supports Bevy 0.18) | De-facto standard for Bevy input handling; many-to-many input-action mapping |
| [bevy_kira_audio](https://crates.io/crates/bevy_kira_audio) | 0.25.x | Audio playback: music, SFX, channels, volume control | MIT/Apache-2.0 | Active (0.25.0 released 2026-01-14 for Bevy 0.18) | Richer audio control than built-in bevy_audio; channel system ideal for BGM + SFX |
| [serde](https://crates.io/crates/serde) + [ron](https://crates.io/crates/ron) | 1.x / 0.8.x | Serialization for save files, dungeon data, game config | MIT/Apache-2.0 | Rust ecosystem staples | RON is the Bevy-native format; human-readable and Rust-syntax-like |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| [bevy_common_assets](https://crates.io/crates/bevy_common_assets) | latest | Load custom RON/JSON/TOML structs as Bevy assets | Loading dungeon definitions, enemy tables, item databases from data files |
| [bevy_asset_loader](https://crates.io/crates/bevy_asset_loader) | latest | Declarative asset loading with states | Managing loading screens and ensuring all assets ready before gameplay |
| [bevy_sprite3d](https://crates.io/crates/bevy_sprite3d) | 7.x | 2D sprites in 3D scenes (billboard enemies) | Rendering enemy sprites as billboards in 3D dungeon |
| [pathfinding](https://crates.io/crates/pathfinding) | 4.8 | A*, Dijkstra for grid navigation | FOE/enemy pathfinding on dungeon grid, auto-map path display |
| [noise](https://crates.io/crates/noise) | latest | Perlin/Simplex noise generation | Procedural dungeon generation, terrain variation |
| [rand](https://crates.io/crates/rand) | 0.8.x | Random number generation | Encounter rolls, loot tables, damage variance, proc-gen |
| [moonshine-save](https://crates.io/crates/moonshine-save) | latest | Selective ECS world save/load | Game save system -- saves only marked entities/components |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|-----------|-----------|----------|
| bevy_egui | Bevy native UI (bevy_ui) | Native UI is ECS-integrated but severely limited in widgets; no text input, no scrollable lists, no complex layouts without massive boilerplate. Use native UI only for HUD overlays. |
| bevy_egui | Quill (reactive UI) | Quill is React-like and more modern, but younger and less battle-tested for game UI at this complexity level. Consider for future migration. |
| bevy_kira_audio | bevy_audio (built-in) | Built-in audio works for basic needs but lacks channel control, crossfading, and fine-grained playback management. |
| moonshine-save | bevy_save | bevy_save offers reflection-based migrations for save compatibility across versions; heavier but better for long-lived save files. Choose bevy_save if save-file backwards compatibility is a priority. |
| leafwing-input-manager | bevy_enhanced_input | Enhanced input is Unreal-inspired, observer-based. Good alternative; leafwing is more established in the ecosystem. |
| Bevy (engine) | Fyrox | Fyrox has a visual editor and built-in RPG tutorials, but a much smaller community and ecosystem. If a visual editor is critical, Fyrox is worth evaluating. |
| Bevy (engine) | Godot (gdext Rust bindings) | Godot has a mature editor, UI system, and large community. If Rust-purity isn't a requirement, Godot + Rust bindings may be faster for UI-heavy games. |

### Installation

```bash
# Create new project
cargo init druum
cd druum

# Cargo.toml dependencies (pin exact Bevy version)
# [dependencies]
# bevy = { version = "0.18.1", features = ["3d"] }
# bevy_egui = "0.39"
# leafwing-input-manager = "0.18"
# bevy_kira_audio = "0.25"
# bevy_common_assets = { version = "*", features = ["ron"] }
# bevy_asset_loader = "*"
# bevy_sprite3d = "7"
# serde = { version = "1", features = ["derive"] }
# ron = "0.8"
# pathfinding = "4.8"
# noise = "0.9"
# rand = "0.8"
# moonshine-save = "*"

# Optimize dependencies in dev mode (critical for Bevy performance)
# [profile.dev.package."*"]
# opt-level = 2

# Enable dynamic linking for faster compile times during development
# [features]
# dev = ["bevy/dynamic_linking"]
```

---

## Architecture Options

Three fundamentally different rendering and content approaches for the first-person dungeon view.

| Option | Description | Pros | Cons | Best When |
|--------|-------------|------|------|-----------|
| **A: Pure 2D Sprite Layers** | Pre-rendered/hand-drawn perspective tiles assembled in layers (classic Eye of the Beholder / Wizardry I-V style). No 3D rendering -- dungeon view is composed of 2D sprites at different depth layers. | Simplest rendering; retro aesthetic; small team art-friendly; no 3D modeling needed; very performant | Limited visual dynamism; no real lighting; rigid perspective; significant art asset creation per tile variant; hard to add new wall types | Solo dev or very small team; intentionally retro aesthetic; 2D art skills but no 3D |
| **B: 3D Dungeon + Billboard Sprites (RECOMMENDED)** | 3D geometry for dungeon walls/floors/ceilings with PBR materials. Enemy and NPC sprites rendered as 2D billboards in 3D space (Etrian Odyssey style). Camera locked to grid positions with smooth interpolation. | Atmospheric lighting (fog, point lights, torches); rotation and movement feel immersive; Bevy's 3D renderer handles it well; enemies are 2D art (tractable for small team); modern look with manageable art pipeline | Need 3D wall/floor/ceiling assets (can be simple geometry with tiling textures); more complex camera/movement code; billboard sprite management | Targeting modern dungeon crawler aesthetic; team has some 3D skills or can use simple procedural geometry; wants lighting/atmosphere |
| **C: Full 3D** | Everything in 3D: dungeon geometry, enemy models, character models visible in combat. Legend of Grimrock style. | Most visually impressive; full use of Bevy's 3D renderer; animation possible on all entities | Massive art pipeline (3D models for all enemies, NPCs, items); rigging/animation needed; longest development time; requires 3D artist | Team includes dedicated 3D artist; targeting AAA-indie visual quality; Grimrock-like production values |

**Recommended: Option B -- 3D Dungeon + Billboard Sprites**

This is the sweet spot for dungeon crawlers: the 3D environment provides atmosphere and immersion (torchlight flickering on stone walls, fog in deep corridors) while billboard sprites keep the art pipeline tractable. This is exactly the approach Etrian Odyssey, Undernauts, Mary Skelter, and the 2024 Wizardry remake use. Bevy's PBR renderer, fog system (`DistanceFog`, `VolumetricFog`), and `bevy_sprite3d` crate directly support this architecture.

### Counterarguments

Why someone might NOT choose Option B:

- **"Option A is simpler and I'm a solo dev with 2D art skills."** -- Response: Valid. If retro aesthetics are the goal and 3D feels like overhead, Option A avoids Bevy's 3D complexity entirely. However, Option B with simple procedural geometry (textured quads for walls) is not much more complex than Option A and gives far more visual flexibility. The wall geometry can be as simple as six-sided boxes with tiling textures.

- **"Option C gives better visuals and Grimrock proved it works."** -- Response: Grimrock was built by a team of experienced developers including dedicated 3D artists, and took over a year. For a Bevy project (where engine iteration adds overhead), the 3D content pipeline is the bottleneck, not the rendering. Option B can be upgraded to Option C incrementally by replacing billboard sprites with 3D models.

- **"Bevy's 3D is not mature enough."** -- Response: For static dungeon geometry with basic materials, Bevy 0.18's renderer is more than capable. The concerns about Bevy 3D maturity apply to advanced features (skeletal animation, complex shader graphs), not to textured boxes and point lights.

---

## Architecture Patterns

### Recommended Project Structure

```
src/
    main.rs                    # App setup, plugin registration
    lib.rs                     # Re-exports
    plugins/
        mod.rs
        dungeon/
            mod.rs             # DungeonPlugin
            grid.rs            # Grid data structures, wall bitmasks
            renderer.rs        # 3D mesh generation from grid data
            movement.rs        # Grid-based movement + interpolation
            features.rs        # Doors, traps, teleporters, spinners
            foe.rs             # Visible enemies on map (FOE system)
            minimap.rs         # Auto-map rendering
        combat/
            mod.rs             # CombatPlugin
            turn_manager.rs    # Turn order, action queue, phase transitions
            actions.rs         # Attack, Defend, Spell, Item, Flee
            damage.rs          # Damage calculation, resistances, elements
            status_effects.rs  # Buff/debuff system with duration tracking
            encounter.rs       # Random encounter tables, spawn logic
            rewards.rs         # XP, loot distribution
        party/
            mod.rs             # PartyPlugin
            character.rs       # Character components: stats, class, race
            inventory.rs       # Item management, equipment slots
            skills.rs          # Skill trees, ability learning
            progression.rs     # Level-up, stat growth, class advancement
        town/
            mod.rs             # TownPlugin
            shop.rs            # Buy/sell, item stock
            inn.rs             # Rest/heal
            temple.rs          # Revive, cure status
            guild.rs           # Party management, character creation
        ui/
            mod.rs             # UiPlugin (egui-based)
            combat_ui.rs       # Battle menus, enemy display
            dungeon_hud.rs     # Compass, party HP bars, minimap overlay
            inventory_ui.rs    # Equipment screen, item management
            character_ui.rs    # Stats, skills, class info
            town_ui.rs         # Shop, inn, temple interfaces
            title_screen.rs    # Main menu, load game
        audio/
            mod.rs             # AudioPlugin wrapper
            bgm.rs             # Background music management
            sfx.rs             # Sound effect triggers
        save/
            mod.rs             # SavePlugin
            save_data.rs       # Saveable component markers
            serialization.rs   # Custom save/load logic
    data/
        mod.rs                 # Data definitions
        enemies.rs             # Enemy stat tables
        items.rs               # Item definitions
        spells.rs              # Spell definitions
        classes.rs             # Class definitions, growth tables
assets/
    dungeons/                  # .dungeon.ron files
    enemies/                   # Enemy sprite sheets
    ui/                        # UI textures, icons
    textures/                  # Wall, floor, ceiling textures
    audio/
        bgm/                   # Background music
        sfx/                   # Sound effects
    fonts/                     # UI fonts
```

### Pattern 1: Game State Machine with SubStates

Bevy's built-in `States` and `SubStates` map directly to a DRPG's flow.

```rust
// Source: Bevy 0.18 States documentation
// https://docs.rs/bevy/latest/bevy/state/index.html

use bevy::prelude::*;

#[derive(States, Default, Debug, Clone, PartialEq, Eq, Hash)]
pub enum GameState {
    #[default]
    Loading,
    TitleScreen,
    Town,
    Dungeon,
    Combat,
    GameOver,
}

// SubStates only exist when parent state is active
#[derive(SubStates, Default, Debug, Clone, PartialEq, Eq, Hash)]
#[source(GameState = GameState::Dungeon)]
pub enum DungeonSubState {
    #[default]
    Exploring,
    EventDialog,
    Paused,
    Inventory,
    Map,
}

#[derive(SubStates, Default, Debug, Clone, PartialEq, Eq, Hash)]
#[source(GameState = GameState::Combat)]
pub enum CombatPhase {
    #[default]
    PlayerInput,   // Selecting actions for each party member
    ExecuteActions, // Resolving the turn
    EnemyTurn,     // Enemy AI acting
    TurnResult,    // Displaying results, checking for victory/defeat
}

#[derive(SubStates, Default, Debug, Clone, PartialEq, Eq, Hash)]
#[source(GameState = GameState::Town)]
pub enum TownLocation {
    #[default]
    Square,
    Shop,
    Inn,
    Temple,
    Guild,
}

fn setup_states(app: &mut App) {
    app.init_state::<GameState>()
        .add_sub_state::<DungeonSubState>()
        .add_sub_state::<CombatPhase>()
        .add_sub_state::<TownLocation>()
        // Systems run only in specific states
        .add_systems(OnEnter(GameState::Dungeon), spawn_dungeon)
        .add_systems(OnExit(GameState::Dungeon), despawn_dungeon)
        .add_systems(
            Update,
            (
                handle_movement,
                check_encounters,
                update_minimap,
            ).run_if(in_state(DungeonSubState::Exploring)),
        )
        .add_systems(
            Update,
            handle_player_combat_input
                .run_if(in_state(CombatPhase::PlayerInput)),
        )
        .add_systems(
            Update,
            execute_combat_actions
                .run_if(in_state(CombatPhase::ExecuteActions)),
        );
}
```

### Pattern 2: Dungeon Grid Data Structure (Razor Walls)

```rust
// Custom dungeon format -- loaded from .dungeon.ron via bevy_common_assets

use serde::{Deserialize, Serialize};
use bevy::prelude::*;

/// Wall bitmask for each cell: which sides have walls
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct WallMask {
    pub north: WallType,
    pub south: WallType,
    pub east: WallType,
    pub west: WallType,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq)]
pub enum WallType {
    #[default]
    Open,        // No wall -- can walk through
    Solid,       // Standard wall
    Door,        // Door (may require key)
    LockedDoor,  // Requires specific key item
    SecretWall,  // Appears solid until discovered
    OneWay,      // Can only pass from one direction
    Illusory,    // Looks solid but can walk through
}

/// Cell features beyond walls
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CellFeatures {
    pub trap: Option<TrapType>,
    pub teleporter: Option<TeleportTarget>,
    pub spinner: bool,          // Randomly rotates player facing
    pub dark_zone: bool,        // Disables auto-map
    pub anti_magic: bool,       // Disables spells
    pub encounter_rate: f32,    // 0.0 = no encounters, 1.0 = every step
    pub event_id: Option<String>, // Scripted event trigger
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrapType {
    Pit { damage: u32, target_floor: Option<u32> },
    Poison,
    Alarm,  // Triggers encounter
    Teleport(TeleportTarget),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeleportTarget {
    pub floor: u32,
    pub x: u32,
    pub y: u32,
    pub facing: Option<Direction>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Direction {
    North,
    South,
    East,
    West,
}

impl Direction {
    pub fn turn_right(self) -> Self {
        match self {
            Self::North => Self::East,
            Self::East => Self::South,
            Self::South => Self::West,
            Self::West => Self::North,
        }
    }

    pub fn turn_left(self) -> Self {
        match self {
            Self::North => Self::West,
            Self::West => Self::South,
            Self::South => Self::East,
            Self::East => Self::North,
        }
    }

    pub fn reverse(self) -> Self {
        match self {
            Self::North => Self::South,
            Self::South => Self::North,
            Self::East => Self::West,
            Self::West => Self::East,
        }
    }

    /// Returns (dx, dy) offset for this direction
    pub fn offset(self) -> (i32, i32) {
        match self {
            Self::North => (0, -1),
            Self::South => (0, 1),
            Self::East => (1, 0),
            Self::West => (-1, 0),
        }
    }
}

/// A single dungeon floor
#[derive(Debug, Clone, Serialize, Deserialize, Asset, TypePath)]
pub struct DungeonFloor {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub floor_number: u32,
    /// Walls stored as [y][x] grid
    pub walls: Vec<Vec<WallMask>>,
    /// Cell features stored as [y][x] grid
    pub features: Vec<Vec<CellFeatures>>,
    /// Starting position for this floor
    pub entry_point: (u32, u32, Direction),
    /// Encounter table ID for this floor
    pub encounter_table: String,
}

impl DungeonFloor {
    pub fn can_move(&self, x: u32, y: u32, dir: Direction) -> bool {
        if x >= self.width || y >= self.height {
            return false;
        }
        let cell = &self.walls[y as usize][x as usize];
        let wall = match dir {
            Direction::North => cell.north,
            Direction::South => cell.south,
            Direction::East => cell.east,
            Direction::West => cell.west,
        };
        matches!(wall, WallType::Open | WallType::Illusory)
    }
}
```

Example dungeon floor in RON format:

```ron
// assets/dungeons/floor_01.dungeon.ron
DungeonFloor(
    name: "Proving Grounds B1F",
    width: 20,
    height: 20,
    floor_number: 1,
    walls: [
        // Row 0
        [
            (north: Solid, south: Open, east: Open, west: Solid),
            (north: Solid, south: Open, east: Solid, west: Open),
            // ... remaining cells
        ],
        // ... remaining rows
    ],
    features: [
        [
            (encounter_rate: 0.15),
            (encounter_rate: 0.15, trap: Some(Pit(damage: 10, target_floor: None))),
            // ...
        ],
    ],
    entry_point: (1, 1, North),
    encounter_table: "b1f_encounters",
)
```

### Pattern 3: RPG Character Components (ECS)

```rust
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

// --- Core Character Identity ---

#[derive(Component, Serialize, Deserialize, Clone)]
pub struct CharacterName(pub String);

#[derive(Component, Serialize, Deserialize, Clone, Copy)]
pub enum Race {
    Human,
    Elf,
    Dwarf,
    Gnome,
    Hobbit,
}

#[derive(Component, Serialize, Deserialize, Clone, Copy)]
pub enum Class {
    Fighter,
    Mage,
    Priest,
    Thief,
    Bishop,  // Mage + Priest hybrid
    Samurai, // Fighter + Mage hybrid
    Lord,    // Fighter + Priest hybrid
    Ninja,   // All-rounder
}

// --- Stats ---

#[derive(Component, Serialize, Deserialize, Clone)]
pub struct BaseStats {
    pub strength: u16,
    pub intelligence: u16,
    pub piety: u16,
    pub vitality: u16,
    pub agility: u16,
    pub luck: u16,
}

#[derive(Component, Serialize, Deserialize, Clone)]
pub struct DerivedStats {
    pub max_hp: u32,
    pub current_hp: u32,
    pub max_mp: u32,
    pub current_mp: u32,
    pub attack: u32,
    pub defense: u32,
    pub magic_attack: u32,
    pub magic_defense: u32,
    pub speed: u32,
    pub accuracy: u32,
    pub evasion: u32,
}

#[derive(Component, Serialize, Deserialize, Clone)]
pub struct Experience {
    pub level: u32,
    pub current_xp: u64,
    pub xp_to_next_level: u64,
}

// --- Party Position ---

#[derive(Component, Serialize, Deserialize, Clone, Copy, PartialEq)]
pub enum PartyRow {
    Front,
    Back,
}

#[derive(Component, Serialize, Deserialize, Clone)]
pub struct PartySlot(pub usize); // 0-5

// --- Equipment ---

#[derive(Component, Serialize, Deserialize, Clone, Default)]
pub struct Equipment {
    pub weapon: Option<Entity>,
    pub shield: Option<Entity>,
    pub head: Option<Entity>,
    pub body: Option<Entity>,
    pub hands: Option<Entity>,
    pub feet: Option<Entity>,
    pub accessory_1: Option<Entity>,
    pub accessory_2: Option<Entity>,
}

// --- Status Effects ---

#[derive(Component, Serialize, Deserialize, Clone)]
pub struct StatusEffects {
    pub effects: Vec<ActiveEffect>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ActiveEffect {
    pub effect_type: StatusEffectType,
    pub remaining_turns: Option<u32>, // None = permanent until cured
    pub potency: f32,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq)]
pub enum StatusEffectType {
    Poison,
    Paralysis,
    Sleep,
    Silence,
    Blind,
    Confused,
    Stone,
    Dead,
    // Buffs
    AttackUp,
    DefenseUp,
    SpeedUp,
    Regen,
}

// --- Bundle for spawning a party member ---

#[derive(Bundle)]
pub struct PartyMemberBundle {
    pub name: CharacterName,
    pub race: Race,
    pub class: Class,
    pub base_stats: BaseStats,
    pub derived_stats: DerivedStats,
    pub experience: Experience,
    pub party_row: PartyRow,
    pub party_slot: PartySlot,
    pub equipment: Equipment,
    pub status_effects: StatusEffects,
}
```

### Pattern 4: Grid Movement with Smooth Interpolation

```rust
use bevy::prelude::*;
use std::time::Duration;

/// Marker for the player party entity in the dungeon
#[derive(Component)]
pub struct PlayerParty;

/// The logical grid position (source of truth)
#[derive(Component, Clone, Copy)]
pub struct GridPosition {
    pub x: i32,
    pub y: i32,
}

/// The direction the party is facing
#[derive(Component, Clone, Copy)]
pub struct Facing(pub Direction);

/// Smooth movement interpolation state
#[derive(Component)]
pub struct MovementAnimation {
    pub from: Vec3,
    pub to: Vec3,
    pub from_rotation: Quat,
    pub to_rotation: Quat,
    pub timer: Timer,
}

const CELL_SIZE: f32 = 4.0; // World units per grid cell
const MOVE_DURATION: f32 = 0.25; // Seconds to interpolate movement

pub fn grid_to_world(x: i32, y: i32) -> Vec3 {
    Vec3::new(x as f32 * CELL_SIZE, 0.0, y as f32 * CELL_SIZE)
}

pub fn facing_to_rotation(dir: Direction) -> Quat {
    let angle = match dir {
        Direction::North => 0.0_f32,
        Direction::East => -std::f32::consts::FRAC_PI_2,
        Direction::South => std::f32::consts::PI,
        Direction::West => std::f32::consts::FRAC_PI_2,
    };
    Quat::from_rotation_y(angle)
}

/// System: Handle movement input and initiate grid movement
pub fn handle_movement_input(
    mut commands: Commands,
    input: Res<ButtonInput<KeyCode>>,
    dungeon: Res<CurrentDungeonFloor>,
    query: Query<
        (Entity, &GridPosition, &Facing, &Transform),
        (With<PlayerParty>, Without<MovementAnimation>),
    >,
) {
    let Ok((entity, grid_pos, facing, transform)) = query.get_single() else {
        return;
    };

    let mut new_pos = *grid_pos;
    let mut new_facing = facing.0;
    let mut moved = false;

    if input.just_pressed(KeyCode::KeyW) {
        // Move forward
        let (dx, dy) = facing.0.offset();
        if dungeon.floor.can_move(grid_pos.x as u32, grid_pos.y as u32, facing.0) {
            new_pos.x += dx;
            new_pos.y += dy;
            moved = true;
        }
    } else if input.just_pressed(KeyCode::KeyS) {
        // Move backward
        let back = facing.0.reverse();
        let (dx, dy) = back.offset();
        if dungeon.floor.can_move(grid_pos.x as u32, grid_pos.y as u32, back) {
            new_pos.x += dx;
            new_pos.y += dy;
            moved = true;
        }
    } else if input.just_pressed(KeyCode::KeyA) {
        // Strafe left
        let left = facing.0.turn_left();
        let (dx, dy) = left.offset();
        if dungeon.floor.can_move(grid_pos.x as u32, grid_pos.y as u32, left) {
            new_pos.x += dx;
            new_pos.y += dy;
            moved = true;
        }
    } else if input.just_pressed(KeyCode::KeyD) {
        // Strafe right
        let right = facing.0.turn_right();
        let (dx, dy) = right.offset();
        if dungeon.floor.can_move(grid_pos.x as u32, grid_pos.y as u32, right) {
            new_pos.x += dx;
            new_pos.y += dy;
            moved = true;
        }
    } else if input.just_pressed(KeyCode::KeyQ) {
        // Turn left
        new_facing = facing.0.turn_left();
        moved = true;
    } else if input.just_pressed(KeyCode::KeyE) {
        // Turn right
        new_facing = facing.0.turn_right();
        moved = true;
    }

    if moved {
        let target_world = grid_to_world(new_pos.x, new_pos.y);
        let target_rot = facing_to_rotation(new_facing);

        commands.entity(entity).insert((
            GridPosition { x: new_pos.x, y: new_pos.y },
            Facing(new_facing),
            MovementAnimation {
                from: transform.translation,
                to: target_world + Vec3::Y * 1.6, // Eye height
                from_rotation: transform.rotation,
                to_rotation: target_rot,
                timer: Timer::from_seconds(MOVE_DURATION, TimerMode::Once),
            },
        ));
    }
}

/// System: Animate the smooth interpolation
pub fn animate_movement(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut Transform, &mut MovementAnimation)>,
) {
    for (entity, mut transform, mut anim) in &mut query {
        anim.timer.tick(time.delta());
        let t = anim.timer.fraction(); // 0.0 -> 1.0

        // Smooth step interpolation for natural feel
        let t_smooth = t * t * (3.0 - 2.0 * t);

        transform.translation = anim.from.lerp(anim.to, t_smooth);
        transform.rotation = anim.from_rotation.slerp(anim.to_rotation, t_smooth);

        if anim.timer.finished() {
            transform.translation = anim.to;
            transform.rotation = anim.to_rotation;
            commands.entity(entity).remove::<MovementAnimation>();
        }
    }
}
```

### Pattern 5: Turn-Based Combat Action Queue

```rust
use bevy::prelude::*;

/// Represents a single combat action to be resolved
#[derive(Clone, Debug)]
pub struct CombatAction {
    pub actor: Entity,
    pub action_type: ActionType,
    pub speed: u32, // For turn ordering
}

#[derive(Clone, Debug)]
pub enum ActionType {
    Attack { target: TargetSelection },
    Defend,
    CastSpell { spell_id: String, target: TargetSelection },
    UseItem { item_entity: Entity, target: TargetSelection },
    Flee,
}

#[derive(Clone, Debug)]
pub enum TargetSelection {
    SingleEnemy(usize),  // Index in enemy group
    AllEnemies,
    SingleAlly(usize),   // Party slot
    AllAllies,
    Self_,
}

/// Resource: holds the queue of actions for the current turn
#[derive(Resource, Default)]
pub struct TurnActionQueue {
    pub actions: Vec<CombatAction>,
    pub current_index: usize,
}

/// Resource: tracks which party member is selecting an action
#[derive(Resource)]
pub struct PlayerInputState {
    pub current_slot: usize,       // Which party member is choosing
    pub total_party_members: usize,
}

/// System: Collect player input for each party member
pub fn player_combat_input(
    mut input_state: ResMut<PlayerInputState>,
    mut action_queue: ResMut<TurnActionQueue>,
    mut next_phase: ResMut<NextState<CombatPhase>>,
    // ... UI interaction, button presses, etc.
) {
    // After player selects action for current character:
    // action_queue.actions.push(selected_action);
    // input_state.current_slot += 1;

    if input_state.current_slot >= input_state.total_party_members {
        // All party members have chosen -- add enemy actions, sort by speed
        // add_enemy_actions(&mut action_queue);
        action_queue.actions.sort_by(|a, b| b.speed.cmp(&a.speed));
        action_queue.current_index = 0;
        next_phase.set(CombatPhase::ExecuteActions);
    }
}

/// System: Execute actions one by one
pub fn execute_combat_actions(
    mut action_queue: ResMut<TurnActionQueue>,
    mut next_phase: ResMut<NextState<CombatPhase>>,
    // ... entity queries for stats, damage calculation, etc.
) {
    if action_queue.current_index >= action_queue.actions.len() {
        // All actions resolved -- check win/lose, then back to player input
        next_phase.set(CombatPhase::TurnResult);
        return;
    }

    let action = &action_queue.actions[action_queue.current_index];
    // Resolve the action: apply damage, healing, status effects, etc.
    // ... damage calculation systems ...

    action_queue.current_index += 1;
}
```

### Pattern 6: Dungeon Mesh Generation (3D Walls from Grid)

```rust
use bevy::prelude::*;

/// Generates 3D wall meshes for visible cells around the player
pub fn generate_dungeon_geometry(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    dungeon: Res<CurrentDungeonFloor>,
    wall_texture: Res<WallTextures>,
) {
    let cell_size = CELL_SIZE;
    let wall_height = 3.0;

    for y in 0..dungeon.floor.height {
        for x in 0..dungeon.floor.width {
            let walls = &dungeon.floor.walls[y as usize][x as usize];
            let world_x = x as f32 * cell_size;
            let world_z = y as f32 * cell_size;

            // Spawn floor quad
            commands.spawn((
                Mesh3d(meshes.add(
                    Plane3d::default().mesh().size(cell_size, cell_size),
                )),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color_texture: Some(wall_texture.floor.clone()),
                    ..default()
                })),
                Transform::from_xyz(world_x, 0.0, world_z),
                DungeonGeometry, // Marker component for cleanup
            ));

            // Spawn ceiling quad
            commands.spawn((
                Mesh3d(meshes.add(
                    Plane3d::default().mesh().size(cell_size, cell_size),
                )),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color_texture: Some(wall_texture.ceiling.clone()),
                    ..default()
                })),
                Transform::from_xyz(world_x, wall_height, world_z)
                    .with_rotation(Quat::from_rotation_x(std::f32::consts::PI)),
                DungeonGeometry,
            ));

            // Spawn wall segments for each solid wall
            if matches!(walls.north, WallType::Solid | WallType::SecretWall) {
                spawn_wall_segment(
                    &mut commands, &mut meshes, &mut materials,
                    &wall_texture,
                    Vec3::new(world_x, wall_height / 2.0, world_z - cell_size / 2.0),
                    Quat::IDENTITY,
                    cell_size, wall_height,
                );
            }
            // ... repeat for south, east, west walls
            // Doors get different material/mesh with interactable component
        }
    }
}

/// Marker for dungeon geometry entities (for cleanup on floor change)
#[derive(Component)]
pub struct DungeonGeometry;

fn spawn_wall_segment(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    textures: &WallTextures,
    position: Vec3,
    rotation: Quat,
    width: f32,
    height: f32,
) {
    commands.spawn((
        Mesh3d(meshes.add(
            Plane3d::default().mesh().size(width, height),
        )),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color_texture: Some(textures.wall.clone()),
            ..default()
        })),
        Transform::from_translation(position).with_rotation(rotation),
        DungeonGeometry,
    ));
}
```

### Anti-Patterns to Avoid

- **Monolithic game system:** Do NOT put all RPG logic in one massive system. Split into composable systems per concern (damage, status, turn order). Bevy's ECS is designed for many small systems, not few large ones.

- **Storing game state in resources instead of components:** Party member stats, inventory, etc. should be components on entities, not fields in a giant `GameState` resource. This enables Bevy queries, change detection, and save/load.

- **Hard-coding dungeon layouts in Rust code:** Define dungeons as data files (RON) loaded as assets. This enables hot-reloading during development, modding, and tooling.

- **Skipping the movement interpolation:** Instant teleportation between grid cells is disorienting. Always animate transitions, even if brief (200-300ms).

- **Using Bevy's native UI for complex menus:** bevy_ui is inadequate for the complex, nested, text-heavy menus DRPGs need. Use bevy_egui for all complex UI and reserve bevy_ui for simple HUD overlays only.

- **Tight-coupling rendering to game logic:** The grid position (logical) and the 3D transform (visual) should be separate components. The renderer reads grid position to determine what to show; it never modifies game state.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Input mapping / rebinding | Custom key-to-action mapping | `leafwing-input-manager` | Many-to-many input mapping, gamepad support, serializable bindings -- surprisingly complex to get right |
| Audio channel management | Custom audio state machine | `bevy_kira_audio` | Crossfading, per-channel volume, spatial audio, format support |
| Save file serialization | Manual serde of entire world | `moonshine-save` or `bevy_save` | Selective entity saving, handles ECS complexity, avoids serializing render state |
| A* pathfinding on grid | Custom pathfinding | `pathfinding` crate | Battle-tested algorithms, handles edge cases, supports multiple algorithms |
| Immediate-mode GUI | Custom retained-mode UI system | `bevy_egui` | Thousands of widgets, text input, tables, scrolling, layout -- years of development |
| Asset loading orchestration | Manual asset handle tracking | `bevy_asset_loader` | Loading states, progress tracking, dynamic collections -- boilerplate elimination |
| Custom data file loading | Manual serde asset loader | `bevy_common_assets` | One-line RON/JSON asset registration with custom file extensions |
| Random number generation | Custom PRNG | `rand` crate | Cryptographic quality, reproducible seeds for debugging, distributions |
| Billboard sprite rendering | Custom 3D quad management | `bevy_sprite3d` | Handles mesh generation, caching, atlas support, camera-facing |

---

## Common Pitfalls

### Pitfall 1: Bevy Version Churn

**What goes wrong:** You build on Bevy 0.18, and 3 months later 0.19 ships with breaking API changes. Third-party plugins may or may not update promptly. Your codebase is stuck on an old version or requires painful migration.

**Why it happens:** Bevy is pre-1.0 and ships breaking changes every ~3 months. The community has raised [significant concerns](https://github.com/bevyengine/bevy/discussions/21838) about this pace.

**How to avoid:**
- Pin to a specific Bevy version (0.18.1) in Cargo.toml with `=0.18.1`.
- Pin all Bevy plugin versions to their 0.18-compatible releases.
- Do NOT upgrade Bevy mid-development unless a specific new feature is critical.
- Budget 1-2 weeks for migration if you do upgrade.
- Build abstractions over Bevy APIs where possible so migration surface is smaller.

### Pitfall 2: UI Complexity Underestimation

**What goes wrong:** DRPGs need 15+ distinct UI screens (character creation, party management, inventory, equipment, skills, shop, combat menus with target selection, auto-map, quest log, save/load, settings...). Developers start with bevy_ui and hit a wall at the second or third screen.

**Why it happens:** bevy_ui is intentionally minimal. It has no text input, no scrollable containers, no radio buttons (until 0.18), no tables, no tree views. Building these from scratch takes longer than building the game itself.

**How to avoid:**
- Use `bevy_egui` for ALL complex UI from day one. Do not "plan to migrate later."
- Use bevy_ui only for the in-game HUD overlay (HP bars, compass, minimap frame).
- Accept that egui's aesthetic is "programmer art" by default and style it later.
- Consider a UI-first development approach: build all menu screens as interactive prototypes before writing game logic.

### Pitfall 3: No Editor, Painful Content Iteration

**What goes wrong:** Designing 20-floor dungeons by editing RON files by hand is slow and error-prone. Balancing encounters requires play-testing that takes too long to start.

**Why it happens:** Bevy has no editor. Unlike Unity or Godot, you cannot visually place walls, enemies, or items.

**How to avoid:**
- Build a simple dungeon editor as an egui tool within your game (separate binary or debug mode).
- Or use TrenchBroom (Quake map editor) with Bevy glTF extensions.
- Or build a web-based editor that outputs RON files.
- Implement hot-reloading of dungeon RON files during development.
- Add debug teleportation and stat-override commands for testing.

### Pitfall 4: Encounter Rate Tuning

**What goes wrong:** Random encounters are either too frequent (tedious) or too rare (player is underprepared for bosses). This is a game design problem, not a code problem, but it manifests as technical debt when the encounter system isn't configurable.

**Why it happens:** Classic Wizardry used per-step encounter rolls with fixed rates. Modern players expect more nuance.

**How to avoid:**
- Make encounter rates data-driven (per-cell in dungeon definition).
- Implement a step counter with increasing probability (e.g., guaranteed encounter after N steps with no encounter).
- Separate encounter tables by dungeon floor/zone.
- Add difficulty scaling options.
- Consider FOE/visible enemy system (Etrian Odyssey) for boss-tier encounters.

### Pitfall 5: Save System Architectural Debt

**What goes wrong:** Save/load is added late and requires refactoring half the codebase because components weren't designed to be serializable, or render state is mixed with game state.

**Why it happens:** Save systems touch every part of the game. If not planned from the start, serialization annotations and state separation are afterthoughts.

**How to avoid:**
- Design components as serializable from day one (`#[derive(Serialize, Deserialize)]` on all game-state components).
- Use marker components (e.g., `Saveable`) to distinguish what gets saved.
- Keep render-only state (mesh handles, material handles) separate from game state.
- Test save/load with every new system added, not at the end.

### Pitfall 6: RPG Balance is Exponentially Hard

**What goes wrong:** A class system with 8 classes, 6 stats, 50+ skills, and 200+ items creates a combinatorial explosion of balance interactions. One overlooked synergy breaks the game.

**Why it happens:** RPG systems are multiplicative: every new mechanic multiplies the testing surface.

**How to avoid:**
- Start with 3-4 classes and expand only after the core loop is fun.
- Data-drive ALL balance values (damage formulas, growth rates, spell costs) -- never hard-code.
- Build automated combat simulators that run thousands of battles to find outliers.
- Study existing games' formulas: Wizardry and Etrian Odyssey formulas are well-documented by fan communities.

---

## Security

### Known Vulnerabilities

No known CVEs or security advisories found for Bevy or the recommended libraries as of 2026-03-26.

Bevy and its ecosystem crates are dual-licensed MIT/Apache-2.0, which are permissive and impose no restrictions on commercial use.

| Library | CVE / Advisory | Severity | Status | Action |
|---------|---------------|----------|--------|--------|
| bevy 0.18.1 | None found | -- | -- | Monitor via `cargo audit` |
| bevy_egui 0.39.x | None found | -- | -- | Monitor |
| leafwing-input-manager 0.18.x | None found | -- | -- | Monitor |
| bevy_kira_audio 0.25.x | None found | -- | -- | Monitor |
| serde 1.x | None found | -- | -- | Monitor |

### Architectural Security Risks

Since this is a single-player offline game, the attack surface is limited compared to networked applications. However:

| Risk | Affected Architecture | How It Manifests | Secure Pattern | Anti-Pattern |
|------|----------------------|------------------|----------------|--------------|
| Malicious save files | Save/Load system | Crafted save files could exploit deserialization bugs (e.g., RON parser edge cases, excessively deep nesting) | Validate save file structure before deserializing; limit recursion depth; use `ron::Options` with bounds | Blindly deserializing arbitrary files without validation |
| Mod/asset injection | Asset pipeline | If the game loads assets from user-modifiable directories, malicious assets could crash the game or exploit image/audio parsers | Load assets only from known paths; validate asset metadata; use Bevy's built-in asset validation | Loading arbitrary files from uncontrolled paths |
| Save file tampering | Game state | Players editing save files to cheat (not a security risk but a design consideration) | Optionally checksum save files; for competitive features (leaderboards), validate server-side | Trusting save file values without bounds checking |

### Trust Boundaries

- **Save file input:** Validate structure, check version compatibility, bounds-check all numeric values before applying to game state.
- **Asset files (dungeon RON, item databases):** These are developer-created content. If modding is supported, validate schema before loading.
- **No network boundary:** Single-player game has no network trust boundary unless online leaderboards or multiplayer are added later.

---

## Performance

| Metric | Value / Range | Source | Notes |
|--------|--------------|-------|-------|
| Bevy compile time (clean, dev) | 30-90 seconds | Community reports | Use `dynamic_linking` feature and `opt-level = 2` for dependencies to reduce to ~5-10s incremental |
| Bevy compile time (incremental) | 2-8 seconds | Community reports | With dynamic linking enabled |
| 3D scene: static geometry | Handles thousands of textured quads easily | Bevy 0.18 clustered forward rendering | Dungeon geometry is simple; not a bottleneck |
| ECS: entity count | Hundreds of thousands | Bevy benchmarks | A DRPG needs < 1000 entities typically; ECS overhead is negligible |
| egui frame time | < 1ms for complex UIs | egui benchmarks | Immediate-mode rendering is efficient |
| Fog/lighting | DistanceFog is cheap; VolumetricFog has GPU cost | Bevy docs | Use DistanceFog for general atmosphere; reserve VolumetricFog for specific atmospheric areas |
| Bundle size (desktop) | 30-60 MB | Typical Bevy games | Depends on asset count; executable alone is ~15-20MB |

**Performance is NOT a concern for this genre.** Dungeon crawlers render a single room or corridor at a time with a handful of sprites. The ECS manages < 100 active entities during combat. The bottleneck will be content creation and UI implementation, not runtime performance.

---

## Code Examples

### Dungeon Lighting Setup

```rust
// Source: Bevy 0.18 fog and lighting examples
// https://bevy.org/examples/3d-rendering/atmospheric-fog/
// https://bevy.org/examples/3d-rendering/fog/

use bevy::prelude::*;

pub fn setup_dungeon_lighting(mut commands: Commands) {
    // Ambient light -- very dim for dungeon atmosphere
    commands.insert_resource(AmbientLight {
        color: Color::srgb(0.1, 0.1, 0.15),
        brightness: 20.0,
    });

    // Distance fog for depth perception
    // (attached to camera entity)
    // DistanceFog {
    //     color: Color::srgb(0.02, 0.02, 0.04),
    //     falloff: FogFalloff::Exponential { density: 0.15 },
    //     ..default()
    // }
}

/// Spawn a torch light at a grid position
pub fn spawn_torch(
    commands: &mut Commands,
    x: i32, y: i32,
    height: f32,
) {
    let world_pos = grid_to_world(x, y) + Vec3::Y * height;

    commands.spawn((
        PointLight {
            color: Color::srgb(1.0, 0.7, 0.3), // Warm torch light
            intensity: 800.0,
            range: 12.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_translation(world_pos),
        DungeonGeometry, // Cleaned up with dungeon
    ));
}
```

### Encounter System

```rust
use bevy::prelude::*;
use rand::Rng;

#[derive(Resource)]
pub struct EncounterState {
    pub steps_since_last: u32,
    pub base_rate: f32,
}

/// System: Check for random encounter after each step
pub fn check_random_encounter(
    mut encounter: ResMut<EncounterState>,
    mut next_state: ResMut<NextState<GameState>>,
    dungeon: Res<CurrentDungeonFloor>,
    player: Query<&GridPosition, With<PlayerParty>>,
) {
    let Ok(pos) = player.get_single() else { return };
    let cell_features = &dungeon.floor.features[pos.y as usize][pos.x as usize];

    encounter.steps_since_last += 1;

    // Increasing probability: guaranteed encounter after enough steps
    let rate = cell_features.encounter_rate
        * (1.0 + encounter.steps_since_last as f32 * 0.05);

    let mut rng = rand::thread_rng();
    if rng.gen::<f32>() < rate {
        encounter.steps_since_last = 0;
        // Generate encounter from floor's encounter table
        // ... select enemies, initialize combat state ...
        next_state.set(GameState::Combat);
    }
}
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|-------------|-----------------|--------------|--------|
| Bevy `SystemSet` ordering with labels | `.chain()` and explicit ordering | Bevy 0.11+ | Simpler system scheduling syntax |
| `bevy_audio` for all audio | `bevy_kira_audio` for game audio | Ongoing | Richer audio control; Kira is the likely future of Bevy audio |
| Manual asset handle management | `bevy_asset_loader` with loading states | Bevy 0.12+ | Cleaner loading screens and asset organization |
| `ComputedVisibility` | Automatic visibility propagation | Bevy 0.14+ | Simplified entity visibility management |
| bevy_ui + manual styling | bevy_feathers (0.18+) for editor widgets | Bevy 0.18 | New standard widget library, but still not game-UI-ready |
| `Handle::Weak` | `Handle::Uuid` with `uuid_handle!` macro | Bevy 0.17 | Migration required for existing handle patterns |
| Single Bevy crate | Feature-gated sub-crates (`"3d"`, `"ui"`) | Bevy 0.18 | Faster compile times by excluding unused features |

**Deprecated/outdated patterns to avoid:**
- **bevy_asset_ron**: Superseded by `bevy_common_assets` which supports multiple formats.
- **`SystemSet` label-based ordering**: Use `.chain()`, `.before()`, `.after()` directly.
- **Manual `DespawnRecursive`**: Use `commands.entity(e).despawn()` which is now recursive by default.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[cfg(test)]` + `cargo test` |
| Config file | None needed (Cargo.toml test configuration) |
| Quick run command | `cargo test --lib` |
| Full suite command | `cargo test` |

### Requirements to Test Map

| Requirement | Behavior | Test Type | Automated Command | File Exists? |
|-------------|----------|-----------|-------------------|--------------|
| Grid wall collision | `can_move()` returns false for solid walls, true for open | Unit | `cargo test dungeon::grid::tests` | Needs creating |
| Direction operations | `turn_left()`, `turn_right()`, `reverse()`, `offset()` correct | Unit | `cargo test dungeon::grid::tests` | Needs creating |
| Dungeon RON loading | Parse `.dungeon.ron` file into `DungeonFloor` struct | Unit | `cargo test dungeon::grid::tests` | Needs creating |
| Encounter probability | Increasing probability over steps, reset on encounter | Unit | `cargo test combat::encounter::tests` | Needs creating |
| Damage calculation | Damage formula produces expected ranges given stats | Unit | `cargo test combat::damage::tests` | Needs creating |
| Turn ordering | Actions sorted by speed, all executed in order | Unit | `cargo test combat::turn_manager::tests` | Needs creating |
| Status effect duration | Effects tick down and expire correctly | Unit | `cargo test combat::status_effects::tests` | Needs creating |
| Character stat derivation | Derived stats calculated correctly from base stats + equipment | Unit | `cargo test party::character::tests` | Needs creating |
| Save/load round-trip | Save game state, load it back, verify equality | Integration | `cargo test save::tests` | Needs creating |
| Grid movement interpolation | Movement animation completes and lands on correct cell | Integration (Bevy app test) | `cargo test --test movement_integration` | Needs creating |

### Gaps (files to create before implementation)

- [ ] `src/plugins/dungeon/grid.rs` -- `#[cfg(test)] mod tests` section with wall collision, direction, and RON loading tests
- [ ] `src/plugins/combat/damage.rs` -- unit tests for damage formula
- [ ] `src/plugins/combat/turn_manager.rs` -- unit tests for action ordering
- [ ] `src/plugins/combat/status_effects.rs` -- unit tests for effect duration
- [ ] `src/plugins/combat/encounter.rs` -- unit tests for encounter probability
- [ ] `src/plugins/party/character.rs` -- unit tests for stat derivation
- [ ] `tests/save_load_integration.rs` -- integration test for save/load round-trip
- [ ] `tests/movement_integration.rs` -- Bevy app-based integration test for grid movement

**Note:** Bevy systems are testable by constructing a minimal `App`, adding the system, and running it. For unit tests of pure data logic (damage formulas, grid operations), standard Rust tests work without Bevy. For integration tests involving ECS queries and state transitions, use Bevy's `App::update()` to step the simulation.

---

## Open Questions

1. **Dungeon Editor Tooling**
   - What we know: No Bevy editor exists. TrenchBroom, Yoleck, and custom egui tools are options.
   - What's unclear: Which approach gives the best iteration speed for designing 20+ dungeon floors with complex features (spinners, teleporters, secret walls).
   - Recommendation: Build a minimal egui-based dungeon editor as a debug tool within the game early in development. Export to RON. Evaluate whether a standalone web editor is worth the investment after 3-4 floors are designed.

2. **bevy_sprite3d Bevy 0.18 Compatibility**
   - What we know: bevy_sprite3d v7.0.0 was released ~December 2025. The crate is actively maintained.
   - What's unclear: Whether v7.0.0 specifically targets Bevy 0.18.1 or an earlier version.
   - Recommendation: Check the crate's Cargo.toml on crates.io before adding as dependency. If incompatible, billboard sprites can be implemented with raw Bevy 3D quads (a textured quad that always faces the camera).

3. **moonshine-save vs bevy_save for Bevy 0.18**
   - What we know: Both crates exist and provide selective save/load. moonshine-save is lighter but lacks save versioning.
   - What's unclear: Exact Bevy 0.18 compatibility for latest versions of each.
   - Recommendation: If save compatibility across game updates matters, use bevy_save with migrations. For initial development, a simple custom serde-based save system may be simpler than either plugin.

4. **Art Pipeline for Billboard Enemies**
   - What we know: Enemies will be 2D sprite sheets displayed as billboards in 3D.
   - What's unclear: Optimal sprite resolution, animation frame count, and whether to use sprite sheets or individual frames. Also unclear whether enemies should have directional sprites (front/side views) or always face camera.
   - Recommendation: Start with single-facing sprites (always face camera). 256x256 or 512x512 per enemy is typical for the genre. Animated idle + attack + damage frames minimum.

5. **Auto-mapping Implementation**
   - What we know: Auto-maps are essential. Etrian Odyssey uses a draw-your-own approach; most others auto-reveal.
   - What's unclear: Best rendering approach for minimap in Bevy (egui canvas, separate camera rendering to texture, or overlay UI).
   - Recommendation: Start with egui canvas rendering of explored cells. Upgrade to render-to-texture if egui performance is insufficient for large maps.

---

## Sources

### Primary (HIGH confidence)

- [Bevy 0.18 Release Notes](https://bevy.org/news/bevy-0-18/) -- Detailed feature list, rendering improvements, UI additions, API changes. Accessed: 2026-03-26
- [Bevy GitHub Repository](https://github.com/bevyengine/bevy) -- 45K stars, active development, license verification (MIT/Apache-2.0). Accessed: 2026-03-26
- [Bevy States Documentation](https://docs.rs/bevy/latest/bevy/state/index.html) -- SubStates API, state transitions, run conditions. Accessed: 2026-03-26
- [Bevy Fog Example](https://bevy.org/examples/3d-rendering/fog/) -- DistanceFog configuration and usage. Accessed: 2026-03-26
- [Bevy Migration Guide 0.17 to 0.18](https://bevy.org/learn/migration-guides/0-17-to-0-18/) -- Breaking changes documentation. Accessed: 2026-03-26
- [bevy_egui GitHub](https://github.com/vladbat00/bevy_egui) -- v0.39.0 for Bevy 0.18, feature set. Accessed: 2026-03-26
- [leafwing-input-manager GitHub](https://github.com/Leafwing-Studios/leafwing-input-manager) -- v0.18.0 for Bevy 0.18. Accessed: 2026-03-26
- [bevy_kira_audio GitHub](https://github.com/NiklasEi/bevy_kira_audio) -- v0.25.0 for Bevy 0.18. Accessed: 2026-03-26
- [bevy_common_assets GitHub](https://github.com/NiklasEi/bevy_common_assets) -- RON/JSON/TOML asset loading. Accessed: 2026-03-26
- [bevy_sprite3d GitHub](https://github.com/FraserLee/bevy_sprite3d) -- 3D billboard sprites, v7.0.0. Accessed: 2026-03-26
- [bevy_ecs_tilemap GitHub](https://github.com/StarArawn/bevy_ecs_tilemap) -- v0.18.1 for Bevy 0.18. Accessed: 2026-03-26
- [pathfinding crate docs](https://docs.rs/pathfinding/latest/pathfinding/) -- A*, Dijkstra, Grid type. v4.8. Accessed: 2026-03-26

### Secondary (MEDIUM confidence)

- [Unofficial Bevy Cheat Book - States](https://bevy-cheatbook.github.io/programming/states.html) -- Practical state management patterns. Accessed: 2026-03-26
- [Unofficial Bevy Cheat Book - Smooth Movement](https://bevy-cheatbook.github.io/cookbook/smooth-movement.html) -- Transform interpolation patterns. Accessed: 2026-03-26
- [Unofficial Bevy Cheat Book - Performance](https://bevy-cheatbook.github.io/pitfalls/performance.html) -- Dev build optimization tips. Accessed: 2026-03-26
- [Tainted Coders - Bevy Code Organization](https://taintedcoders.com/bevy/code-organization) -- Plugin architecture best practices. Accessed: 2026-03-26
- [Turn Based Patterns Discussion #3370](https://github.com/bevyengine/bevy/discussions/3370) -- Community patterns for turn-based ECS. Published: 2022, Accessed: 2026-03-26 (patterns still applicable)
- [First Person Dungeons Tutorial - Screaming Brain Studios](https://screamingbrainstudios.com/first-person-dungeons/) -- 2D layered dungeon rendering technique. Published: 2022-01-29, Accessed: 2026-03-26
- [GameDev.net - Dungeon Map Structure](https://www.gamedev.net/forums/topic/701322-first-person-dungeoncrawler-map-structure/) -- Thin vs thick wall discussion. Accessed: 2026-03-26
- [Rust Game Engines in 2026 Comparison](https://aarambhdevhub.medium.com/rust-game-engines-in-2026-bevy-vs-macroquad-vs-ggez-vs-fyrox-which-one-should-you-actually-use-9bf93669e83f) -- Published: Feb 2026, Accessed: 2026-03-26
- [dcrawl - Bevy DRPG Experiment](https://github.com/khongcodes/dcrawl) -- Reference Bevy dungeon crawler project. Accessed: 2026-03-26
- [bevy_turn-based_combat](https://github.com/Fabinistere/bevy_turn-based_combat) -- Reference turn-based combat implementation. Accessed: 2026-03-26
- [logic-turn-based-rpg](https://github.com/mwbryant/logic-turn-based-rpg) -- Bevy RPG with combat system example. Accessed: 2026-03-26
- [moonshine-save GitHub](https://github.com/Zeenobit/moonshine_save) -- Selective ECS save/load framework. Accessed: 2026-03-26
- [bevy_save GitHub](https://github.com/hankjordan/bevy_save) -- Full world save/load with migrations. Accessed: 2026-03-26

### Tertiary (LOW confidence)

- [Bevy Breaking Changes Community Discussion #21838](https://github.com/bevyengine/bevy/discussions/21838) -- Developer frustration with version churn. Accessed: 2026-03-26
- [Bevy UI Limitations Discussion](https://github.com/bevyengine/bevy/discussions/9538) -- Community discussion on bevy_ui shortcomings. Accessed: 2026-03-26
- ["Brutal Truth" of Bevy Development](https://medium.com/@theopinionatedev/i-made-a-game-in-bevy-rust-instead-of-unity-and-heres-the-brutal-truth-349fb78f88bf) -- One developer's experience (validate with multiple sources). Accessed: 2026-03-26
- [Etrian Odyssey Wiki - FOE](https://etrian.fandom.com/wiki/FOE) -- FOE mechanics reference. Accessed: 2026-03-26
- [Binary Space Partitioning Dungeon Generation Guide](https://copyprogramming.com/howto/simple-example-of-bsp-dungeon-generation) -- BSP algorithm for procedural dungeons. Accessed: 2026-03-26
- [dungeoncrawlers.org](https://www.dungeoncrawlers.org/) -- Genre reference and game database. Accessed: 2026-03-26

---

## Metadata

**Confidence breakdown:**

- Standard stack: HIGH -- Bevy 0.18.1 is verified current; all recommended crates have verified Bevy 0.18 compatibility via crates.io release dates.
- Architecture (rendering approach): MEDIUM-HIGH -- Option B (3D+billboards) is well-supported by Bevy's renderer and matches genre conventions. bevy_sprite3d exact 0.18 compat needs runtime verification.
- Architecture (project structure): MEDIUM -- Based on Bevy community best practices and dcrawl reference project. No single authoritative source for DRPG-specific Bevy architecture.
- Genre mechanics: HIGH -- Based on well-documented game series with decades of precedent.
- Pitfalls: MEDIUM-HIGH -- Bevy version churn and UI limitations are well-documented community concerns with multiple corroborating sources. RPG balance pitfalls are general game design knowledge.
- Security: HIGH -- No CVEs found; attack surface is minimal for single-player offline game.
- Performance: MEDIUM -- Based on Bevy benchmarks and community reports rather than direct measurement for this specific architecture.

**Research date:** 2026-03-26
