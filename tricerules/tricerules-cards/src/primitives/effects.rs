//! Spell and continuous-effect vocabulary plus shared effect parameters.

use super::{
    AnthemFilter, GraveyardDestination, GraveyardFilter, Keyword, SpellTypeFilter, TargetFilter,
    TargetKind,
};
use serde::{Deserialize, Serialize};
use std::fmt;

/// An effect amount that is either a fixed literal or the spell's cast-time X (CR 107.3).
///
/// In RON a bare integer (`amount: 3`) is [`Amount::Fixed`]; the string `amount: "X"` is the
/// chosen X, resolved from the resolving stack item's `chosen_x`. Custom (de)serialize keeps the
/// existing integer corpus untouched and roundtrips X as the string `"X"` (RON renders a bare
/// `X` identifier as an ambiguous unit value, so the quoted form is used). Applied to the
/// amount-bearing effects that can legally scale with X — the "name two cards" pair is Fireball
/// (`DamageTarget { amount: "X" }`) and Blue Sun's Zenith (`Draw { count: "X" }`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Amount {
    /// A literal count baked into the card data.
    Fixed(u32),
    /// The spell's cast-time X value (CR 107.3); resolved at resolution from `chosen_x`.
    X,
}

impl Amount {
    /// Resolve to a concrete count given the spell's chosen X (0 for non-X spells).
    pub fn resolve(self, x: u32) -> u32 {
        match self {
            Amount::Fixed(n) => n,
            Amount::X => x,
        }
    }

    /// True if this amount depends on the cast-time X.
    pub fn is_x(self) -> bool {
        matches!(self, Amount::X)
    }
}

impl From<u32> for Amount {
    fn from(n: u32) -> Self {
        Amount::Fixed(n)
    }
}

impl Serialize for Amount {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Amount::Fixed(n) => s.serialize_u32(*n),
            Amount::X => s.serialize_str("X"),
        }
    }
}

impl<'de> Deserialize<'de> for Amount {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct AmountVisitor;
        impl serde::de::Visitor<'_> for AmountVisitor {
            type Value = Amount;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a non-negative integer or the string \"X\"")
            }
            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Amount, E> {
                Ok(Amount::Fixed(v as u32))
            }
            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Amount, E> {
                u32::try_from(v)
                    .map(Amount::Fixed)
                    .map_err(|_| E::custom("amount must be non-negative"))
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Amount, E> {
                if v == "X" {
                    Ok(Amount::X)
                } else {
                    Err(E::custom(format!("unknown amount {v:?}, expected \"X\"")))
                }
            }
        }
        d.deserialize_any(AmountVisitor)
    }
}

/// How a multi-target damage effect assigns its damage. This is shared by Fire (choose the
/// allocation while casting) and Fireball (divide evenly as it resolves), rather than baking a
/// card-specific branch into the engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DamageDivision {
    #[default]
    ChooseAtCast,
    EvenAtResolution,
}

/// How much life a [`SpellEffectKind::LoseLife`] costs.
///
/// Kept separate from [`Amount`] on purpose: `Amount::resolve(x)` answers from the cast-time X
/// alone, while `TargetManaValue` needs the resolving spell's target, so folding it into `Amount`
/// would force a context argument through ~15 call sites where it is meaningless.
///
/// Named for widening — a later `TargetPower` (Rite of Consumption, Fling) or
/// `ManaValueOfRevealedCard` (Dark Confidant) is an added variant, not a rename.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LifeAmount {
    /// A literal amount baked into the card data (Thoughtseize: `Fixed(2)`).
    Fixed(u32),
    /// CR 202.3: the mana value of the object this spell targets (Reanimate). Legal only in an
    /// effect list that also contains a target-bearing effect — enforced at registry load.
    TargetManaValue,
}

/// A kind of counter that can sit on a permanent (CR 122.1). Only the two counter kinds
/// with engine rules interactions exist so far: the +1/+1 / -1/-1 pair, which modify P/T in
/// CR 613.4 layer 7d and annihilate as a state-based action (CR 122.3). Loyalty, charge, and
/// keyword counters are added by their dependent plans (planeswalkers, Chalice-style chargers)
/// when the first card needs them. `Ord` is required so [`crate`] consumers can store counters
/// in a `BTreeMap` for deterministic iteration/serialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum CounterKind {
    /// CR 122 +1/+1 counter — adds 1 to power and toughness each (layer 7d).
    PlusOnePlusOne,
    /// CR 122 -1/-1 counter — subtracts 1 from power and toughness each (layer 7d).
    MinusOneMinusOne,
}

