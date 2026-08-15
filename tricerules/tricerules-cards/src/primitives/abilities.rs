//! Activated, triggered, and static ability definitions.

use super::{
    AbilityCost, Amount, BattlefieldCreatureCountFilter, CardTypeFilter, Color, CounterKind,
    EffectContext, GameCondition, Keyword, SpellEffectKind, TargetController, TargetFilter,
    TargetKind, TargetingDef,
};
use crate::ManaAmount;
use serde::{Deserialize, Serialize};

/// One activated ability on a permanent (RON data tier). Cost + effect compose freely.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivatedAbilityDef {
    /// Costs in authored order. Every component is validated before any component is paid.
    pub costs: Vec<AbilityCost>,
    /// CR 608.2: the ability's effects, resolved in the order written — the same shape and the
    /// same semantics as a spell's `spell_effect`. A single-effect ability is a one-element list.
    pub effect: Vec<SpellEffectKind>,
    #[serde(default)]
    pub targeting: Option<TargetingDef>,
    /// Additional timing instruction printed after the ability's effect (CR 602.1b). Normal
    /// activated abilities already require priority; this dimension records stricter timing.
    #[serde(default)]
    pub timing: ActivationTiming,
    /// Public conditions that must all hold when activation begins. They are activation
    /// instructions, not intervening-if clauses, and are not checked again on resolution.
    #[serde(default)]
    pub conditions: Vec<ActivationCondition>,
    /// Oracle-style ability text shown as annotation on the stack card.
    pub text: String,
}

/// Extra timing imposed by an activated ability's instructions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ActivationTiming {
    /// The ordinary CR 117.1b timing: activate while the player has priority.
    #[default]
    Normal,
    /// CR 307.5 / 602.5d: controller's main phase, with priority and an empty stack.
    SorcerySpeed,
}

/// A public predicate checked exactly when an activated ability begins activation (CR 602.1b).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActivationCondition {
    /// Reuse an engine-owned public turn-history fact such as "a creature died this turn."
    GameCondition(GameCondition),
    /// Require the bounded number of battlefield creatures matching the shared derived filter.
    /// Celestial Enforcer and Goblin Bird-Grabber use `min: Some(1)` plus Flying.
    BattlefieldCreatureCount {
        filter: BattlefieldCreatureCountFilter,
        #[serde(default)]
        min: Option<u32>,
        #[serde(default)]
        max: Option<u32>,
    },
}

impl ActivationCondition {
    pub(crate) fn validate(&self) -> Result<(), String> {
        match self {
            ActivationCondition::GameCondition(condition) => condition.validate(),
            ActivationCondition::BattlefieldCreatureCount { filter, min, max } => {
                filter.validate()?;
                if min.is_none() && max.is_none() {
                    return Err(
                        "BattlefieldCreatureCount activation condition requires at least one of min or max"
                            .into(),
                    );
                }
                if min
                    .as_ref()
                    .zip(max.as_ref())
                    .is_some_and(|(minimum, maximum)| minimum > maximum)
                {
                    return Err(
                        "BattlefieldCreatureCount activation condition min cannot exceed max"
                            .into(),
                    );
                }
                Ok(())
            }
        }
    }
}

impl ActivatedAbilityDef {
    /// CR 605.1a: a mana ability produces mana, doesn't target, and isn't a loyalty ability.
    /// Modelled as "the ability's *sole* effect is `ProduceMana`" — an ability that produced
    /// mana alongside another effect would not use the fast no-stack path, and deliberately
    /// answers `false` here rather than being silently mis-resolved.
    pub fn mana_options(&self) -> Option<&Vec<ManaAmount>> {
        match self.effect.as_slice() {
            [SpellEffectKind::ProduceMana { options, .. }] => Some(options),
            _ => None,
        }
    }

    pub fn mana_restriction(&self) -> Option<&super::ManaSpendingRestriction> {
        match self.effect.as_slice() {
            [SpellEffectKind::ProduceMana { restriction, .. }] => restriction.as_ref(),
            _ => None,
        }
    }

