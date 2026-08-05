//! Tier-3 custom card resolution — the card model's escape hatch (see `CLAUDE.md`).
//!
//! Tiers 1 (RON data) and 2 (generic primitives) describe a card's resolution as
//! `(effect_kind, parameters)` static data. A handful of cards instead need a *resolution
//! algorithm*: a mid-resolution player choice over live objects (Brainstorm), or multiple
//! players choosing interdependently over one revealed set (Gifts Ungiven). Those land here,
//! one [`CardEffect`] per file, keyed by card id.
//!
//! ## The narrow surface (do not widen it into a scripting layer)
//! A card may only resolve via this module if a reviewer agrees no `(effect_kind, parameters)`
//! description exists — prefer widening a primitive every time it is close. Custom code never
//! touches `&mut GameState` directly: it drives a [`ResolutionCtx`], a capability-narrowed view
//! that exposes only audited mutators which preserve zone integrity, keeping the engine the
//! single writer of state.
//!
//! ## Adding one
//! Create `src/custom/<card_id>.rs` — anywhere under that tree, subdirectories are fine — and
//! export `pub(crate) static EFFECT: &dyn CardEffect = &YourType;`. `build.rs` scans the
//! directory and registers it; no shared file is edited and the key is declared nowhere in Rust,
//! because **the file stem is the card id**. The card's RON `custom_effect` stays the single
//! declaration of the binding. Shared, non-effect helper modules go under `src/custom/support/`,
//! which the scan skips.
//!
//! Effects are **1:1 with card ids**, exactly like RON data cards; two files claiming one id is a
//! build error. Two cards wanting the same algorithm is the signal to widen a primitive (tier 2),
//! not to share an impl — a shared algorithm *is* the `(effect_kind, parameters)` description
//! whose absence is the only reason a card is admitted here.
//!
//! ## Resumable resolution
//! [`CardEffect::begin`] starts resolving and either finishes ([`ResolutionStep::Done`]) or
//! returns a [`ResolutionInterrupt`] requesting input from a specific player. The engine parks
//! the in-flight spell (`GameState::pending_resolution`), emits the request, and on the logged
//! choice command calls [`CardEffect::resume`], looping until `Done`. Because every choice is a
//! logged command, replay (`(seed, command log) → state`) reconstructs the same resolution.

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::state::{GameObject, GameState, ObjectId, PlayerId, Zone};
use tricerules_cards::{CardDefinition, CardRegistry};
use tricerules_proto::ruled::v1 as rv1;

// `pub(crate) mod <card_id>;` per file under `src/custom/`, plus the `EFFECT_IMPLS` table they
// feed. Generated so adding a custom card edits no shared file — see `build.rs`.
include!(concat!(env!("OUT_DIR"), "/custom_effects.rs"));

/// A card-specific resolution algorithm the data tiers cannot express. Implementations are
/// unit structs, one per file, registered by card id in [`lookup`].
pub trait CardEffect: Send + Sync {
    /// Begin resolving. Either completes, or requests a player choice via a [`ResolutionInterrupt`].
    /// Any randomness must come from the engine's seeded source (none of the current effects need it).
    fn begin(&self, ctx: &mut ResolutionCtx) -> ResolutionStep;

    /// Resume after the engine validated and collected the player's `choice` for the prior
    /// interrupt. `ctx.step` counts choices already answered; `ctx.scratch` carries object ids
    /// between steps (e.g. the cards Gifts revealed). May request a further choice.
    fn resume(&self, ctx: &mut ResolutionCtx, choice: &ResolutionChoice) -> ResolutionStep;
}

/// Outcome of a [`CardEffect`] step: finished, or blocked on a player input request.
pub enum ResolutionStep {
    Done,
    NeedsChoice(ResolutionInterrupt),
}

/// A request for one player to choose among `candidates` (object ids). Generic across every
/// tier-3 card (and reused later by X-spells / modal spells): the engine validates the response
/// against `candidates`/`min`/`max` before resuming, so per-card proto is never needed.
pub struct ResolutionInterrupt {
    /// Who must answer (CR allows the *opponent* to decide for part of Gifts Ungiven).
    pub deciding_player: PlayerId,
    /// Display-only prompt text (server-only; no Oracle lookup on the relay).
    pub prompt: String,
    /// What the candidate ids refer to, so the UI can present them correctly.
    pub choice_kind: ChoiceKind,
    pub candidates: Vec<ObjectId>,
    pub min: u32,
    pub max: u32,
    /// True when the chosen order is significant (Brainstorm: order the cards put back on top).
    pub ordered: bool,
    /// True when all chosen objects must have distinct card names (Gifts Ungiven: "different names").
    /// The engine enforces this at choice submission; the card effect does not need to re-check.
    pub unique_names: bool,
}