impl CounterKind {
    /// Short human-readable label for client display (e.g. in card annotations).
    /// Matches the conventional MTG counter naming ("+1/+1", "-1/-1").
    pub fn label(self) -> &'static str {
        match self {
            CounterKind::PlusOnePlusOne => "+1/+1",
            CounterKind::MinusOneMinusOne => "-1/-1",
        }
    }
}

/// Where an effect is being resolved from. Controls validation that depends on context —
/// e.g. [`TargetKind::Self_`] is only meaningful for an ability bound to a source permanent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectContext {
    /// A spell's `spell_effect` list (no source permanent to self-reference).
    Spell,
    /// An activated or triggered ability bound to a source permanent.
    Ability,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpellEffectKind {
    DamageTarget {
        amount: Amount,
        target: TargetFilter,
    },
    /// Divide `amount` damage among any number of targets (CR 601.2d). Costs
    /// `extra_mana_per_target` additional generic mana per target beyond the first (Fireball = 1,
    /// Fire = 0). `max_targets` caps the count (Fire = `Some(2)`; `None` = unlimited).
    /// At cast time the player submits `(target, damage_amount)` pairs via `TargetRef`; the sum
    /// must equal `amount.resolve(x_value)`. Covers Fireball (X divided unlimited) and Fire
    /// (fixed 2 divided among ≤ 2 targets). CR 608.2b: if some targets become illegal at
    /// resolution, damage is applied only to the remaining legal targets (partial fizzle).
    DamageTargets {
        amount: Amount,
        target: TargetFilter,
        #[serde(default)]
        division: DamageDivision,
        #[serde(default)]
        extra_mana_per_target: u32,
        #[serde(default)]
        max_targets: Option<u32>,
    },
    Draw {
        count: Amount,
    },
    /// CR 701.18: look at the top `count` cards of your library, put any number of them on the
    /// bottom of it in any order, and the rest back on top in any order. The cards never leave the
    /// library, so this fires no zone-change triggers.
    ///
    /// Resolution suspends for the player's decision — up to two interrupts, the second (ordering
    /// the cards kept on top) skipped when it would be a no-op. Because it suspends, any effect
    /// declared after it resumes via `PendingResolution::resume_effect_index`; `[Scry, Draw]` is
    /// Preordain and Opt.
    Scry {
        count: u32,
    },
    /// Destroy target matching `target` filter (default: any creature on the battlefield).
    /// Characteristic restrictions (e.g. `tapped: true` for Royal Assassin) live in the filter.
    DestroyTarget {
        #[serde(default = "TargetFilter::default_creature")]
        target: TargetFilter,
    },
    /// Give +power/+toughness until end of turn to a creature matching `target`
    /// (default: any creature, for Giant Growth). Use `(kind: Self_)` for an ability that
    /// pumps its own source permanent (e.g. an upkeep self-pump) — auto-bound, untargeted.
    PumpTarget {
        power: i32,
        toughness: i32,
        #[serde(default = "TargetFilter::default_creature")]
        target: TargetFilter,
    },
    /// Tap target permanent matching `target` filter.
    TapTarget {
        target: TargetFilter,
    },
    /// Tap every creature controlled by the selected relative player set. This is untargeted and
    /// snapshots the battlefield as it resolves. Covers Cryptic Command and Tempest Caller.
    TapAllCreatures {
        players: RelativePlayerSet,
    },
    /// CR 701.20: untap target permanent matching `target` filter. The mirror of
    /// [`Self::TapTarget`], and deliberately *not* restricted to tapped permanents — an untapped
    /// permanent is a legal target and the effect simply does nothing (add `tapped: Some(true)`
    /// to the filter only for a card whose own text demands it). Covers Seeker of Skybreak
    /// (`{T}: Untap target creature`) and Aphetto Alchemist.
    UntapTarget {
        target: TargetFilter,
    },
    /// CR 701.20: untap every permanent matching `filter` controlled by `players`. Untargeted, and
    /// snapshots the battlefield as it resolves, like [`Self::TapAllCreatures`]. Controller scope
    /// lives in `players` rather than in the filter's `only_controller`, because untargeted mass
    /// selection goes through `battlefield_objects_matching`, which has no activating player to
    /// compare against. Covers Vitalize (`Controller` + creature) and Early Harvest / Turnabout
    /// (a player's lands).
    UntapAll {
        players: RelativePlayerSet,
        #[serde(default = "TargetFilter::default_creature")]
        filter: TargetFilter,
    },
    /// CR 701.5: counter target spell on the stack. `spell_filter` narrows which spells are legal
    /// targets — `None` is unrestricted (Counterspell), `Some(Creature)` is Essence Scatter,
    /// `Some(Noncreature)` is Negate. Reuses [`SpellTypeFilter`] so any future "counter target
    /// X spell" needs no new variant.
    CounterTargetSpell {
        #[serde(default)]
        spell_filter: Option<SpellTypeFilter>,
    },
    /// CR 707.10: put `count` copies of target spell on the stack, each controlled by this
    /// spell's controller. A copy is **not cast** (no mana, no cast triggers, no storm count) and
    /// ceases to exist after it resolves (CR 707.10d). The copy uses the original's chosen modes,
    /// X, and targets; CR 707.10c lets the copy's controller choose new targets (deferred — copies
    /// keep the original's targets for now). `count` covers Twincast / Fork / Reverberate (1) and
    /// "copy it twice" effects without a new variant. `spell_filter` restricts the legal target
    /// the same way as [`Self::CounterTargetSpell`] — `Some(InstantOrSorcery)` for Twincast /
    /// Reverberate ("copy target instant or sorcery spell"); only spells (not abilities) qualify.
    CopyTargetSpell {
        #[serde(default = "one")]
        count: u32,
        #[serde(default)]
        spell_filter: Option<SpellTypeFilter>,
    },
    /// CR 613.4 layer 7c: give every creature matching `filter` +power/+toughness until end of
    /// turn (the mass, one-shot sibling of [`Self::PumpTarget`]). Untargeted — `filter` selects
    /// the set the same way a static anthem does. Glorious Charge / Inspired Charge
    /// (`controller: YouControl`); attacking-creature pumps reuse the same filter machinery.
    PumpAll {
        #[serde(default)]
        filter: AnthemFilter,
        power: i32,
        toughness: i32,
    },
    /// CR 303.4: the aura's "Enchant [type]" clause. Authored in `spell_effect` of every Aura
    /// enchantment — it is the sole effect that requires a target during casting, and at resolution
    /// it records the attachment (engine sets `attached_to` before processing this effect). The
    /// `target` filter mirrors the card's "Enchant [type]" line; default is any creature. Validated
    /// at registry load to reject player-kind filters (auras enchant permanents, CR 303.4a).
    AuraAttach {
        #[serde(default = "TargetFilter::default_creature")]
        target: TargetFilter,
    },
    /// CR 613 layer 6: grant one or more keyword abilities to every creature matching `filter`
    /// until end of turn. Untargeted — the one-shot keyword-grant sibling of
    /// [`StaticAbilityDef::AnthemKeyword`]. Covers Overrun (Trample to all your creatures) and
    /// Trumpet Blast (First Strike to attacking creatures you control until EOT).
    GrantKeywordsAll {
        #[serde(default)]
        filter: AnthemFilter,
        keywords: Vec<Keyword>,
    },
    /// CR 613 layer 6: grant one or more keyword abilities to target permanent until end of
    /// turn. Covers Boros Charm (Double Strike) and Temur Battle Rage (Double Strike + Trample).
    GrantKeywordsTarget {
        #[serde(default = "TargetFilter::default_creature")]
        target: TargetFilter,
        keywords: Vec<Keyword>,
    },
    /// CR 613 layer 6: grant keywords until end of turn to the permanents matching `filter`.
    /// The affected set is snapshotted as this effect resolves, rather than remaining dynamic.
    /// Covers Boros Charm and Heroic Intervention.
    GrantKeywordsAllPermanents {
        filter: TargetFilter,
        keywords: Vec<Keyword>,
    },
    GainLife {
        amount: Amount,
    },
    /// CR 118: the effect's controller loses life. **Untargeted** — "you lose life" does not
    /// target (CR 115.1), so this is deliberately absent from `spell_effect_kind_needs_target`
    /// and from [`SpellEffectKind::target_filters`]. Adding it to either would make every
    /// `LoseLife`-only card demand a target.
    /// Two cards: Thoughtseize (`Fixed(2)`), Reanimate (`TargetManaValue`).
    LoseLife {
        amount: LifeAmount,
    },
    TargetPlayerGainsLife {
        amount: u32,
        target: TargetFilter,
    },
    TargetPlayerLosesLife {
        amount: u32,
        target: TargetFilter,
    },
    EachOpponentLosesLifeYouGainEqual {
        amount: u32,
    },
    ExileTarget,
    ExileTargetGainLifeEqualToPower,
    ReturnTargetCreatureToHand,
    ReturnTargetPermanentToHand,
    /// Move a card from a graveyard to hand or battlefield (CR 400.1: graveyard is public).
    /// Raise Dead / Disentomb (creature → hand); future reanimation (creature → battlefield).
    /// The `filter` selects which graveyard and which card types are legal targets; the engine
    /// validates this at cast time and again at resolution (fizzle if no longer legal).
    ReturnFromGraveyard {
        filter: GraveyardFilter,
        destination: GraveyardDestination,
    },
    MillTargetPlayer {
        count: u32,
        target: TargetFilter,
    },
    /// CR 701.7a: force `count` cards from the target player's hand to their graveyard.
    /// `random: true` chooses cards at random (Hymn to Tourach); `random: false` lets
    /// the spell's controller choose from the revealed hand (Coercion, Thoughtseize).
    /// When `count` exceeds the hand size, the player discards all remaining cards.
    /// Two cards covered: Hymn to Tourach (`random: true`); Coercion (`random: false`).
    DiscardCards {
        count: u32,
        target: TargetFilter,
        /// True = at random (Hymn to Tourach). False = caster chooses from revealed hand (Coercion).
        #[serde(default)]
        random: bool,
    },
    /// Destroy every battlefield permanent matching `kind` (CR 701.7). Untargeted, so it
    /// ignores hexproof/shroud and never fizzles. `kind` selects the affected set — `Creature`
    /// for Wrath of God / Day of Judgment, `AnyPermanent` for "destroy all permanents". Only
    /// object kinds are legal (validated at load); player kinds make no sense here.
    /// `prevent_regeneration: true` means regeneration shields are bypassed (Wrath of God:
    /// "they can't be regenerated", CR 701.15b).
    DestroyAll {
        #[serde(default = "TargetFilter::default_creature")]
        kind: TargetFilter,
        #[serde(default)]
        prevent_regeneration: bool,
    },
    /// CR 701.15: put a regeneration shield on target creature. The next time that creature would
    /// be destroyed this turn, instead tap it, remove it from combat, and clear all damage from it.
    /// Legal only as an activated ability effect — never a spell (validated at load). Covers
    /// Cudgel Troll (`{G}: Regenerate`) and Drudge Skeletons (`{B}: Regenerate`).
    Regenerate {
        #[serde(default = "TargetFilter::default_creature")]
        target: TargetFilter,
    },
    /// Deal `amount` damage to every battlefield permanent matching `kind` (CR 119). Untargeted.
    /// `Creature` covers Pyroclasm / Pestilence-style sweeps; `AnyPermanent` is reserved for
    /// future "damage to each permanent" effects. Only object kinds are legal (validated at load).
    DamageAll {
        amount: u32,
        #[serde(default = "TargetFilter::default_creature")]
        kind: TargetFilter,
    },
    /// CR 111: create `count` token permanents of the registry-defined [`token`](crate::token_def)
    /// under the chosen controller. Untargeted — the characteristics come from the
    /// [`TokenDefinition`](crate::token_def::TokenDefinition); only `count` and `controller` vary
    /// per maker. Covers Raise the Alarm / Dragon Fodder (`Controller`, count 2) and symmetrical
    /// makers (`EachPlayer`). Token *copies* of existing permanents (CR 707) are a separate effect.
    CreateTokens {
        /// Token id (slug of the token's name) in the registry's token namespace.
        token: String,
        count: u32,
        #[serde(default)]
        controller: TokenController,
    },
    /// CR 122/121.6: put `count` counters of `counter` on a creature matching `target`
    /// (default: any creature). The `counter` kind covers both +1/+1 counter spells
    /// (Battlegrowth, Common Bond) and -1/-1 counter spells (Instill Infection) without a new
    /// variant. Use `(kind: Self_)` for an ability that puts counters on its own source
    /// (modular/graft/outlast self-buffs). Counter *removal* spells are deferred — counter
    /// removal in MTG is almost always an ability cost (see the plan's `AbilityCost` phase).
    PutCounters {
        counter: CounterKind,
        count: u32,
        #[serde(default = "TargetFilter::default_creature")]
        target: TargetFilter,
    },
    /// CR 119 + 119.4: drain `amount` life from a target player and give that much life to the
    /// controller ("target player loses N life and you gain N life"). Covered by Blood Artist,
    /// Falkenrath Noble, and drain-life spells like Vampire's Kiss. The target must be a player.
    DrainTarget {
        amount: u32,
        target: TargetFilter,
    },
    /// CR 605 mana ability: add mana to the activating player's pool. Legal only as an
    /// activated ability's `effect` (never a spell `spell_effect`) — the engine classifies an
    /// ability with this effect as a mana ability (CR 605.1a), so it doesn't use the stack and
    /// resolves immediately. `options` lists the mutually exclusive bags of mana producible;
    /// one option = no choice (basic lands, Llanowar Elves, Sol Ring), several = the player
    /// picks at activation (dual/filter lands, any-color rocks). Untargeted.
    ProduceMana {
        options: Vec<ManaAmount>,
    },
    /// CR 701.18: pause resolution, let the casting player search their library for a card
    /// matching `filter` (None = any card; Some = only cards of that spell type), move it to
    /// `destination`, then shuffle if `shuffle` is true. Uses the tier-3 interrupt mechanism
    /// (`ResolutionChoiceRequired` / `SubmitResolutionChoice`) with `ChoiceKind::LibrarySearch`
    /// (private to the searching player). Named two: Demonic Tutor (any → hand),
    /// Mystical Tutor (instant or sorcery → top of library).
    SearchLibrary {
        /// `None` = any card is valid; `Some(f)` = only cards of this type qualify.
        #[serde(default)]
        filter: Option<SpellTypeFilter>,
        /// Where the found card goes. Default: Hand.
        #[serde(default)]
        destination: SearchDestination,
        /// Shuffle the library after searching. Default: true (all canonical tutors shuffle).
        #[serde(default = "default_true")]
        shuffle: bool,
        /// Reveal the found card publicly (Mystical Tutor reveals to both players). Default: false.
        #[serde(default)]
        reveal: bool,
    },
    /// CR 301.5 / 702.6: the equip activated ability — attach this equipment to `target` creature
    /// you control. At resolution the engine moves `attached_to` on the equipment's `GameObject`
    /// to the new target (detaching from any previous creature automatically). The P/T bonus
    /// (if any) is a separate [`StaticAbilityDef::EquippedBonus`] that reads `attached_to`
    /// dynamically, so no continuous effect is updated on re-equip. Legal only as an activated
    /// ability's `effect`, never a spell effect; equip only as a sorcery (CR 702.6a).
    /// Covers Bonesplitter (equip {1}) and Vulshok Morningstar (equip {2}).
    Equip {
        #[serde(default = "TargetFilter::default_equip")]
        target: TargetFilter,
    },
    /// CR 701.17: force the targeted player to sacrifice a permanent matching `filter` (default:
    /// any creature). The target is specified by `target` (kind must be AnyPlayer or
    /// OpponentPlayer — validated at registry load). The targeted player chooses which qualifying
    /// permanent to sacrifice; if they have none the effect fizzles. Covers Diabolic Edict
    /// (opponent sacrifices a creature) and Plaguecrafter (target player sacrifices a creature).
    TargetPlayerSacrifices {
        /// Who must sacrifice — kind must be AnyPlayer or OpponentPlayer.
        target: TargetFilter,
        /// What kind of permanent may be sacrificed (default: Creature).
        #[serde(default = "TargetFilter::default_creature")]
        filter: TargetFilter,
    },
    /// CR 614.1a: place a prevention shield on the target (creature or player) that will absorb
    /// the next `amount` damage dealt to it this turn. Healing Salve mode 2 (`amount: 3,
    /// target: AnyTarget`); Circle of Protection variants use `amount: 1` and repeat.
    /// When damage would be dealt to a shielded object, subtract up to `amount` from the shield
    /// before recording the damage; the shield expires when fully consumed or at cleanup.
    PreventNextDamage {
        amount: u32,
        target: TargetFilter,
    },
    /// CR 614.1a: prevent all combat damage that would be dealt this turn (Fog, Holy Day,
    /// Safe Passage partial). Untargeted — sets a global flag checked when combat damage resolves.
    /// Cleared at the cleanup step alongside marked damage.
    PreventAllCombatDamageTurn,
    None,
}