    pub fn conditional_mana_output(&self) -> Option<&super::ConditionalManaOutput> {
        match self.effect.as_slice() {
            [SpellEffectKind::ProduceMana { conditional, .. }] => conditional.as_ref(),
            _ => None,
        }
    }

    /// CR 702.6a: equip has "Activate only as a sorcery" built in.
    pub fn is_equip(&self) -> bool {
        self.effect
            .iter()
            .any(|e| matches!(e, SpellEffectKind::Equip { .. }))
    }

    /// Equip carries the same timing instruction intrinsically (CR 702.6a), so callers use this
    /// one query instead of maintaining separate explicit/keyword timing paths.
    pub fn requires_sorcery_speed(&self) -> bool {
        self.timing == ActivationTiming::SorcerySpeed || self.is_equip()
    }

    pub(crate) fn validate_shape(&self) -> Result<(), String> {
        if self.text.trim().is_empty() {
            return Err("activated ability text must not be empty".into());
        }
        if self.effect.is_empty() {
            return Err("activated ability must contain at least one effect".into());
        }
        if self
            .effect
            .iter()
            .any(SpellEffectKind::uses_trigger_object_reference)
        {
            return Err("activated abilities cannot reference a trigger object".into());
        }
        if self
            .effect
            .iter()
            .any(SpellEffectKind::uses_defending_player_reference)
        {
            return Err("activated abilities cannot reference a trigger's defending player".into());
        }
        for condition in &self.conditions {
            condition.validate()?;
        }
        if self
            .costs
            .iter()
            .filter(|cost| matches!(cost, AbilityCost::Mana(_)))
            .count()
            > 1
        {
            return Err("activated ability may have at most one mana cost component".into());
        }
        for cost in &self.costs {
            if let AbilityCost::SacrificePermanent { filter } = cost {
                filter.validate_characteristic_constraints()?;
                if !matches!(filter.kind, TargetKind::Creature | TargetKind::AnyPermanent)
                    || filter.controller != TargetController::You
                    || filter.exclude_source
                {
                    return Err("sacrifice cost filter requires Creature or AnyPermanent, controller: You, and may include its source".into());
                }
            }
        }
        for effect in &self.effect {
            effect.validate(EffectContext::Ability)?;
        }
        SpellEffectKind::validate_list(&self.effect)?;
        TargetingDef::validate_optional(self.targeting.as_ref(), &self.effect)
    }
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
    /// Delayed-trigger-only condition: the next end step that begins after creation.
    AtBeginningOfNextEndStep,
    /// Delayed-trigger-only condition: the controller that created the delayed trigger stops
    /// controlling the observed permanent.
    WhenControllerLosesControlOf,
    /// CR 508.1m / 508.3a: whenever this creature attacks, optionally requiring a minimum number
    /// of *other* creatures in the same declaration group. Zero is an ordinary self-attack
    /// trigger; two is Battalion (Makeshift Battalion, Firefist Striker, Haazda Marshal).
    WheneverSelfAttacks { minimum_other_attackers: u32 },
    /// CR 508.1m / 508.3a: whenever the object this Aura or Equipment is attached to attacks.
    /// Heart-Piercer Bow and Battle Mastery establish Equipment/Aura reuse of the event-time
    /// attachment relation; the observed attacker is available as `TriggerObject`.
    WheneverAttachedObjectAttacks,
    /// CR 603.6e / 603.10a: when the creature this Aura is attached to dies. The attachment and
    /// creature identity are captured immediately before the zone change. Unholy Indenture and
    /// Fool's Demise use this shape.
    WheneverAttachedObjectDies,
    /// CR 508.3b: whenever the player this Aura is attached to is attacked. This fires once for
    /// the declaration event regardless of the number of creatures attacking that player.
    /// Curse of Opulence and Curse of Disturbance use this shape.
    WheneverAttachedPlayerIsAttacked,
    /// CR 509.3b: whenever this creature blocks a creature matching `attacker`. One occurrence
    /// is created for each attacker-blocker edge, which also supports creatures that may block
    /// more than one creature (Palace Guard, Guardian of the Gateless).
    WheneverSelfBlocksCreature {
        #[serde(default)]
        attacker: CreatureEventFilter,
    },
    /// CR 509.3d: whenever this creature becomes blocked by a creature matching `blocker`. One
    /// occurrence is created for each blocking creature (Gloom Sower, Engulfing Slagwurm).
    WheneverSelfBecomesBlockedByCreature {
        #[serde(default)]
        blocker: CreatureEventFilter,
    },
    /// Whenever this creature deals combat damage to a player (e.g. Scroll Thief).
    WheneverSelfDealsCombatDamageToPlayer,
    /// Whenever this creature deals damage to an opponent, combat or non-combat (e.g. Thieving Magpie).
    WheneverSelfDealsDamageToOpponent,
    /// CR 503.1a: at the beginning of an upkeep step. `player` filters whose upkeep qualifies,
    /// relative to the source's controller — `Controller` = "at the beginning of your upkeep"
    /// (Phyrexian Arena, Serendib Efreet, Bitterblossom), `AnyPlayer` = "at the beginning of
    /// each player's upkeep" (Sulfuric Vortex, Tangle Wire), `Opponent` for "each opponent's
    /// upkeep" (Ebony Owl Netsuke).
    ///
    /// The player whose upkeep it is becomes the trigger's *affected* player (via
    /// `StackItem::trigger_context.affected_player`), which is what makes "**that player**" work when the
    /// source's controller is somebody else. Note the corollary, shared with
    /// [`Self::AtBeginningOfDrawStep`]: for an `Opponent`-scoped trigger whose effect is meant
    /// to benefit the *controller*, `affected_player` is the wrong player — such a card needs an
    /// explicit recipient on the effect. No shipped card hits it.
    AtBeginningOfUpkeep {
        /// Whose upkeep fires this, relative to the source permanent's controller. Defaults to
        /// `Controller` — the printed template on the overwhelming majority of upkeep triggers,
        /// and the meaning of the unit variant this replaced.
        #[serde(default)]
        player: CastTriggerPlayer,
    },
    /// CR 504.2: at the beginning of a draw step. `player` filters whose draw step qualifies,
    /// relative to the source's controller — `AnyPlayer` = "at the beginning of each player's
    /// draw step" (Howling Mine, Kami of the Crescent Moon, Rites of Flourishing), `Controller`
    /// = "at the beginning of your draw step" (Sylvan Library, Phyrexian Arena's draw-step
    /// analogues), `Opponent` for the opponents-only reading.
    ///
    /// The player whose draw step it is becomes the trigger's *affected* player
    /// ([`crate::TriggeredAbilityDef`] effects resolve against it via
    /// `StackItem::trigger_context.affected_player`), which is what makes "**that player** draws" work when the
    /// source's controller is somebody else.
    AtBeginningOfDrawStep {
        /// Whose draw step fires this, relative to the source permanent's controller.
        /// Defaults to `AnyPlayer` (the Howling Mine "each player's draw step" reading).
        #[serde(default = "any_player_trigger")]
        player: CastTriggerPlayer,
    },
    /// CR 513.1-2: at the beginning of an end step. `player` filters whose end step qualifies,
    /// relative to the source's controller. Defaults to `Controller`, the common "your end step"
    /// template used by Sabertooth Mauler and Twinblade Assassins.
    AtBeginningOfEndStep {
        #[serde(default)]
        player: CastTriggerPlayer,
    },
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
        spell_type: Option<CardTypeFilter>,
    },
    /// Whenever this permanent becomes the target of the selected kind of stack object. The
    /// targeting object has already been legally cast, activated, copied, or put on the stack;
    /// changing or copying targets can reuse the same event vocabulary when those actions exist.
    /// Covers Bonecrusher Giant (spells only) and Altanak, the Thrice-Called (opponent-controlled
    /// spells or abilities).
    WheneverSelfBecomesTarget {
        /// Whether casts, all spells (including copies), abilities, or either kind qualify.
        source: TargetingSourceFilter,
        /// Who controls the targeting spell or ability, relative to this permanent's controller.
        source_controller: CastTriggerPlayer,
    },
    /// Whenever a qualifying permanent becomes the target of a selected kind of stack object.
    /// One trigger is created for each distinct matching permanent, even when the same spell or
    /// ability names that permanent more than once. Covers Monk Gyatso's "another creature you
    /// control" watcher and similar heroic-support permanents.
    WheneverPermanentBecomesTarget {
        /// Whether casts, all spells (including copies), abilities, or either kind qualify.
        source: TargetingSourceFilter,
        /// Who controls the targeting spell or ability, relative to this permanent's controller.
        source_controller: CastTriggerPlayer,
        /// Who controls the targeted permanent, relative to this permanent's controller.
        target_controller: CastTriggerPlayer,
        /// If present, only targeted permanents of this type qualify.
        permanent_type: Option<PermanentTypeFilter>,
        /// The "another" clause: suppress this source permanent as the watched object.
        #[serde(default)]
        exclude_self: bool,
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
    /// Whenever a player gains life (CR 118.3). The lifegain-payoff analog of
    /// [`Self::WheneverPlayerCastsSpell`]: `player` filters whose life gain counts, relative to
    /// the source permanent's controller. Covers "whenever you gain life" (Ajani's Pridemate,
    /// Bloodthirsty Aerialist, Marauding Blight-Priest) and opponent-watching payoffs
    /// (`Opponent`). Fires once per life-gain event regardless of the amount, so a single
    /// 3-life gain triggers it once.
    WheneverPlayerGainsLife {
        /// Whose life gain triggers this, relative to the source permanent's controller.
        /// Defaults to `Controller` ("whenever you gain life").
        #[serde(default)]
        player: CastTriggerPlayer,
    },
}

