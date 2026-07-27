//! Activated, triggered, and static ability definitions.

use super::{AbilityCost, Color, Keyword, SpellEffectKind};
use serde::{Deserialize, Serialize};

/// One activated ability on a permanent (RON data tier). Cost + effect compose freely.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivatedAbilityDef {
    pub cost: AbilityCost,
    pub effect: SpellEffectKind,
    /// Oracle-style ability text shown as annotation on the stack card.
    pub text: String,
}

// ---------------------------------------------------------------------------
// Triggered abilities
// ---------------------------------------------------------------------------

/// Condition that causes a triggered ability to fire (CR 603).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TriggerCondition {
    /// When this permanent enters the battlefield.
    WhenSelfEntersBattlefield,
    /// When this permanent is put into a graveyard from the battlefield.
    WhenSelfDies,
    /// Whenever this creature attacks.
    WheneverSelfAttacks,
    /// Whenever this creature deals combat damage to a player (e.g. Scroll Thief).
    WheneverSelfDealsCombatDamageToPlayer,
    /// Whenever this creature deals damage to an opponent, combat or non-combat (e.g. Thieving Magpie).
    WheneverSelfDealsDamageToOpponent,
    /// At the beginning of this permanent's controller's upkeep.
    AtBeginningOfControllerUpkeep,
    /// Whenever a player casts a spell (optionally filtered by type). Parameters control
    /// whose casts qualify and which spell types count. Covers enchantress triggers
    /// (Argothian Enchantress), prowess-style draw/damage (Talrand, Young Pyromancer,
    /// Guttersnipe), and any-spell-cast watchers.
    WheneverPlayerCastsSpell {
        /// Whose casts trigger this ability relative to the source permanent's controller.
        /// Defaults to `Controller` ("whenever you cast").
        #[serde(default)]
        caster: CastTriggerPlayer,
        /// If `Some`, only spells of this type fire the trigger. `None` matches any spell.
        #[serde(default)]
        spell_type: Option<SpellTypeFilter>,
    },
    /// Whenever a permanent enters the battlefield (CR 603.6). The ETB-watcher analog of
    /// [`Self::WheneverPlayerCastsSpell`]: parameters control whose permanents and which type
    /// qualify. Covers Soul Warden (`controller: AnyPlayer`, `Creature`, `exclude_self`),
    /// landfall (`Controller`, `Land`), and constellation (`Controller`, `Enchantment`).
    WheneverPermanentEntersBattlefield {
        /// Whose permanents trigger this, relative to the source's controller. Defaults to
        /// `AnyPlayer` (the Soul Warden "whenever a creature enters" reading).
        #[serde(default = "any_player_trigger")]
        controller: CastTriggerPlayer,
        /// If `Some`, only permanents of this type fire the trigger. `None` matches any permanent.
        #[serde(default)]
        permanent_type: Option<PermanentTypeFilter>,
        /// If true, the source permanent's own entry does not trigger it (the "another" clause,
        /// e.g. Soul Warden). If false, the source can trigger off itself entering.
        #[serde(default)]
        exclude_self: bool,
    },
    /// Whenever a creature is put into a graveyard from the battlefield (CR 603.6). Observer
    /// variant of `WhenSelfDies` that watches *any* creature die. Covers Blood Artist, Falkenrath
    /// Noble ("whenever a creature dies, target player loses 1 life…"), Grim Haruspex
    /// ("whenever another nontoken creature you control dies, draw a card"), and similar
    /// "death matters" triggers. `controller` filters whose creatures dying count (relative to
    /// the source permanent's controller); `exclude_self` suppresses the source's own death.
    WheneverCreatureDies {
        /// Whose creatures dying trigger this, relative to the source permanent's controller.
        /// Defaults to `AnyPlayer` ("whenever a creature dies").
        #[serde(default = "any_player_trigger")]
        controller: CastTriggerPlayer,
        /// If true, the source permanent dying does not trigger it (the "another" clause,
        /// e.g. Grim Haruspex). If false, the source can trigger off its own death.
        #[serde(default)]
        exclude_self: bool,
    },
}

fn any_player_trigger() -> CastTriggerPlayer {
    CastTriggerPlayer::AnyPlayer
}

/// Permanent card-type filter for [`TriggerCondition::WheneverPermanentEntersBattlefield`].
/// Only types that can exist on the battlefield (CR 110.4) — instants/sorceries are excluded
/// by construction, unlike [`SpellTypeFilter`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermanentTypeFilter {
    Creature,
    Artifact,
    Enchantment,
    Land,
}

/// Which player's spell casts trigger a `WheneverPlayerCastsSpell` ability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CastTriggerPlayer {
    /// "Whenever you cast" — only the controller of this permanent.
    #[default]
    Controller,
    /// "Whenever an opponent casts" — any player who is not the controller.
    Opponent,
    /// "Whenever a player casts" — any player including the controller.
    AnyPlayer,
}

/// Spell type filter for `WheneverPlayerCastsSpell`. `None` on the field means any type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpellTypeFilter {
    Enchantment,
    Instant,
    Sorcery,
    /// Matches instants and sorceries (the most common pairing — Talrand, Young Pyromancer, etc.).
    InstantOrSorcery,
    Creature,
    Artifact,
    /// Matches any non-creature spell.
    Noncreature,
}