fn default_true() -> bool {
    true
}

/// Where a searched-out card goes (for [`SpellEffectKind::SearchLibrary`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SearchDestination {
    /// The card goes to the searching player's hand (Demonic Tutor, Cultivate).
    #[default]
    Hand,
    /// The card is placed on top of the searching player's library (Mystical Tutor).
    TopOfLibrary,
}

/// One bag of mana a mana ability can produce (CR 106): a count per mana type. A mana ability's
/// [`SpellEffectKind::ProduceMana`] carries a `Vec<ManaAmount>` of *options*; with one option the
/// ability produces it unconditionally (basic Forest `(g: 1)`, Sol Ring `(c: 2)`, Llanowar Elves
/// `(g: 1)`), with several the activating player picks one (a dual land's two colors; "any color"
/// enumerated as five single-color options). Serde defaults every field to 0 so RON lists only the
/// nonzero types (`(g: 1)`, `(w: 1, u: 1)`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ManaAmount {
    #[serde(default)]
    pub w: u32,
    #[serde(default)]
    pub u: u32,
    #[serde(default)]
    pub b: u32,
    #[serde(default)]
    pub r: u32,
    #[serde(default)]
    pub g: u32,
    #[serde(default)]
    pub c: u32,
}

/// Who receives the tokens made by [`SpellEffectKind::CreateTokens`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TokenController {
    /// The spell/ability's controller (the common case: Raise the Alarm, Krenko).
    #[default]
    Controller,
    /// Each player still in the game gets `count` tokens (symmetrical makers).
    EachPlayer,
}