impl TriggerCondition {
    pub(crate) fn is_delayed_only(&self) -> bool {
        matches!(
            self,
            Self::AtBeginningOfNextEndStep | Self::WhenControllerLosesControlOf
        )
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        match self {
            Self::WheneverSelfBlocksCreature { attacker } => attacker.validate(),
            Self::WheneverSelfBecomesBlockedByCreature { blocker } => blocker.validate(),
            _ => Ok(()),
        }
    }

    /// Whether a matching event supplies the distinct object referenced by
    /// `EffectSubject::TriggerObject` and `PlayerRecipient::TriggerObjectController`.
    pub(crate) fn supplies_trigger_object(&self) -> bool {
        matches!(
            self,
            Self::WheneverSelfBlocksCreature { .. }
                | Self::WheneverSelfBecomesBlockedByCreature { .. }
                | Self::WheneverAttachedObjectAttacks
                | Self::WheneverAttachedObjectDies
                | Self::WheneverSelfBecomesTarget { .. }
                | Self::WheneverPermanentBecomesTarget { .. }
                | Self::AtBeginningOfNextEndStep
                | Self::WhenControllerLosesControlOf
        )
    }

    /// Whether a matching attack event supplies an event-time defending player.
    pub(crate) fn supplies_defending_player(&self) -> bool {
        matches!(
            self,
            Self::WheneverSelfAttacks { .. }
                | Self::WheneverAttachedObjectAttacks
                | Self::WheneverAttachedPlayerIsAttacked
        )
    }
}

