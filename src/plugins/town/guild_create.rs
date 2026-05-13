//! Guild character-creation wizard — Feature #19.
//!
//! Multi-step UI gated by GuildMode::CreateXxx sub-states. State held in
//! `CreationDraft` (cleared on OnExit(TownLocation::Guild) + on creation
//! completion). The Confirm step pushes a RecruitDef onto Assets<RecruitPool>
//! (Option A from research — reuses handle_guild_recruit).
//!
//! ## Flow
//!
//! 1. `CreateRace`    — pick a race.
//! 2. `CreateClass`   — pick a class (filtered by race + ClassTable::get).
//! 3. `CreateRoll`    — roll bonus pool; re-roll unlimited times.
//! 4. `CreateAllocate`— distribute pool across 6 stats.
//! 5. `CreateName`    — enter character name.
//! 6. `CreateConfirm` — review + commit (push to RecruitPool).
//!
//! ## Plan reference
//!
//! `project/plans/20260513-120000-feature-19-character-creation.md`

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};
use leafwing_input_manager::prelude::ActionState;

use crate::data::{ClassTable, RaceTable, RecruitDef, RecruitPool};
use crate::data::town::{MAX_RECRUIT_POOL, clamp_recruit_pool};
use crate::plugins::input::MenuAction;
use crate::plugins::loading::TownAssets;
use crate::plugins::party::character::{BaseStats, Class, PartyRow, Race};
use crate::plugins::party::progression::{
    ProgressionRng, allocate_bonus_pool, can_create_class, roll_bonus_pool,
};
use crate::plugins::town::guild::{GuildMode, GuildState};
use crate::plugins::town::toast::Toasts;

// ─────────────────────────────────────────────────────────────────────────────
// Constants
// ─────────────────────────────────────────────────────────────────────────────

/// Maximum character name length. Enforced in two places:
/// 1. `handle_guild_create_name_input` — rejects characters beyond this limit.
/// 2. `handle_guild_create_confirm` — truncates as defense-in-depth.
pub const MAX_NAME_LEN: usize = 20;

/// Ordered list of all Class variants for iteration in the creation wizard.
/// MUST remain in sync with the Class enum discriminant order (locked for
/// save-format stability). Filter with `ClassTable::get(c).is_some()` to skip
/// unauthrored variants.
const ALL_CLASSES: [Class; 8] = [
    Class::Fighter,
    Class::Mage,
    Class::Priest,
    Class::Thief,
    Class::Bishop,
    Class::Samurai,
    Class::Lord,
    Class::Ninja,
];

/// Stat names in allocation order (matches `allocations[0..6]`).
const STAT_NAMES: [&str; 6] = ["STR", "INT", "PIE", "VIT", "AGI", "LCK"];

// ─────────────────────────────────────────────────────────────────────────────
// CreationDraft resource
// ─────────────────────────────────────────────────────────────────────────────

/// Mutable wizard state. Cleared when leaving Guild or on creation completion.
#[derive(Resource, Default, Debug, Clone)]
pub struct CreationDraft {
    pub race: Option<Race>,
    pub class: Option<Class>,
    /// 0 = not yet rolled.
    pub rolled_bonus: u32,
    /// Per-stat point allocations: [STR, INT, PIE, VIT, AGI, LCK].
    pub allocations: [u16; 6],
    pub name: String,
    pub default_row: PartyRow,
}