/// Which players' creatures a mass one-shot effect affects, relative to the effect controller.
/// Kept separate from target selection because these effects do not target. Covers Cryptic
/// Command / Tempest Caller (`Opponents`) and controller-only mass tap/untap effects (`Controller`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelativePlayerSet {
    Controller,
    Opponents,
    All,
}

impl SpellEffectKind {
    /// The target filter(s) this effect selects against, if any. Used by validation and by
    /// the engine's generic legality/targeting paths (one place to enumerate target-bearing
    /// variants instead of repeating the list).
    pub fn target_filters(&self) -> Vec<&TargetFilter> {
        match self {
            SpellEffectKind::DamageTarget { target, .. }
            | SpellEffectKind::DamageTargets { target, .. }
            | SpellEffectKind::DestroyTarget { target }
            | SpellEffectKind::PumpTarget { target, .. }
            | SpellEffectKind::GrantKeywordsTarget { target, .. }
            | SpellEffectKind::TapTarget { target }
            | SpellEffectKind::UntapTarget { target }
            | SpellEffectKind::TargetPlayerGainsLife { target, .. }
            | SpellEffectKind::TargetPlayerLosesLife { target, .. }
            | SpellEffectKind::DrainTarget { target, .. }
            | SpellEffectKind::MillTargetPlayer { target, .. }
            | SpellEffectKind::DiscardCards { target, .. }
            | SpellEffectKind::PutCounters { target, .. }
            | SpellEffectKind::AuraAttach { target }
            | SpellEffectKind::Equip { target }
            | SpellEffectKind::Regenerate { target }
            | SpellEffectKind::TargetPlayerSacrifices { target, .. }
            | SpellEffectKind::PreventNextDamage { target, .. } => vec![target],
            _ => vec![],
        }
    }