/// Derived creature characteristics captured by a discrete trigger event. Required and excluded
/// keywords compose, supporting Snarespinner (requires flying) and flanking (excludes flanking)
/// without putting either card-specific rule into the engine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CreatureEventFilter {
    #[serde(default)]
    pub required_keywords: Vec<Keyword>,
    #[serde(default)]
    pub excluded_keywords: Vec<Keyword>,
}

impl CreatureEventFilter {
    pub(crate) fn validate(&self) -> Result<(), String> {
        if self
            .required_keywords
            .iter()
            .any(|keyword| self.excluded_keywords.contains(keyword))
        {
            return Err("creature event filter cannot require and exclude the same keyword".into());
        }
        Ok(())
    }
}

fn any_player_trigger() -> CastTriggerPlayer {
    CastTriggerPlayer::AnyPlayer
}

/// Permanent card-type filter for [`TriggerCondition::WheneverPermanentEntersBattlefield`].
/// Only types that can exist on the battlefield (CR 110.4) — instants/sorceries are excluded
/// by construction, unlike [`CardTypeFilter`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PermanentTypeFilter {
    Creature,
    Artifact,
    Enchantment,
    Land,
}

impl PermanentTypeFilter {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Creature => "Creature",
            Self::Artifact => "Artifact",
            Self::Enchantment => "Enchantment",
            Self::Land => "Land",
        }
    }
}