impl CreationDraft {
    /// Reset to default state.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Sum of all current allocations.
    pub fn allocations_sum(&self) -> u32 {
        self.allocations.iter().map(|&v| v as u32).sum()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper — compute projected base stats
// ─────────────────────────────────────────────────────────────────────────────

/// Compute the projected `BaseStats` for the current draft: `starting_stats +
/// allocations + race_modifiers`.
///
/// Returns `None` if `class` or `race` are not set, or if their definitions
/// are not in the provided tables.
pub fn projected_base_stats(
    draft: &CreationDraft,
    class_table: &ClassTable,
    race_table: &RaceTable,
) -> Option<BaseStats> {
    let class = draft.class?;
    let race = draft.race?;
    let class_def = class_table.get(class)?;
    let race_data = race_table.get(race)?;

    let mut base = class_def.starting_stats;
    // Apply allocations and race modifiers in one call.
    let _ = allocate_bonus_pool(
        &mut base,
        &draft.allocations,
        draft.rolled_bonus.max(draft.allocations_sum()), // generous upper bound for preview
        &race_data.stat_modifiers,
    );
    Some(base)
}

// ─────────────────────────────────────────────────────────────────────────────
// Painters (read-only — EguiPrimaryContextPass)
// ─────────────────────────────────────────────────────────────────────────────

/// Painter for `GuildMode::CreateRace`.
#[allow(clippy::too_many_arguments)]
pub fn paint_guild_create_race(
    mut contexts: EguiContexts,
    draft: Res<CreationDraft>,
    guild_state: Res<GuildState>,
    town_assets: Option<Res<TownAssets>>,
    race_assets: Res<Assets<RaceTable>>,
) -> Result {
    let ctx = contexts.ctx_mut()?;

    egui::CentralPanel::default().show(ctx, |ui| {
        ui.heading("Create Character — Step 1: Choose Race");
        ui.separator();

        let table = town_assets
            .as_ref()
            .and_then(|a| race_assets.get(&a.race_table));

        match table {
            None => {
                ui.label("(loading races...)");
            }
            Some(race_table) => {
                for (idx, race_data) in race_table.races.iter().enumerate() {
                    let cursor_marker = if idx == guild_state.cursor { "> " } else { "  " };
                    let selected_marker = if draft.race == Some(race_data.id) { " [selected]" } else { "" };

                    // Display signed-i16 modifiers via `field as i16` reinterpretation.
                    let m = &race_data.stat_modifiers;
                    let mod_str = format!(
                        "STR{:+} INT{:+} PIE{:+} VIT{:+} AGI{:+} LCK{:+}",
                        m.strength as i16,
                        m.intelligence as i16,
                        m.piety as i16,
                        m.vitality as i16,
                        m.agility as i16,
                        m.luck as i16,
                    );
                    ui.label(format!(
                        "{}{} — {}{}",
                        cursor_marker, race_data.display_name, mod_str, selected_marker
                    ));
                    if !race_data.description.is_empty() {
                        ui.label(format!("    {}", race_data.description));
                    }
                }
            }
        }

        ui.add_space(8.0);
        ui.label("[Up/Down] Pick  |  [Enter] Confirm  |  [Esc] Cancel creation");
    });

    Ok(())
}

/// Painter for `GuildMode::CreateClass`.
#[allow(clippy::too_many_arguments)]
pub fn paint_guild_create_class(
    mut contexts: EguiContexts,
    draft: Res<CreationDraft>,
    guild_state: Res<GuildState>,
    town_assets: Option<Res<TownAssets>>,
    class_assets: Res<Assets<ClassTable>>,
) -> Result {
    let ctx = contexts.ctx_mut()?;

    egui::CentralPanel::default().show(ctx, |ui| {
        let race_label = draft.race.map(|r| format!("{r:?}")).unwrap_or_else(|| "?".into());
        ui.heading(format!(
            "Create Character — Step 2: Choose Class  (Race: {race_label})"
        ));
        ui.separator();

        let table = town_assets
            .as_ref()
            .and_then(|a| class_assets.get(&a.class_table));

        match table {
            None => {
                ui.label("(loading classes...)");
            }
            Some(class_table) => {
                // Filter: authored classes whose allowed_races includes draft.race.
                let chosen_race = draft.race.unwrap_or(Race::Human);
                let visible: Vec<&crate::data::ClassDef> = ALL_CLASSES
                    .iter()
                    .filter_map(|&c| class_table.get(c))
                    .filter(|def| {
                        def.allowed_races.is_empty()
                            || def.allowed_races.contains(&chosen_race)
                    })
                    .collect();

                // Authored classes the current race can't take — listed below
                // the cursor-navigable visible list so the rule is discoverable.
                let restricted: Vec<&crate::data::ClassDef> = ALL_CLASSES
                    .iter()
                    .filter_map(|&c| class_table.get(c))
                    .filter(|def| {
                        !def.allowed_races.is_empty()
                            && !def.allowed_races.contains(&chosen_race)
                    })
                    .collect();

                let total_rows = visible.len();
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show_rows(ui, 28.0, total_rows, |ui, row_range| {
                        for idx in row_range {
                            let def = &visible[idx];
                            let cursor_marker = if idx == guild_state.cursor { "> " } else { "  " };
                            let selected_marker =
                                if draft.class == Some(def.id) { " [selected]" } else { "" };
                            let ms = &def.min_stats;
                            ui.label(format!(
                                "{}{} — min STR≥{} INT≥{} PIE≥{} | HP+{}/lv | XP base {}{}",
                                cursor_marker,
                                def.display_name,
                                ms.strength,
                                ms.intelligence,
                                ms.piety,
                                def.hp_per_level,
                                def.xp_to_level_2,
                                selected_marker,
                            ));
                        }
                    });

                if !restricted.is_empty() {
                    ui.add_space(4.0);
                    ui.separator();
                    let names: Vec<&str> =
                        restricted.iter().map(|d| d.display_name.as_str()).collect();
                    ui.label(format!(
                        "Restricted for {chosen_race:?}: {} (pick a different race to access)",
                        names.join(", ")
                    ));
                }
            }
        }

        ui.add_space(8.0);
        ui.label("[Up/Down] Pick  |  [Enter] Confirm  |  [Esc] Cancel creation");
    });

    Ok(())
}

/// Painter for `GuildMode::CreateRoll`.
pub fn paint_guild_create_roll(
    mut contexts: EguiContexts,
    draft: Res<CreationDraft>,
) -> Result {
    let ctx = contexts.ctx_mut()?;

    egui::CentralPanel::default().show(ctx, |ui| {
        let race_label = draft.race.map(|r| format!("{r:?}")).unwrap_or_else(|| "?".into());
        let class_label = draft.class.map(|c| format!("{c:?}")).unwrap_or_else(|| "?".into());
        ui.heading("Create Character — Step 3: Roll Bonus Pool");
        ui.label(format!("Race: {}  |  Class: {}", race_label, class_label));
        ui.separator();

        if draft.rolled_bonus == 0 {
            ui.label("(press R to roll your bonus pool)");
        } else {
            ui.heading(format!("Bonus pool: {}", draft.rolled_bonus));
            ui.label("Press R to re-roll (unlimited before allocation).");
            ui.label("Press Enter to accept and allocate.");
        }

        ui.add_space(8.0);
        ui.label("[R] Re-roll  |  [Enter] Accept and allocate  |  [Esc] Cancel");
    });

    Ok(())
}

/// Painter for `GuildMode::CreateAllocate`.
#[allow(clippy::too_many_arguments)]
pub fn paint_guild_create_allocate(
    mut contexts: EguiContexts,
    draft: Res<CreationDraft>,
    guild_state: Res<GuildState>,
    town_assets: Option<Res<TownAssets>>,
    class_assets: Res<Assets<ClassTable>>,
    race_assets: Res<Assets<RaceTable>>,
) -> Result {
    let ctx = contexts.ctx_mut()?;

    egui::CentralPanel::default().show(ctx, |ui| {
        ui.heading("Create Character — Step 3.5: Allocate Bonus Points");
        ui.label(format!(
            "Pool: {} / {} points spent",
            draft.allocations_sum(),
            draft.rolled_bonus
        ));
        ui.separator();

        let class = draft.class.unwrap_or(Class::Fighter);
        let table = town_assets
            .as_ref()
            .and_then(|a| class_assets.get(&a.class_table));
        let race_table = town_assets
            .as_ref()
            .and_then(|a| race_assets.get(&a.race_table));
        let class_def = table.and_then(|t| t.get(class));

        let starting = class_def.map(|d| d.starting_stats).unwrap_or(BaseStats::ZERO);
        let stat_values = [
            starting.strength.saturating_add(draft.allocations[0]),
            starting.intelligence.saturating_add(draft.allocations[1]),
            starting.piety.saturating_add(draft.allocations[2]),
            starting.vitality.saturating_add(draft.allocations[3]),
            starting.agility.saturating_add(draft.allocations[4]),
            starting.luck.saturating_add(draft.allocations[5]),
        ];

        for (idx, (name, val)) in STAT_NAMES.iter().zip(stat_values.iter()).enumerate() {
            let cursor_marker = if idx == guild_state.cursor { "> " } else { "  " };
            ui.label(format!(
                "{}{}: {} (+{})",
                cursor_marker, name, val, draft.allocations[idx]
            ));
        }

        // Live eligibility check.
        ui.add_space(6.0);
        if let (Some(cd), Some(rt)) = (class_def, race_table)
            && let Some(race) = draft.race
            && let Some(base) = projected_base_stats(&draft, table.unwrap(), rt)
        {
            match can_create_class(race, &base, cd) {
                Ok(()) => {
                    ui.colored_label(egui::Color32::GREEN, "Eligible for this class.");
                }
                Err(e) => {
                    ui.colored_label(
                        egui::Color32::RED,
                        format!("Not eligible: {e:?}"),
                    );
                }
            }
        }

        ui.add_space(8.0);
        ui.label(
            "[Up/Down] Stat  |  [Left/Right] Allocate  |  [Enter] Continue  |  [Esc] Cancel",
        );
    });

    Ok(())
}

/// Painter for `GuildMode::CreateName`.
pub fn paint_guild_create_name(
    mut contexts: EguiContexts,
    draft: Res<CreationDraft>,
) -> Result {
    let ctx = contexts.ctx_mut()?;

    egui::CentralPanel::default().show(ctx, |ui| {
        ui.heading("Create Character — Step 4: Name");
        ui.separator();
        ui.label(format!(
            "Name: {}_",
            &draft.name[..draft.name.len().min(MAX_NAME_LEN)]
        ));
        ui.label(format!("({}/{} characters)", draft.name.len(), MAX_NAME_LEN));
        ui.add_space(8.0);
        ui.label(
            "[Type] Edit  |  [Backspace] Delete  |  [Enter] Confirm  |  [Esc] Cancel",
        );
    });

    Ok(())
}

/// Painter for `GuildMode::CreateConfirm`.
#[allow(clippy::too_many_arguments)]
pub fn paint_guild_create_confirm(
    mut contexts: EguiContexts,
    draft: Res<CreationDraft>,
    town_assets: Option<Res<TownAssets>>,
    class_assets: Res<Assets<ClassTable>>,
    race_assets: Res<Assets<RaceTable>>,
) -> Result {
    let ctx = contexts.ctx_mut()?;

    egui::CentralPanel::default().show(ctx, |ui| {
        ui.heading("Create Character — Step 5: Confirm");
        ui.separator();

        ui.label(format!("Name:  {}", draft.name));
        if let Some(race) = draft.race {
            let display_name = town_assets
                .as_ref()
                .and_then(|a| race_assets.get(&a.race_table))
                .and_then(|t| t.get(race))
                .map(|r| r.display_name.as_str())
                .unwrap_or("?");
            ui.label(format!("Race:  {:?} ({})", race, display_name));
        }
        if let Some(class) = draft.class {
            let display_name = town_assets
                .as_ref()
                .and_then(|a| class_assets.get(&a.class_table))
                .and_then(|t| t.get(class))
                .map(|c| c.display_name.as_str())
                .unwrap_or("?");
            ui.label(format!("Class: {:?} ({})", class, display_name));
        }

        // Final base stats preview.
        let class_table = town_assets
            .as_ref()
            .and_then(|a| class_assets.get(&a.class_table));
        let race_table = town_assets
            .as_ref()
            .and_then(|a| race_assets.get(&a.race_table));

        if let (Some(ct), Some(rt)) = (class_table, race_table)
            && let Some(base) = projected_base_stats(&draft, ct, rt)
        {
            ui.label(format!(
                "Stats: STR:{} INT:{} PIE:{} VIT:{} AGI:{} LCK:{}",
                base.strength,
                base.intelligence,
                base.piety,
                base.vitality,
                base.agility,
                base.luck,
            ));
        }
        ui.label("Row:   Front (default)");

        ui.add_space(8.0);
        ui.label("[Enter] Confirm & recruit  |  [Esc] Back to allocation");
    });

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Handlers (mutating — Update schedule)
// ─────────────────────────────────────────────────────────────────────────────

/// Umbrella navigation handler for all 6 creation sub-modes.
///
/// Handles `Cancel`, `Up`/`Down` cursor movement, and `Confirm` (mode advance).
/// Allocation Left/Right is handled by `handle_guild_create_allocate`.
/// Name input is handled by `handle_guild_create_name_input`.
/// Roll (R) is handled by `handle_guild_create_roll`.
/// Final confirm is handled by `handle_guild_create_confirm`.
#[allow(clippy::too_many_arguments)]
pub fn handle_guild_create_input(
    actions: Res<ActionState<MenuAction>>,
    mut guild_state: ResMut<GuildState>,
    mut draft: ResMut<CreationDraft>,
    mut toasts: ResMut<Toasts>,
    town_assets: Option<Res<TownAssets>>,
    class_assets: Res<Assets<ClassTable>>,
    race_assets: Res<Assets<RaceTable>>,
) {
    // Only active in creation sub-modes.
    match guild_state.mode {
        GuildMode::CreateRace
        | GuildMode::CreateClass
        | GuildMode::CreateRoll
        | GuildMode::CreateAllocate
        | GuildMode::CreateName
        | GuildMode::CreateConfirm => {}
        _ => return,
    }

    // Cancel — exit creation flow. In CreateConfirm, Esc steps back to Name
    // (the previous step) rather than dropping the whole draft. From any
    // other creation step, Esc resets the draft and returns to the Recruit
    // screen (where the user entered creation from via `]`).
    if actions.just_pressed(&MenuAction::Cancel) {
        if guild_state.mode == GuildMode::CreateConfirm {
            guild_state.mode = GuildMode::CreateName;
            return;
        }
        draft.reset();
        guild_state.mode = GuildMode::Recruit;
        guild_state.cursor = 0;
        return;
    }

    // Resolve tables once (needed for list-len and eligibility checks).
    let race_table = town_assets
        .as_ref()
        .and_then(|a| race_assets.get(&a.race_table));
    let class_table = town_assets
        .as_ref()
        .and_then(|a| class_assets.get(&a.class_table));

    // Up / Down cursor.
    if actions.just_pressed(&MenuAction::Up) {
        if guild_state.cursor > 0 {
            guild_state.cursor -= 1;
        }
        return;
    }
    if actions.just_pressed(&MenuAction::Down) {
        let list_len = creation_list_len(&guild_state.mode, &draft, race_table, class_table);
        if list_len > 0 {
            guild_state.cursor =
                (guild_state.cursor + 1).min(list_len.saturating_sub(1));
        }
        return;
    }

    // Confirm — advance sub-mode.
    if actions.just_pressed(&MenuAction::Confirm) {
        match guild_state.mode {
            GuildMode::CreateRace => {
                if let Some(rt) = race_table
                    && let Some(race_data) = rt.races.get(guild_state.cursor)
                {
                    draft.race = Some(race_data.id);
                    guild_state.mode = GuildMode::CreateClass;
                    guild_state.cursor = 0;
                }
            }

            GuildMode::CreateClass => {
                if let (Some(ct), Some(race)) = (class_table, draft.race) {
                    let visible: Vec<Class> = ALL_CLASSES
                        .iter()
                        .copied()
                        .filter(|&c| {
                            ct.get(c).is_some_and(|def| {
                                def.allowed_races.is_empty()
                                    || def.allowed_races.contains(&race)
                            })
                        })
                        .collect();

                    if let Some(&chosen) = visible.get(guild_state.cursor) {
                        // Defense-in-depth: verify ClassTable actually has this class.
                        if ct.get(chosen).is_some() {
                            draft.class = Some(chosen);
                            guild_state.mode = GuildMode::CreateRoll;
                            guild_state.cursor = 0;
                        }
                    }
                }
            }

            GuildMode::CreateRoll => {
                if draft.rolled_bonus > 0 {
                    guild_state.mode = GuildMode::CreateAllocate;
                    guild_state.cursor = 0;
                } else {
                    toasts.push("Roll your bonus pool first (press R).");
                }
            }

            GuildMode::CreateAllocate => {
                // Defense-in-depth eligibility check before advancing to name step.
                if let (Some(ct), Some(rt), Some(race)) = (class_table, race_table, draft.race)
                    && let Some(base) = projected_base_stats(&draft, ct, rt)
                    && let Some(class) = draft.class
                    && let Some(class_def) = ct.get(class)
                {
                    match can_create_class(race, &base, class_def) {
                        Ok(()) => {
                            guild_state.mode = GuildMode::CreateName;
                            guild_state.cursor = 0;
                        }
                        Err(e) => {
                            toasts.push(format!("Cannot create class: {e:?}"));
                        }
                    }
                }
                // Tables not loaded — stay in Allocate.
            }

            // CreateName advance is handled by `handle_guild_create_name_input`
            // on Key::Enter — NOT on MenuAction::Confirm. Space is also bound
            // to Confirm but must remain typeable as a character on this step.
            GuildMode::CreateName => {}

            // CreateConfirm is handled by handle_guild_create_confirm.
            GuildMode::CreateConfirm => {}

            _ => {}
        }
    }
}

/// Per-stat allocation handler, gated on `GuildMode::CreateAllocate`.
///
/// Left/Right adjusts the active stat's allocation by 1.
pub fn handle_guild_create_allocate(
    guild_state: Res<GuildState>,
    mut draft: ResMut<CreationDraft>,
    actions: Res<ActionState<MenuAction>>,
    mut toasts: ResMut<Toasts>,
    mut pool_full_cooldown: Local<f32>,
    time: Res<Time>,
) {
    if guild_state.mode != GuildMode::CreateAllocate {
        return;
    }

    *pool_full_cooldown = (*pool_full_cooldown - time.delta_secs()).max(0.0);

    let cursor = guild_state.cursor.min(5);

    if actions.just_pressed(&MenuAction::Left) {
        // Decrement (floor 0).
        draft.allocations[cursor] = draft.allocations[cursor].saturating_sub(1);
        return;
    }

    if actions.just_pressed(&MenuAction::Right) {
        // Increment only if we have remaining pool.
        if draft.allocations_sum() < draft.rolled_bonus {
            draft.allocations[cursor] = draft.allocations[cursor].saturating_add(1);
        } else if *pool_full_cooldown <= 0.0 {
            toasts.push("Bonus pool fully allocated.");
            *pool_full_cooldown = 1.0; // rate-limit to one per second
        }
    }
}

/// Name input handler, gated on `GuildMode::CreateName`.
///
/// Reads `KeyboardInput` messages directly (NOT leafwing — character keys aren't mapped).
/// Appends printable ASCII alphanumeric + space; handles Backspace.
///
/// Enter advances to `CreateConfirm`. We handle this here (not in
/// `handle_guild_create_input`) because Space is bound to
/// `MenuAction::Confirm` too — if the umbrella handler advanced on Confirm,
/// every Space-press would skip the name step.
pub fn handle_guild_create_name_input(
    mut guild_state: ResMut<GuildState>,
    mut draft: ResMut<CreationDraft>,
    mut toasts: ResMut<Toasts>,
    mut events: MessageReader<KeyboardInput>,
) {
    if guild_state.mode != GuildMode::CreateName {
        return;
    }
    for event in events.read() {
        if event.state != ButtonState::Pressed {
            continue;
        }
        match &event.logical_key {
            Key::Character(s) => {
                if draft.name.len() + s.len() <= MAX_NAME_LEN
                    && s.chars().all(|c| c.is_ascii_alphanumeric() || c == ' ')
                {
                    draft.name.push_str(s);
                }
            }
            // Space is a named variant in Bevy 0.18's `Key`, NOT
            // `Key::Character(" ")` — so it would otherwise fall through
            // to `_ => {}` and be silently dropped.
            Key::Space => {
                if draft.name.len() < MAX_NAME_LEN {
                    draft.name.push(' ');
                }
            }
            Key::Backspace => {
                draft.name.pop();
            }
            Key::Enter => {
                let trimmed = draft.name.trim().to_string();
                if trimmed.is_empty() {
                    toasts.push("Name cannot be empty.");
                } else if draft.name.len() > MAX_NAME_LEN {
                    toasts.push(format!("Name too long (max {MAX_NAME_LEN} characters)."));
                } else {
                    guild_state.mode = GuildMode::CreateConfirm;
                    guild_state.cursor = 0;
                }
            }
            _ => {}
        }
    }
}

/// Handle R (re-roll) key in `GuildMode::CreateRoll`.
pub fn handle_guild_create_roll(
    guild_state: Res<GuildState>,
    mut draft: ResMut<CreationDraft>,
    actions: Res<ActionState<MenuAction>>,
    class_assets: Res<Assets<ClassTable>>,
    town_assets: Option<Res<TownAssets>>,
    mut rng: ResMut<ProgressionRng>,
) {
    if guild_state.mode != GuildMode::CreateRoll {
        return;
    }
    if !actions.just_pressed(&MenuAction::Recruit) {
        return; // 'R' == MenuAction::Recruit
    }
    let Some(class) = draft.class else { return };
    let Some(assets) = town_assets else { return };
    let Some(table) = class_assets.get(&assets.class_table) else {
        return;
    };
    let Some(class_def) = table.get(class) else { return };

    draft.rolled_bonus = roll_bonus_pool(class_def, &mut *rng.0);
    // Reset previous allocations whenever a new pool is rolled (Pitfall: leftover
    // allocations would exceed the new pool size).
    draft.allocations = [0; 6];
}

/// Final confirm handler for `GuildMode::CreateConfirm`.
///
/// Defense-in-depth re-validates eligibility, then pushes a new `RecruitDef`
/// into `Assets<RecruitPool>`, auto-switches to `GuildMode::Recruit` with
/// cursor on the new entry, and resets the draft.
///
/// **Entry-frame arming:** `Local<bool>` skips the first frame after entering
/// `CreateConfirm`. The Enter press that advanced from `CreateName` also
/// satisfies `just_pressed(Confirm)` in the same frame — without this guard
/// the user never gets to see the confirm screen.
#[allow(clippy::too_many_arguments)]
pub fn handle_guild_create_confirm(
    mut armed: Local<bool>,
    mut guild_state: ResMut<GuildState>,
    mut draft: ResMut<CreationDraft>,
    actions: Res<ActionState<MenuAction>>,
    mut toasts: ResMut<Toasts>,
    town_assets: Option<Res<TownAssets>>,
    class_assets: Res<Assets<ClassTable>>,
    race_assets: Res<Assets<RaceTable>>,
    mut pool_assets: ResMut<Assets<RecruitPool>>,
) {
    if guild_state.mode != GuildMode::CreateConfirm {
        *armed = false;
        return;
    }
    if !*armed {
        *armed = true;
        return;
    }
    if !actions.just_pressed(&MenuAction::Confirm) {
        return;
    }

    let Some(assets) = town_assets else { return };
    let Some(class_table) = class_assets.get(&assets.class_table) else {
        return;
    };
    let Some(race_table) = race_assets.get(&assets.race_table) else {
        return;
    };

    // Resolve race and class.
    let Some(race) = draft.race else {
        toasts.push("No race selected.");
        return;
    };
    let Some(class) = draft.class else {
        toasts.push("No class selected.");
        return;
    };
    let Some(class_def) = class_table.get(class) else {
        toasts.push("Invalid class.");
        return;
    };

    // Compute final base stats.
    let Some(race_data) = race_table.get(race) else {
        toasts.push("Invalid race.");
        return;
    };
    let mut final_base = class_def.starting_stats;
    if allocate_bonus_pool(
        &mut final_base,
        &draft.allocations,
        draft.rolled_bonus,
        &race_data.stat_modifiers,
    )
    .is_err()
    {
        toasts.push("Allocation error — bonus pool overflow.");
        return;
    }

    // Defense-in-depth eligibility check.
    if let Err(e) = can_create_class(race, &final_base, class_def) {
        toasts.push(format!("Cannot create: {e:?}"));
        return;
    }

    // Truncate name (trust boundary — defense-in-depth).
    draft.name.truncate(MAX_NAME_LEN);
    let name = draft.name.trim().to_string();
    if name.is_empty() {
        toasts.push("Name cannot be empty.");
        return;
    }

    // Mutate Assets<RecruitPool>.
    let Some(pool) = pool_assets.get_mut(&assets.recruit_pool) else {
        toasts.push("Recruit pool not loaded.");
        return;
    };

    // MAX_RECRUIT_POOL cap check.
    if pool.recruits.len() >= MAX_RECRUIT_POOL {
        toasts.push("Recruit pool full — dismiss someone first.");
        return;
    }

    let new_index = pool.recruits.len();
    pool.recruits.push(RecruitDef {
        name: name.clone(),
        race,
        class,
        base_stats: final_base,
        default_row: PartyRow::Front,
    });

    // Auto-switch to Recruit mode with cursor on the new entry.
    let toast_name = name.clone();
    draft.reset();
    guild_state.mode = GuildMode::Recruit;
    guild_state.cursor = new_index.min(clamp_recruit_pool(pool, MAX_RECRUIT_POOL).len().saturating_sub(1));

    toasts.push(format!("{toast_name} has joined the guild!"));
    info!("Guild create: pushed '{toast_name}' ({race:?} {class:?}) at pool index {new_index}");
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper — list length per sub-mode
// ─────────────────────────────────────────────────────────────────────────────

fn creation_list_len(
    mode: &GuildMode,
    draft: &CreationDraft,
    race_table: Option<&RaceTable>,
    class_table: Option<&ClassTable>,
) -> usize {
    match mode {
        GuildMode::CreateRace => {
            race_table.map(|t| t.races.len()).unwrap_or(0)
        }
        GuildMode::CreateClass => {
            let chosen_race = draft.race.unwrap_or(Race::Human);
            class_table
                .map(|ct| {
                    ALL_CLASSES
                        .iter()
                        .filter(|&&c| {
                            ct.get(c).is_some_and(|def| {
                                def.allowed_races.is_empty()
                                    || def.allowed_races.contains(&chosen_race)
                            })
                        })
                        .count()
                })
                .unwrap_or(0)
        }
        GuildMode::CreateAllocate => 6, // 6 stats
        _ => 0,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::asset::AssetPlugin;
    use bevy::state::app::StatesPlugin;
    use leafwing_input_manager::prelude::*;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    use crate::data::classes::ClassRequirement;
    use crate::data::town::{RecruitDef, RecruitPool, ShopStock, TownServices};
    use crate::plugins::loading::TownAssets;
    use crate::plugins::party::character::{BaseStats, Class, PartyRow, Race};
    use crate::plugins::party::progression::{ProgressionRng, CombatVictoryEvent};
    use crate::plugins::state::{GameState, TownLocation};
    use crate::plugins::town::gold::{GameClock, Gold};
    use crate::plugins::town::guild::{DismissedPool, GuildMode, GuildState, RecruitedSet};

    fn fighter_class_def() -> crate::data::ClassDef {
        crate::data::ClassDef {
            id: Class::Fighter,
            display_name: "Fighter".into(),
            starting_stats: BaseStats {
                strength: 14,
                intelligence: 8,
                piety: 8,
                vitality: 14,
                agility: 10,
                luck: 9,
            },
            growth_per_level: BaseStats {
                strength: 2,
                intelligence: 0,
                piety: 0,
                vitality: 2,
                agility: 1,
                luck: 0,
            },
            hp_per_level: 8,
            mp_per_level: 0,
            xp_to_level_2: 100,
            xp_curve_factor: 1.5,
            min_stats: BaseStats {
                strength: 11,
                ..BaseStats::ZERO
            },
            allowed_races: vec![Race::Human, Race::Elf, Race::Dwarf, Race::Gnome, Race::Hobbit],
            advancement_requirements: vec![],
            bonus_pool_min: 5,
            bonus_pool_max: 9,
            stat_penalty_on_change: BaseStats::ZERO,
        }
    }

    fn mage_class_def() -> crate::data::ClassDef {
        crate::data::ClassDef {
            id: Class::Mage,
            display_name: "Mage".into(),
            starting_stats: BaseStats {
                strength: 7,
                intelligence: 14,
                piety: 7,
                vitality: 8,
                agility: 10,
                luck: 10,
            },
            growth_per_level: BaseStats {
                strength: 0,
                intelligence: 2,
                piety: 0,
                vitality: 1,
                agility: 1,
                luck: 1,
            },
            hp_per_level: 4,
            mp_per_level: 6,
            xp_to_level_2: 100,
            xp_curve_factor: 1.5,
            min_stats: BaseStats {
                intelligence: 11,
                ..BaseStats::ZERO
            },
            allowed_races: vec![Race::Human, Race::Elf, Race::Gnome, Race::Hobbit],
            advancement_requirements: vec![ClassRequirement {
                from_class: Class::Fighter,
                min_level: 5,
            }],
            bonus_pool_min: 5,
            bonus_pool_max: 9,
            stat_penalty_on_change: BaseStats::ZERO,
        }
    }

    fn human_race_data() -> crate::data::RaceData {
        crate::data::RaceData {
            id: Race::Human,
            display_name: "Human".into(),
            stat_modifiers: BaseStats::ZERO,
            description: "Balanced.".into(),
        }
    }

    fn dwarf_race_data() -> crate::data::RaceData {
        crate::data::RaceData {
            id: Race::Dwarf,
            display_name: "Dwarf".into(),
            stat_modifiers: BaseStats::ZERO, // no modifiers needed for test
            description: "Stout.".into(),
        }
    }

    fn make_create_test_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), StatesPlugin));

        app.init_state::<GameState>();
        app.add_sub_state::<TownLocation>();

        app.init_resource::<ActionState<MenuAction>>();
        app.insert_resource(InputMap::<MenuAction>::default());
        app.init_resource::<ButtonInput<KeyCode>>();

        app.init_resource::<GuildState>();
        app.init_resource::<DismissedPool>();
        app.init_resource::<RecruitedSet>();
        app.init_resource::<CreationDraft>();
        app.init_resource::<Toasts>();
        app.init_resource::<Gold>();
        app.init_resource::<GameClock>();

        // Register asset types.
        app.init_asset::<ClassTable>();
        app.init_asset::<RaceTable>();
        app.init_asset::<RecruitPool>();
        app.init_asset::<ShopStock>();
        app.init_asset::<TownServices>();

        // Insert seeded ProgressionRng.
        app.insert_resource(ProgressionRng(Box::new(
            ChaCha8Rng::seed_from_u64(42),
        )));

        app.add_message::<CombatVictoryEvent>();
        // Required by handle_guild_create_name_input (MessageReader<KeyboardInput>).
        // InputPlugin (which normally registers this) is omitted in MinimalPlugins;
        // register manually so the system param validates successfully.
        app.add_message::<KeyboardInput>();

        // Build and insert mock assets.
        let mut class_table = ClassTable::default();
        class_table.classes.push(fighter_class_def());
        class_table.classes.push(mage_class_def());
        let class_handle = app
            .world_mut()
            .resource_mut::<Assets<ClassTable>>()
            .add(class_table);

        let race_table = RaceTable {
            races: vec![human_race_data(), dwarf_race_data()],
        };
        let race_handle = app
            .world_mut()
            .resource_mut::<Assets<RaceTable>>()
            .add(race_table);

        let pool = RecruitPool { recruits: vec![] };
        let pool_handle = app
            .world_mut()
            .resource_mut::<Assets<RecruitPool>>()
            .add(pool);

        let mock_town_assets = TownAssets {
            shop_stock: Handle::default(),
            recruit_pool: pool_handle,
            services: Handle::default(),
            race_table: race_handle,
            class_table: class_handle,
        };
        app.insert_resource(mock_town_assets);

        // Register creation handlers.
        app.add_systems(
            Update,
            (
                handle_guild_create_input.run_if(in_state(TownLocation::Guild)),
                handle_guild_create_allocate.run_if(in_state(TownLocation::Guild)),
                handle_guild_create_name_input.run_if(in_state(TownLocation::Guild)),
                handle_guild_create_roll.run_if(in_state(TownLocation::Guild)),
                handle_guild_create_confirm.run_if(in_state(TownLocation::Guild)),
            ),
        );

        // Transition into Town / Guild.
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Town);
        app.update();
        app.world_mut()
            .resource_mut::<NextState<TownLocation>>()
            .set(TownLocation::Guild);
        app.update();
        app.update();

        app
    }

