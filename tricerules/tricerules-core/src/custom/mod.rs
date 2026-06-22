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
//! ## Resumable resolution
//! [`CardEffect::begin`] starts resolving and either finishes ([`ResolutionStep::Done`]) or
//! returns a [`ResolutionInterrupt`] requesting input from a specific player. The engine parks
//! the in-flight spell (`GameState::pending_resolution`), emits the request, and on the logged
//! choice command calls [`CardEffect::resume`], looping until `Done`. Because every choice is a
//! logged command, replay (`(seed, command log) → state`) reconstructs the same resolution.

use crate::state::{GameObject, GameState, ObjectId, PlayerId, Zone};
use tricerules_cards::{CardDefinition, CardRegistry};
use tricerules_proto::ruled::v1 as rv1;

mod brainstorm;
mod gifts_ungiven;

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
/// crucially, for hidden-information redaction by the relay. `HandCards` and `LibrarySearch` are
/// *private* to the deciding player (their own concealed zone), so Servatrice strips the candidate
/// ids/names from every other player; `RevealedCards` are public (they were shown to the table).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ChoiceKind {
    /// Cards in the deciding player's hand (Brainstorm: pick 2 to put back). Private.
    HandCards,
    /// Cards revealed during resolution (Gifts Ungiven: opponent picks 2 of the revealed set). Public.
    RevealedCards,
    /// Cards the deciding player is searching out of their own library (Gifts Ungiven's search step,
    /// Demonic Tutor, …). Private — the library stays hidden; only the chosen cards become public.
    LibrarySearch,
}

impl ChoiceKind {
    /// Stable proto-wire discriminant (kept in sync with `RuledEvent.ResolutionChoiceRequired.choice_kind`
    /// and the relay's redaction in `Server_Game::sendRuledBatch`). 0 and 2 are private kinds.
    pub fn as_proto(self) -> i32 {
        match self {
            ChoiceKind::HandCards => 0,
            ChoiceKind::RevealedCards => 1,
            ChoiceKind::LibrarySearch => 2,
        }
    }
}

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
    pub fn put_on_top_of_library(&mut self, oids: &[ObjectId]) {
        // Push in reverse so oids[0] is the final front (top) of the library.
        for &oid in oids.iter().rev() {
            let Some(owner) = self.state.objects.get(&oid).map(|o| o.owner) else {
                continue;
            };
            let Some(idx) = self.state.player_idx(owner) else {
                continue;
            };
            let p = &mut self.state.players[idx];
            p.hand.retain(|&x| x != oid);
            p.library.retain(|&x| x != oid);
            p.battlefield.retain(|&x| x != oid);
            p.graveyard.retain(|&x| x != oid);
            p.exile.retain(|&x| x != oid);
            p.library.push_front(oid);
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
    /// `PermanentMoved` event for graveyard/exile destinations — the per-player zone-view sync
    /// carries hand/library/battlefield but not graveyard/exile, so the relay needs an explicit
    /// move there (the same event the mill/discard paths emit).
    pub fn move_to_zone(&mut self, oid: ObjectId, zone: Zone) {
        let Some(owner) = self.state.objects.get(&oid).map(|o| o.owner) else {
            return;
        };
        let Some(idx) = self.state.player_idx(owner) else {
            return;
        };
        let p = &mut self.state.players[idx];
        p.library.retain(|&x| x != oid);
        p.hand.retain(|&x| x != oid);
        p.battlefield.retain(|&x| x != oid);
        p.graveyard.retain(|&x| x != oid);
        p.exile.retain(|&x| x != oid);
        match zone {
            Zone::Graveyard => p.graveyard.push(oid),
            Zone::Hand => p.hand.push(oid),
            Zone::Battlefield => p.battlefield.push(oid),
            Zone::Library => p.library.push_back(oid),
            Zone::Exile => p.exile.push(oid),
            Zone::Stack => {}
        }
        if let Some(o) = self.state.objects.get_mut(&oid) {
            o.zone = zone;
        }
        let dest = match zone {
            Zone::Graveyard => Some(rv1::permanent_moved::Destination::Graveyard),
            Zone::Exile => Some(rv1::permanent_moved::Destination::Exile),
            _ => None,
        };
        if let Some(dest) = dest {
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
            })),
        });
    }
}

/// Resolve a card's `custom_effect` key to its [`CardEffect`] implementation. Returns `None`
/// for an unregistered key; the engine treats that as illegal card data (a startup test
/// asserts every `custom_effect` in the registry resolves here).
pub fn lookup(key: &str) -> Option<&'static dyn CardEffect> {
    match key {
        "brainstorm" => Some(&brainstorm::Brainstorm),
        "gifts_ungiven" => Some(&gifts_ungiven::GiftsUngiven),
        _ => None,
    }
}