    /// True if this effect selects an *object* target (not a player). Used by the effect-list
    /// validation below to decide whether a `LifeAmount::TargetManaValue` has something to read.
    fn targets_an_object(&self) -> bool {
        // `ReturnFromGraveyard` carries a `GraveyardFilter`, not a `TargetFilter`, so it is
        // invisible to `target_filters()` — it still targets an object (a graveyard card).
        matches!(self, SpellEffectKind::ReturnFromGraveyard { .. })
            || self.target_filters().iter().any(|f| !f.is_player())
    }

    /// Startup validation for a whole effect list (one face's `spell_effect`, or one mode's
    /// effects). Rules that depend on *sibling* effects live here; per-effect rules live in
    /// [`SpellEffectKind::validate`], which the caller runs too.
    pub fn validate_list(effects: &[SpellEffectKind]) -> Result<(), String> {
        // `TargetManaValue` reads the mana value of the spell's target, so the list must contain
        // an effect that declares an object target (Reanimate: `ReturnFromGraveyard`). Without
        // one there is nothing to read and the amount would silently resolve to 0.
        if effects.iter().any(|e| {
            matches!(
                e,
                SpellEffectKind::LoseLife {
                    amount: LifeAmount::TargetManaValue
                }
            )
        }) && !effects.iter().any(|e| e.targets_an_object())
        {
            return Err(
                "LoseLife(amount: TargetManaValue) requires an object-targeting effect in the \
                 same effect list to read the mana value from"
                    .into(),
            );
        }
        Ok(())
    }