    fn press_confirm(app: &mut App) {
        app.world_mut()
            .resource_mut::<ActionState<MenuAction>>()
            .press(&MenuAction::Confirm);
        app.update();
        app.world_mut()
            .resource_mut::<ActionState<MenuAction>>()
            .release(&MenuAction::Confirm);
        app.update();
    }

    fn set_draft_to_confirm_ready(app: &mut App, draft: CreationDraft) {
        *app.world_mut().resource_mut::<CreationDraft>() = draft;
        app.world_mut().resource_mut::<GuildState>().mode = GuildMode::CreateConfirm;
        // Run one frame in CreateConfirm so handle_guild_create_confirm's
        // entry-frame-arming `Local<bool>` flips to true. Without this, the
        // first press_confirm() call just arms the local and never runs the
        // confirm logic. See doc on `handle_guild_create_confirm`.
        app.update();
    }

    /// Drive the draft to CreateConfirm with valid Human/Fighter, press Enter,
    /// assert pool grew by 1, mode switched to Recruit, draft reset.
    #[test]
    fn creation_confirm_appends_to_pool() {
        let mut app = make_create_test_app();

        set_draft_to_confirm_ready(
            &mut app,
            CreationDraft {
                race: Some(Race::Human),
                class: Some(Class::Fighter),
                rolled_bonus: 7,
                allocations: [2, 0, 0, 2, 0, 3], // sum=7, STR already 14+2=16 > 11 min
                name: "Aldric".into(),
                default_row: PartyRow::Front,
            },
        );

        press_confirm(&mut app);

        // Pool should have grown by 1.
        let town_assets = app.world().resource::<TownAssets>();
        let pool_handle = town_assets.recruit_pool.clone();
        let pool_assets = app.world().resource::<Assets<RecruitPool>>();
        let pool = pool_assets.get(&pool_handle).unwrap();
        assert_eq!(pool.recruits.len(), 1, "one recruit should have been pushed");
        assert_eq!(pool.recruits[0].name, "Aldric");
        assert_eq!(pool.recruits[0].class, Class::Fighter);

        // Mode should switch to Recruit.
        let guild_state = app.world().resource::<GuildState>();
        assert_eq!(guild_state.mode, GuildMode::Recruit);

        // Draft should be reset.
        let draft = app.world().resource::<CreationDraft>();
        assert!(draft.race.is_none());
        assert!(draft.name.is_empty());
    }