/// What the candidate object ids in a [`ResolutionInterrupt`] are, for client presentation and,
/// crucially, for hidden-information redaction by the relay. This *is* the proto enum
/// (`ruled.v1.ChoiceKind`) rather than a parallel Rust copy kept in sync by hand — the variant
/// docs and the private/public classification live in `ruled_v1.proto`.
pub use rv1::ChoiceKind;

/// A player's answer to a [`ResolutionInterrupt`]: the chosen object ids, in order when the
/// interrupt was `ordered`. Validated against the interrupt's candidates/min/max by the engine.
pub struct ResolutionChoice {
    pub object_ids: Vec<ObjectId>,
}

/// Capability-narrowed view of the engine handed to a [`CardEffect`]. Wraps `&mut GameState`
/// but exposes only audited mutators that maintain zone integrity (the same operations the
/// engine's primitive resolution uses), so custom code cannot corrupt invariants.
pub struct ResolutionCtx<'a> {
    state: &'a mut GameState,
    registry: &'static CardRegistry,
    events: &'a mut Vec<rv1::RuledEvent>,
    /// The spell's controller (CR 608.1: the resolving spell's controller makes its choices,
    /// except where the effect hands a choice to another player).
    pub controller: PlayerId,
    /// Number of choices already answered (0 during `begin`).
    pub step: u32,
    /// Object ids carried between steps (effect-private; e.g. Gifts' revealed cards).
    pub scratch: Vec<ObjectId>,
}

impl<'a> ResolutionCtx<'a> {
    pub(crate) fn new(
        state: &'a mut GameState,
        registry: &'static CardRegistry,
        events: &'a mut Vec<rv1::RuledEvent>,
        controller: PlayerId,
        step: u32,
        scratch: Vec<ObjectId>,
    ) -> Self {
        ResolutionCtx {
            state,
            registry,
            events,
            controller,
            step,
            scratch,
        }
    }

    /// Draw `n` cards for `player` (CR 120). Returns the drawn object ids (fewer than `n` if the
    /// library empties). CR 120.3 / 104.3c: *attempting* to draw from an empty library makes the
    /// player lose as a state-based action — so if the library runs out before `n` cards are drawn,
    /// we flag the player `has_lost` (the engine's post-command `sweep_life` then names the winner),
    /// exactly as the primitive `Draw` effect does. Silently stopping would let a player Brainstorm
    /// into an empty library without decking out.
    pub fn draw(&mut self, player: PlayerId, n: u32) -> Vec<ObjectId> {
        let Some(idx) = self.state.player_idx(player) else {
            return Vec::new();
        };
        let mut drawn = Vec::new();
        let mut decked_out = false;
        for _ in 0..n {
            let Some(oid) = self.state.players[idx].library.pop_front() else {
                decked_out = true;
                break;
            };
            self.state.players[idx].hand.push(oid);
            if let Some(o) = self.state.objects.get_mut(&oid) {
                o.zone = Zone::Hand;
            }
            drawn.push(oid);
        }
        if decked_out {
            self.state.players[idx].has_lost = true;
            self.log(format!(
                "P{player} tried to draw from an empty library and loses (CR 104.3c)."
            ));
        }
        drawn
    }

    /// The object ids in `player`'s hand, in hand order.
    pub fn hand(&self, player: PlayerId) -> Vec<ObjectId> {
        self.state
            .player_idx(player)
            .map(|i| self.state.players[i].hand.clone())
            .unwrap_or_default()
    }

    /// The object ids in `player`'s library, top (next draw) first.
    pub fn library(&self, player: PlayerId) -> Vec<ObjectId> {
        self.state
            .player_idx(player)
            .map(|i| self.state.players[i].library.iter().copied().collect())
            .unwrap_or_default()
    }