/// An additive CR 205.1b type-line change. Card types and creature subtypes stay distinct in the
/// authored vocabulary even though the current characteristics snapshot stores their canonical
/// names in one ordered list. Dub exercises `creature_types`; Liquimetal Coating exercises
/// `card_types`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TypeLineAddition {
    #[serde(default)]
    pub card_types: Vec<PermanentTypeFilter>,
    #[serde(default)]
    pub creature_types: Vec<String>,
}

impl TypeLineAddition {
    pub fn is_empty(&self) -> bool {
        self.card_types.is_empty() && self.creature_types.is_empty()
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.is_empty() {
            return Err("type-line addition must add a card type or creature type".into());
        }
        let unique_card_types: std::collections::HashSet<_> =
            self.card_types.iter().copied().collect();
        if unique_card_types.len() != self.card_types.len() {
            return Err("type-line addition repeats a card type".into());
        }
        if self
            .creature_types
            .iter()
            .any(|creature_type| creature_type.trim().is_empty())
        {
            return Err("type-line addition creature type cannot be empty".into());
        }
        let unique_creature_types: std::collections::HashSet<_> =
            self.creature_types.iter().collect();
        if unique_creature_types.len() != self.creature_types.len() {
            return Err("type-line addition repeats a creature type".into());
        }
        Ok(())
    }
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

/// Which kind of object choosing targets can satisfy a becomes-the-target trigger.
///
/// `SpellCast` is deliberately narrower than `Spell`: heroic-style "whenever you cast a spell
/// that targets" abilities do not fire for spell copies, while Bonecrusher Giant's "target of a
/// spell" ability does (CR 707.10). `Ability` includes activated and triggered abilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TargetingSourceFilter {
    SpellCast,
    Spell,
    Ability,
    SpellOrAbility,
}

/// CR 603.4 intervening-"if" clause — the `if …` between the trigger event and the effect
/// ("at the beginning of each player's draw step, **if this artifact is untapped**, …"). Unlike
/// an ordinary condition it is checked *twice*: when the ability would go on the stack, and
/// again as it resolves; failing either check means the ability does nothing.
///
/// This is the general CR 603.4 slot on [`TriggeredAbilityDef`] — a new condition is a variant
/// here, never a per-card bool on the def.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InterveningIf {
    /// "if {this} is untapped" — Howling Mine.
    SourceUntapped,
    /// Total spells successfully cast during the immediately preceding turn, inclusive bounds.
    /// `None` is an open bound. Covers both faces of the original Innistrad werewolves.
    SpellsCastLastTurn {
        #[serde(default)]
        min: Option<u32>,
        #[serde(default)]
        max: Option<u32>,
    },
    /// A reusable public game-state predicate. The nested condition is validated at registry load
    /// and evaluated through the engine's canonical condition funnel for both CR 603.4 checks.
    GameCondition(GameCondition),
}