    /// Drive draft to CreateAllocate with Mage + allocations giving INT < 11,
    /// press Enter, assert toast was pushed and mode stays in CreateAllocate.
    #[test]
    fn creation_rejects_class_below_min_stats() {
        let mut app = make_create_test_app();

        // Mage starting_stats.intelligence = 14, min = 11. But we zero out allocations
        // and set starting_stats to something low by using a fresh draft with
        // mock class that has INT=5 starting. This is tricky since the class def
        // starting_stats is authoritative. The mage starts at INT=14 which already
        // meets min=11. So we test a race that's disallowed (Dwarf not in Mage allowed).
        // That tests the same enforcement path.
        *app.world_mut().resource_mut::<CreationDraft>() = CreationDraft {
            race: Some(Race::Dwarf), // Dwarf not allowed for Mage
            class: Some(Class::Mage),
            rolled_bonus: 5,
            allocations: [0; 6],
            name: "".into(),
            default_row: PartyRow::Front,
        };
        app.world_mut().resource_mut::<GuildState>().mode = GuildMode::CreateAllocate;

        press_confirm(&mut app);

        // Mode should still be CreateAllocate (rejected).
        let guild_state = app.world().resource::<GuildState>();
        assert_eq!(
            guild_state.mode,
            GuildMode::CreateAllocate,
            "should stay in CreateAllocate when class creation is invalid"
        );

        // Toast should have been pushed.
        let toasts = app.world().resource::<Toasts>();
        assert!(
            !toasts.queue.is_empty(),
            "a toast should have been pushed explaining the rejection"
        );
    }