    /// Startup validation: reject effect/filter combinations the engine cannot honor.
    /// Returns `Err` with a human-readable reason; called from the card registry loader.
    /// `context` distinguishes spells from abilities so context-only filters (`Self_`) are
    /// rejected where they make no sense.
    pub fn validate(&self, context: EffectContext) -> Result<(), String> {
        // CR 115: a self-referencing ability effect is not "targeting" and only exists where
        // there is a source permanent — never in a spell's effect list.
        if context == EffectContext::Spell
            && self
                .target_filters()
                .iter()
                .any(|f| f.kind == TargetKind::Self_)
        {
            return Err(
                "Self_ target is only valid on an activated or triggered ability, not a spell"
                    .into(),
            );
        }
        match self {
            SpellEffectKind::TargetPlayerGainsLife { target, .. }
            | SpellEffectKind::TargetPlayerLosesLife { target, .. }
            | SpellEffectKind::DrainTarget { target, .. }
            | SpellEffectKind::MillTargetPlayer { target, .. }
            | SpellEffectKind::DiscardCards { target, .. } => {
                if target.is_player() {
                    Ok(())
                } else {
                    Err(format!(
                        "player-targeted effect requires AnyPlayer or OpponentPlayer kind, got {:?}",
                        target.kind
                    ))
                }
            }
            // CR 701.19/701.20: tapping and untapping act on permanents, never players.
            SpellEffectKind::TapTarget { target } | SpellEffectKind::UntapTarget { target } => {
                if target.is_player() {
                    Err(format!(
                        "tap/untap cannot target players, got {:?}",
                        target.kind
                    ))
                } else {
                    Ok(())
                }
            }
            // CR 122: counters go on permanents, never players.
            SpellEffectKind::PutCounters { target, .. } => {
                if target.is_player() {
                    Err(format!(
                        "PutCounters cannot target players, got {:?}",
                        target.kind
                    ))
                } else {
                    Ok(())
                }
            }
            // TargetPlayerSacrifices targets a player (kind must be AnyPlayer/OpponentPlayer),
            // and the sacrifice filter must select objects not players.
            SpellEffectKind::TargetPlayerSacrifices { target, filter } => {
                if !target.is_player() {
                    return Err(format!(
                        "TargetPlayerSacrifices.target must be AnyPlayer or OpponentPlayer, got {:?}",
                        target.kind
                    ));
                }
                if filter.is_player() {
                    return Err(format!(
                        "TargetPlayerSacrifices.filter must select permanents, not players, got {:?}",
                        filter.kind
                    ));
                }
                Ok(())
            }
            // Mass effects select objects, not players, and never use Self_/AnyTarget (which
            // include players). Only Creature / AnyPermanent are honored by the engine.
            SpellEffectKind::DestroyAll { kind, .. }
            | SpellEffectKind::DamageAll { kind, .. }
            | SpellEffectKind::UntapAll { filter: kind, .. } => {
                if matches!(kind.kind, TargetKind::Creature | TargetKind::AnyPermanent) {
                    Ok(())
                } else {
                    Err(format!(
                        "mass effect kind must be Creature or AnyPermanent, got {:?}",
                        kind.kind
                    ))
                }
            }
            SpellEffectKind::GrantKeywordsTarget { target, keywords } => {
                if target.is_player() {
                    Err(format!(
                        "GrantKeywordsTarget cannot target players, got {:?}",
                        target.kind
                    ))
                } else if keywords.is_empty() {
                    Err("GrantKeywordsTarget requires at least one keyword".into())
                } else {
                    Ok(())
                }
            }
            SpellEffectKind::GrantKeywordsAllPermanents { filter, keywords } => {
                if !matches!(filter.kind, TargetKind::Creature | TargetKind::AnyPermanent) {
                    Err(format!(
                        "GrantKeywordsAllPermanents filter must be Creature or AnyPermanent, got {:?}",
                        filter.kind
                    ))
                } else if keywords.is_empty() {
                    Err("GrantKeywordsAllPermanents requires at least one keyword".into())
                } else {
                    Ok(())
                }
            }
            // CR 605.1a: a mana ability is an activated/triggered ability — never a spell. An
            // empty option set would produce nothing and is rejected as malformed.
            SpellEffectKind::ProduceMana { options } => {
                if context == EffectContext::Spell {
                    Err("ProduceMana is only valid on a mana ability, not a spell".into())
                } else if options.is_empty() {
                    Err("ProduceMana requires at least one mana option".into())
                } else {
                    Ok(())
                }
            }
            // CR 701.18: library search uses the stack interrupt machinery; it is a spell effect
            // only — using it on a mana ability makes no sense.
            SpellEffectKind::SearchLibrary { .. } => {
                if context != EffectContext::Spell {
                    Err("SearchLibrary is only valid on a spell, not a mana ability".into())
                } else {
                    Ok(())
                }
            }
            // CR 701.18: scry is legal on spells and on abilities alike (scry lands, Sensei's
            // Divining Top-style activations), so the only malformed case is scrying zero cards.
            SpellEffectKind::Scry { count } => {
                if *count == 0 {
                    Err("Scry requires a count of at least 1".into())
                } else {
                    Ok(())
                }
            }
            // CR 303.4a: an aura enchants a permanent (never a player).
            SpellEffectKind::AuraAttach { target } => {
                if target.is_player() {
                    Err(
                        "AuraAttach cannot target players; auras enchant permanents (CR 303.4a)"
                            .into(),
                    )
                } else {
                    Ok(())
                }
            }
            // CR 702.6a: equip is an activated ability that only attaches to creatures you
            // control — never a spell effect, and the filter must be creature-typed.
            SpellEffectKind::Equip { target } => {
                if context == EffectContext::Spell {
                    Err("Equip is only valid on an activated ability, not a spell".into())
                } else if target.is_player() {
                    Err(format!(
                        "Equip cannot target players, got {:?}",
                        target.kind
                    ))
                } else {
                    Ok(())
                }
            }
            // CR 701.15: Regenerate puts a shield on the target; it is an activated ability, not
            // a spell. Applying a regeneration shield via a spell would have no source permanent
            // to attach the replacement to and is a nonsensical card design — reject early.
            SpellEffectKind::Regenerate { .. } => {
                if context == EffectContext::Spell {
                    Err("Regenerate is only valid on an activated or triggered ability, not a spell"
                        .into())
                } else {
                    Ok(())
                }
            }
            _ => Ok(()),
        }
    }
}