/// One triggered ability on a permanent (RON data tier). The effects are plain
/// [`SpellEffectKind`]s — the same effect type spells and activated abilities use. A
/// self-referencing effect (e.g. an upkeep self-pump) uses [`EffectSubject::Source`] rather than
/// a dedicated effect variant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TriggeredAbilityDef {
    pub trigger: TriggerCondition,
    /// CR 608.2: the ability's effects, resolved in the order written — the same shape and the
    /// same semantics as a spell's `spell_effect`. Phyrexian Arena's "you draw a card and you
    /// lose 1 life" is `[Draw(count: 1), LoseLife(amount: Fixed(1))]`.
    pub effect: Vec<SpellEffectKind>,
    #[serde(default)]
    pub targeting: Option<TargetingDef>,
    /// Oracle-style ability text shown as annotation on the stack card.
    pub text: String,
    /// CR 603.5: true when the triggered ability says "you may" and its controller may decline
    /// it while choosing targets.
    #[serde(default)]
    pub may: bool,
    /// CR 603.4: optional intervening-"if" clause, checked both when the trigger would be put
    /// on the stack and again on resolution. `None` for the overwhelming majority of triggers.
    #[serde(default)]
    pub intervening_if: Option<InterveningIf>,
}

impl TriggeredAbilityDef {
    pub(crate) fn validate_shape(&self) -> Result<(), String> {
        self.trigger.validate()?;
        if self.text.trim().is_empty() {
            return Err("triggered ability text must not be empty".into());
        }
        if self.effect.is_empty() {
            return Err("triggered ability must contain at least one effect".into());
        }
        if self
            .effect
            .iter()
            .any(SpellEffectKind::uses_trigger_object_reference)
            && !self.trigger.supplies_trigger_object()
        {
            return Err(
                "trigger-object effect requires a trigger that supplies an observed object".into(),
            );
        }
        if self
            .effect
            .iter()
            .any(SpellEffectKind::uses_defending_player_reference)
            && !self.trigger.supplies_defending_player()
        {
            return Err(
                "defending-player target requires an attack trigger that supplies a defender"
                    .into(),
            );
        }
        if let Some(InterveningIf::GameCondition(condition)) = self.intervening_if.as_ref() {
            condition.validate()?;
        }
        for effect in &self.effect {
            effect.validate(EffectContext::Ability)?;
        }
        SpellEffectKind::validate_list(&self.effect)?;
        TargetingDef::validate_optional(self.targeting.as_ref(), &self.effect)
    }
}

// ---------------------------------------------------------------------------
// Static abilities (CR 604) and shared creature scopes
// ---------------------------------------------------------------------------

/// Controller restriction for a [`CreatureScopeFilter`]. `None` on the field means "every creature in
/// play" (Crusade, Bad Moon — symmetrical anthems); `Some(YouControl)` means only the source's
/// controller's creatures (Glorious Anthem, Goblin King); `Some(Opponents)` means creatures
/// controlled by an opponent of the source's controller (Uncomfortable Chill, Make Obsolete).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CreatureScopeController {
    /// Only creatures controlled by the effect's reference player ("creatures you control").
    YouControl,
    /// Only creatures controlled by opponents of the source's controller ("creatures your
    /// opponents control"). Untargeted and player-set-generic.
    Opponents,
}