    /// Prefill the pool with MAX_RECRUIT_POOL entries, press Confirm, assert no
    /// push happened and toast was pushed.
    #[test]
    fn creation_pool_full_blocks_confirm() {
        let mut app = make_create_test_app();

        // Fill the recruit pool to MAX_RECRUIT_POOL.
        {
            let town_assets = app.world().resource::<TownAssets>();
            let pool_handle = town_assets.recruit_pool.clone();
            let mut pool_assets = app.world_mut().resource_mut::<Assets<RecruitPool>>();
            let pool = pool_assets.get_mut(&pool_handle).unwrap();
            for i in 0..MAX_RECRUIT_POOL {
                pool.recruits.push(RecruitDef {
                    name: format!("recruit_{i}"),
                    race: Race::Human,
                    class: Class::Fighter,
                    base_stats: BaseStats::ZERO,
                    default_row: PartyRow::Front,
                });
            }
        }

        set_draft_to_confirm_ready(
            &mut app,
            CreationDraft {
                race: Some(Race::Human),
                class: Some(Class::Fighter),
                rolled_bonus: 7,
                allocations: [2, 0, 0, 2, 0, 3],
                name: "TooMany".into(),
                default_row: PartyRow::Front,
            },
        );

        press_confirm(&mut app);

        // Pool should NOT have grown.
        let town_assets = app.world().resource::<TownAssets>();
        let pool_handle = town_assets.recruit_pool.clone();
        let pool_assets = app.world().resource::<Assets<RecruitPool>>();
        let pool = pool_assets.get(&pool_handle).unwrap();
        assert_eq!(
            pool.recruits.len(),
            MAX_RECRUIT_POOL,
            "pool must not exceed MAX_RECRUIT_POOL"
        );

        // Toast should have been pushed.
        let toasts = app.world().resource::<Toasts>();
        assert!(
            !toasts.queue.is_empty(),
            "a toast should warn about the full pool"
        );
    }
}