/// Serde default for `CopyTargetSpell.count` — the overwhelmingly common "make one copy".
fn one() -> u32 {
    1
}

// ---------------------------------------------------------------------------
// Continuous effects (layer system, CR 613)
// ---------------------------------------------------------------------------

/// How long a continuous effect lasts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EffectDuration {
    /// Expires at the next cleanup step (CR 514.2). One-shot effects created by a resolving
    /// spell or ability (Giant Growth, firebreathing) — independent of their source once made
    /// (CR 611.2g), so they persist even if the source permanent later leaves the battlefield.
    UntilEndOfTurn,
    /// CR 604.3 / 611.3: a continuous effect generated by a permanent's *static* ability (an
    /// anthem such as Glorious Anthem or Lord of Atlantis). It exists only while that permanent
    /// is on the battlefield, so the engine drains it when the source leaves (LTB), not at
    /// cleanup. The source is identified by [`ContinuousEffect::source_id`].
    WhileSourceOnBattlefield,
}

/// The kind of modification a continuous effect applies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContinuousEffectKind {
    /// CR 613 layer 7c — modifying effects (+N/+N, -N/-N).
    PtModify {
        delta_power: i32,
        delta_toughness: i32,
    },
    /// CR 613 layer 6 — grant a keyword ability to affected permanents. Covers lords
    /// (Goblin Chieftain → Haste), pump sorceries (Overrun → Trample), and any
    /// "creatures you control gain [keyword] until end of turn" effect.
    Layer6AddKeyword(Keyword),
    /// CR 305.2b / layer 5 (rule-change): controller may play `count` additional lands per turn.
    /// Covers Exploration, Oracle of Mul Daya, and similar enchantments/permanents.
    ExtraLandPlays(u32),
    // Future: Layer7bSetPt { power: i32, toughness: i32 }, …
}