    /// Put `oids` on top of their owner's library, `oids[0]` ending up on top (CR 120: "in any
    /// order" is the player's chosen order, passed top-first). The cards are pulled from whatever
    /// zone they are in first, so this also serves "move to top of library".
    ///
    /// **Emits no event, deliberately** — the library is a hidden zone, so per
    /// [`public_move_event_destination`] there is nothing a public event may say about it. This is
    /// the mutator where that matters most: announcing a Brainstorm put-back would tell the
    /// opponent exactly which cards were hidden on top. Recipients learn the new library through
    /// the per-player, per-recipient-redacted zone view instead.
    pub fn put_on_top_of_library(&mut self, oids: &[ObjectId]) {
        // Push in reverse so oids[0] is the final front (top) of the library.
        for &oid in oids.iter().rev() {
            let Some(owner) = self.state.objects.get(&oid).map(|o| o.owner) else {
                continue;
            };
            let Some(idx) = self.state.player_idx(owner) else {
                continue;
            };
            // Not `move_object_to_zone`: that appends to the *bottom* of the library, and the
            // whole point here is the top. Removal still has to sweep every player — `battlefield`
            // is keyed by controller, so an owner-only retain would strand a ghost oid there.
            for p in &mut self.state.players {
                p.hand.retain(|&x| x != oid);
                p.library.retain(|&x| x != oid);
                p.battlefield.retain(|&x| x != oid);
                p.graveyard.retain(|&x| x != oid);
                p.exile.retain(|&x| x != oid);
            }
            self.state.players[idx].library.push_front(oid);
            if let Some(o) = self.state.objects.get_mut(&oid) {
                o.zone = Zone::Library;
            }
        }
    }

    /// Shuffle `player`'s library (CR 701.19 "then shuffle"). Deterministic: the mix folds the
    /// game seed with the command index so replay reproduces the order, while distinct searches
    /// in one game shuffle differently.
    pub fn shuffle_library(&mut self, player: PlayerId) {
        if let Some(idx) = self.state.player_idx(player) {
            let mix = self
                .state
                .seed
                .wrapping_add(self.state.command_index.wrapping_mul(0x9E37_79B9_7F4A_7C15))
                ^ (player as u64);
            crate::engine::shuffle_player_library(self.state, idx, mix);
        }
    }

    /// Move `oid` to `zone` (graveyard/hand/exile/…), maintaining zone membership lists. Emits a
    /// `PermanentMoved` event exactly for the public destinations
    /// ([`public_move_event_destination`]) — the per-player zone-view sync carries hand, library
    /// and battlefield in full, and the graveyard only as object ids, so the relay needs an
    /// explicit move to learn *which card* landed in a graveyard or exile (the same event the
    /// mill/discard paths emit).
    pub fn move_to_zone(&mut self, oid: ObjectId, zone: Zone) {
        let Some(owner) = self.state.objects.get(&oid).map(|o| o.owner) else {
            return;
        };
        let Some(idx) = self.state.player_idx(owner) else {
            return;
        };
        let _ = idx;
        // Delegate rather than re-implementing zone bookkeeping: the canonical mover also removes
        // the oid from *every* player's lists (`battlefield` is keyed by controller, not owner),
        // resets the CR 400.7 new-object state, and drains the source's continuous effects. Both
        // of today's callers move library/hand cards, so those extra branches are inert here —
        // but a second copy of this logic is exactly how the owner-only-retain bug spreads.
        if crate::engine::move_object_to_zone(self.state, self.registry, oid, zone, None).is_err() {
            return;
        }
        if let Some(dest) = public_move_event_destination(zone) {
            self.events.push(crate::engine::permanent_moved_event(
                self.state, oid, owner, dest,
            ));
        }
    }

    /// The game object for `oid`, if it exists.
    pub fn object(&self, oid: ObjectId) -> Option<&GameObject> {
        self.state.objects.get(&oid)
    }