/// Which creatures a static ability or resolving one-shot effect applies to (CR 611/613).
/// AND-combined
/// optional constraints over the creatures in play, mirroring how [`TargetFilter`] narrows a
/// chosen target. "Name two" per field: `controller` (Glorious Anthem, Goblin King) · `subtype`
/// (Lord of Atlantis = Merfolk, Goblin Chieftain = Goblin) · `color` (Crusade = White, Bad Moon =
/// Black) · `exclude_self` (every "Other ... creatures" lord) · `attacking` (Trumpet Blast,
/// Warded Battlements). Reused by both [`StaticAbilityDef::AnthemPt`] and
/// [`SpellEffectKind::PumpAll`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CreatureScopeFilter {
    /// `None` = every creature in play; otherwise use the listed relationship to the source's
    /// controller.
    #[serde(default)]
    pub controller: Option<CreatureScopeController>,
    /// If `Some`, only creatures whose type line contains this subtype (e.g. "Merfolk", "Goblin").
    #[serde(default)]
    pub subtype: Option<String>,
    /// If `Some`, only creatures of this color (Crusade = White, Bad Moon = Black).
    #[serde(default)]
    pub color: Option<Color>,
    /// If `Some`, only creatures whose current layer-one copiable face has this name. Pack
    /// Mastiff and Cylian Sunsinger use name-selected resolving effects; copied permanents match
    /// the name they acquired rather than their physical card definition id.
    #[serde(default)]
    pub name: Option<String>,
    /// If `Some`, only creatures with at least one counter of this kind. Pridemalkin, Abzan
    /// Falconer, Ainok Bond-Kin, and Tuskguard Captain share this static-scope predicate.
    #[serde(default)]
    pub required_counter: Option<CounterKind>,
    /// CR "other ... creatures": exclude the effect's physical source permanent when it is on
    /// the battlefield. A resolving instant or sorcery source cannot itself match this filter.
    #[serde(default)]
    pub exclude_self: bool,
    /// If true, only creatures currently attacking in the authoritative combat assignment match.
    /// One-shot consumers snapshot that membership as they resolve; static abilities re-evaluate
    /// it continuously.
    #[serde(default)]
    pub attacking: bool,
}

impl CreatureScopeFilter {
    pub fn validate(&self) -> Result<(), String> {
        if self
            .name
            .as_ref()
            .is_some_and(|name| name.trim().is_empty())
        {
            return Err("creature scope name cannot be empty".into());
        }
        Ok(())
    }
}

/// Which creature(s) a static damage-prevention ability protects. `Source` covers Anti-Venom;
/// `OtherCreaturesYouControl` covers Vigor and future controller-scoped prevention permanents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DamagePreventionSubject {
    Source,
    OtherCreaturesYouControl,
}

/// Capacity of a static prevention application. Static abilities do not have a total pool: the
/// amount resets for each damage event while the source remains on the battlefield.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StaticDamagePreventionAmount {
    All,
    FixedPerEvent(u32),
}

/// Which quantity an additional prevention effect uses. `Attempted` is the damage still present
/// when this application begins (Anti-Venom); `Prevented` supports wording such as Vigor's
/// "for each 1 damage prevented this way".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PreventionAmountBasis {
    Attempted,
    Prevented,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DamagePreventionAdditionalEffect {
    PutCounters {
        counter: CounterKind,
        basis: PreventionAmountBasis,
    },
}

/// Which permanents an enters-tapped replacement ability affects. `Self_` abilities are the
/// CR 614.12 exception that function on the card before it reaches the battlefield; `Permanents`
/// abilities function only from an existing battlefield source (Orb of Dreams).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntersTappedAffected {
    Self_,
    Permanents,
}