/// One triggered ability on a permanent (RON data tier). The effect is a plain
/// [`SpellEffectKind`] — the same effect type spells and activated abilities use. A
/// self-referencing effect (e.g. an upkeep self-pump) uses a `Self_` target filter rather
/// than a dedicated variant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TriggeredAbilityDef {
    pub trigger: TriggerCondition,
    pub effect: SpellEffectKind,
    /// Oracle-style ability text shown as annotation on the stack card.
    pub text: String,
}

// ---------------------------------------------------------------------------
// Static abilities (CR 604) and anthem/lord scopes
// ---------------------------------------------------------------------------

/// Controller restriction for an [`AnthemFilter`]. `None` on the field means "every creature in
/// play" (Crusade, Bad Moon — symmetrical anthems); `Some(YouControl)` means only the source's
/// controller's creatures (Glorious Anthem, Goblin King). An opponents-only variant is added with
/// its first card (e.g. an "opponents' creatures get -1/-1" enchantment).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnthemController {
    /// Only creatures controlled by the anthem source's controller ("creatures you control").
    YouControl,
}

/// Which creatures a static anthem or one-shot mass pump applies to (CR 613). AND-combined
/// optional constraints over the creatures in play, mirroring how [`TargetFilter`] narrows a
/// chosen target. "Name two" per field: `controller` (Glorious Anthem, Goblin King) · `subtype`
/// (Lord of Atlantis = Merfolk, Goblin Chieftain = Goblin) · `color` (Crusade = White, Bad Moon =
/// Black) · `exclude_self` (every "Other ... creatures" lord). Reused by both
/// [`StaticAbilityDef::AnthemPt`] and [`SpellEffectKind::PumpAll`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AnthemFilter {
    /// `None` = every creature in play; `Some(YouControl)` = only the source controller's creatures.
    #[serde(default)]
    pub controller: Option<AnthemController>,
    /// If `Some`, only creatures whose type line contains this subtype (e.g. "Merfolk", "Goblin").
    #[serde(default)]
    pub subtype: Option<String>,
    /// If `Some`, only creatures of this color (Crusade = White, Bad Moon = Black).
    #[serde(default)]
    pub color: Option<Color>,
    /// CR "other ... creatures": exclude the anthem's own source permanent (a lord that doesn't
    /// pump itself). Ignored by [`SpellEffectKind::PumpAll`], which has no persistent source.
    #[serde(default)]
    pub exclude_self: bool,
}

/// One static ability on a permanent (CR 604) — a continuous effect that exists only while the
/// permanent is on the battlefield. Distinct from triggered/activated abilities (which use the
/// stack); the engine emits the corresponding continuous effect on ETB and drains it at LTB.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StaticAbilityDef {
    /// CR 613.4 layer 7c: every creature matching `filter` gets +`delta_power`/+`delta_toughness`
    /// (negative values for a debuff anthem). Anthems (Glorious Anthem) and lords (Crusade, Bad Moon).
    AnthemPt {
        #[serde(default)]
        filter: AnthemFilter,
        delta_power: i32,
        delta_toughness: i32,
    },
    /// CR 613.4 layer 7c + CR 303.4: the enchanted creature (stored as `attached_to` on the aura's
    /// `GameObject`) gets +`delta_power`/+`delta_toughness` as long as the aura remains attached.
    /// The effect drains via `WhileSourceOnBattlefield` (source = the aura permanent); it is
    /// scoped to a single permanent (`AffectedScope::Single`) so it disappears the moment the aura
    /// leaves. Holy Strength (+1/+2), Unholy Strength (+2/+1).
    AuraPtModify {
        delta_power: i32,
        delta_toughness: i32,
    },
    /// CR 301.5b / 702.6: while this equipment is attached to a creature (i.e.
    /// `attached_to` is `Some`), that creature gets +`delta_power`/+`delta_toughness`
    /// (layer 7c). The scope is `AffectedScope::EquippedBy(equipment_oid)` — it reads
    /// `attached_to` dynamically at P/T query time, so re-equipping shifts the bonus
    /// without recreating the continuous effect. Covers Bonesplitter (+2/+0) and
    /// Vulshok Morningstar (+2/+2); any equipment with a stat boost uses this variant.
    EquippedBonus {
        delta_power: i32,
        delta_toughness: i32,
    },
    /// CR 613 layer 6: every creature matching `filter` gains `keyword` while the source is on the
    /// battlefield. Covers lords (Goblin Chieftain, Captain of the Watch) and keyword-granting
    /// enchantments. Pairs with `AnthemPt` on the same card for combined "+1/+1 and haste" effects.
    AnthemKeyword {
        #[serde(default)]
        filter: AnthemFilter,
        keyword: Keyword,
    },
    /// CR 305.2b / layer 5: controller may play `count` additional lands per turn while this
    /// permanent is on the battlefield. Exploration, Oracle of Mul Daya.
    ExtraLandPlays { count: u32 },
}