    /// The card definition backing `oid` (registry-owned; rules data, never Oracle display).
    pub fn card_def(&self, oid: ObjectId) -> Option<&'static CardDefinition> {
        let card_id = &self.state.objects.get(&oid)?.card_id;
        self.registry.get(card_id)
    }

    /// The card's name for `oid` (for prompt text), or `"card"` if unknown.
    pub fn card_name(&self, oid: ObjectId) -> String {
        self.card_def(oid)
            .map(|d| d.name.clone())
            .unwrap_or_else(|| "card".to_string())
    }

    /// The opponent of `player` in a 1v1 game (CR 102.1), if there is exactly one.
    pub fn opponent_of(&self, player: PlayerId) -> Option<PlayerId> {
        let others: Vec<PlayerId> = self
            .state
            .players
            .iter()
            .filter(|p| p.id != player)
            .map(|p| p.id)
            .collect();
        if others.len() == 1 {
            Some(others[0])
        } else {
            None
        }
    }

    /// Append a game-log line.
    pub fn log(&mut self, text: impl Into<String>) {
        self.events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::Log(rv1::LogMessage {
                text: text.into(),
                visible_to_player_id: None,
                hidden_from_player_id: None,
            })),
        });
    }
}

/// Whether a move to `zone` may be announced with the fully-public `PermanentMoved` event, and
/// with which destination.
///
/// `PermanentMoved` carries `card_id` at `FIELD_VISIBILITY_PUBLIC` (`ruled_v1.proto`), so it may
/// only name a destination whose contents are public *by identity*. Hand and library are hidden
/// zones (CR 400.2): their contents reach each player through the redacted per-player zone view,
/// and announcing them here would leak — [`ResolutionCtx::put_on_top_of_library`] is the exact
/// case. The stack is announced by the stack events, not this one.
///
/// Exhaustive on purpose: a new [`Zone`] variant must make this decision rather than inherit
/// silence from a `_` arm.
fn public_move_event_destination(zone: Zone) -> Option<rv1::permanent_moved::Destination> {
    use rv1::permanent_moved::Destination;
    match zone {
        Zone::Graveyard => Some(Destination::Graveyard),
        Zone::Exile => Some(Destination::Exile),
        // No custom effect reaches the battlefield today, but this is the destination the engine's
        // own reanimation path already emits, so "public zone ⇒ event" holds without an exception.
        Zone::Battlefield => Some(Destination::Battlefield),
        Zone::Hand | Zone::Library | Zone::Stack => None,
    }
}

/// The registration table as a map, built once. Keys cannot collide: they are file stems, and
/// `build.rs` fails the build on a duplicate.
fn by_key() -> &'static HashMap<&'static str, &'static dyn CardEffect> {
    static BY_KEY: OnceLock<HashMap<&'static str, &'static dyn CardEffect>> = OnceLock::new();
    BY_KEY.get_or_init(|| EFFECT_IMPLS.iter().copied().collect())
}

/// Resolve a card's `custom_effect` key to its [`CardEffect`] implementation. Returns `None`
/// for an unregistered key; the engine treats that as illegal card data (a test asserts every
/// `custom_effect` in the registry resolves here, and [`keys`] the converse).
pub fn lookup(key: &str) -> Option<&'static dyn CardEffect> {
    by_key().get(key).copied()
}

/// Every registered card id. The reverse direction of [`lookup`]: it lets a test assert that no
/// impl is orphaned — a file stem no registry card claims is a typo'd filename or a deleted RON,
/// neither of which the forward check can see.
pub fn keys() -> impl Iterator<Item = &'static str> {
    EFFECT_IMPLS.iter().map(|(key, _)| *key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_registered_key_resolves_and_unknown_keys_do_not() {
        let mut count = 0;
        for key in keys() {
            assert!(
                lookup(key).is_some(),
                "registered key `{key}` does not resolve"
            );
            count += 1;
        }
        assert!(count > 0, "build.rs found no custom effect files");
        assert!(lookup("no_such_card").is_none());
    }

    /// The redaction rule asserted directly: a public event may never name a hidden zone
    /// (CR 400.2), because `PermanentMoved.card_id` is broadcast to every player.
    #[test]
    fn only_public_zones_get_a_move_event() {
        use rv1::permanent_moved::Destination;
        assert_eq!(
            public_move_event_destination(Zone::Graveyard),
            Some(Destination::Graveyard)
        );
        assert_eq!(
            public_move_event_destination(Zone::Exile),
            Some(Destination::Exile)
        );
        assert_eq!(
            public_move_event_destination(Zone::Battlefield),
            Some(Destination::Battlefield)
        );
        assert_eq!(public_move_event_destination(Zone::Hand), None);
        assert_eq!(public_move_event_destination(Zone::Library), None);
        assert_eq!(public_move_event_destination(Zone::Stack), None);
    }
}