/// One static ability on a permanent (CR 604). Most entries generate a continuous effect while
/// its source is on the battlefield. An ability that modifies how its own object enters is the
/// CR 113.6h/614.12 exception and is inspected during the proposed entry event. Static abilities
/// do not use the stack, unlike triggered and activated abilities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StaticAbilityDef {
    /// CR 614.12 / 707.5: as this permanent enters, its controller may replace its copiable
    /// values with those of a live battlefield permanent matching `filter`. This is a selection,
    /// not a target; Clone and Stunt Double therefore ignore hexproof and shroud.
    EntersAsCopy {
        #[serde(default = "TargetFilter::default_creature")]
        filter: TargetFilter,
    },
    /// CR 614.1d: modify a proposed battlefield-entry event rather than tapping the permanent
    /// after it enters. Intrinsic examples include Diregraf Ghoul and the gainland cycle;
    /// `Permanents` is the global Orb of Dreams form.
    EntersTapped { affected: EntersTappedAffected },
    /// CR 614.1c / 122.6: modify the proposed battlefield-entry event so this permanent starts
    /// with `amount` counters of `counter`. `Amount` keeps fixed, X, conditional, and counted
    /// values on the same numeric vocabulary used by resolving effects. Endless One, Squad
    /// Captain, and Bloodcrazed Paladin exercise three distinct amount sources.
    EntersWithCounters {
        counter: CounterKind,
        amount: Amount,
    },
    /// CR 615: a prevention effect generated continuously while this permanent is on the
    /// battlefield. Anti-Venom protects itself and counts attempted damage; Vigor protects other
    /// creatures its controller controls and counts damage actually prevented.
    PreventDamage {
        subject: DamagePreventionSubject,
        amount: StaticDamagePreventionAmount,
        #[serde(default)]
        additional_effect: Option<DamagePreventionAdditionalEffect>,
    },
    /// CR 613.4 layer 7c: every creature matching `filter` gets +`delta_power`/+`delta_toughness`
    /// (negative values for a debuff anthem). Anthems (Glorious Anthem) and lords (Crusade, Bad Moon).
    AnthemPt {
        #[serde(default)]
        filter: CreatureScopeFilter,
        delta_power: i32,
        delta_toughness: i32,
    },
    /// Continuous modifiers supplied to the permanent currently attached to this Aura or
    /// Equipment. Type additions apply in layer 4, keywords in layer 6, P/T changes in layer 7c,
    /// and combat prohibitions as rule restrictions. The dynamic attachment scope moves every
    /// modifier together when an Equipment is re-equipped. Dub, Holy Strength/Oakenform,
    /// Flight/Swiftfoot Boots, and Pacifism.
    AttachedModifier {
        #[serde(default)]
        add_types: TypeLineAddition,
        #[serde(default)]
        delta_power: i32,
        #[serde(default)]
        delta_toughness: i32,
        #[serde(default)]
        keywords: Vec<Keyword>,
        /// Triggered abilities the attached permanent has while this static ability applies.
        /// Infernal Scarring and similar Auras use the enchanted permanent's controller and
        /// last-known identity when the granted ability triggers.
        #[serde(default)]
        triggered_abilities: Vec<TriggeredAbilityDef>,
        /// Activated abilities the attached permanent has while this static ability applies.
        /// Gift of Paradise and Hermetic Study exercise mana and targeted nonmana abilities.
        #[serde(default)]
        activated_abilities: Vec<ActivatedAbilityDef>,
        #[serde(default)]
        cant_attack: bool,
        #[serde(default)]
        cant_block: bool,
        /// The attached permanent does not untap during its controller's untap step. Other
        /// untap effects remain legal.
        #[serde(default)]
        doesnt_untap_during_untap_step: bool,
    },
    /// CR 613.1b: the controller of this Aura controls the permanent it is attached to.
    /// Mind Control and Confiscate share this source-relative layer-2 ability.
    ControlsAttached,
    /// CR 613 layer 6: every creature matching `filter` gains `keyword` while the source is on the
    /// battlefield. Covers lords (Goblin Chieftain, Captain of the Watch) and keyword-granting
    /// enchantments. Pairs with `AnthemPt` on the same card for combined "+1/+1 and haste" effects.
    AnthemKeyword {
        #[serde(default)]
        filter: CreatureScopeFilter,
        keyword: Keyword,
    },
    /// A self-scoped static effect whose condition is continuously reevaluated. Characteristic
    /// changes apply in their normal layers; the defender exception changes only attack legality.
    /// Daggersail Aeronaut, Drowsing Tyrannodon, Gearsmith Guardian, and Gearsmith Prodigy.
    ConditionalSelfModifier {
        condition: GameCondition,
        #[serde(default)]
        delta_power: i32,
        #[serde(default)]
        delta_toughness: i32,
        #[serde(default)]
        keywords: Vec<Keyword>,
        #[serde(default)]
        can_attack_as_though_without_defender: bool,
    },
    /// CR 305.2b / layer 5: controller may play `count` additional lands per turn while this
    /// permanent is on the battlefield. Exploration, Oracle of Mul Daya.
    ExtraLandPlays { count: u32 },
}
