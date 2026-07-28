//! Debug-only cheat commands for the dev console (`RuledCommand.dev_command`).
//!
//! These exist so a board state can be reached without editing deck files and playing turns to
//! get there. They are cheats: none of them is a legal game action. What keeps them safe is the
//! gate, not their behaviour — [`GameEngine::apply_command`] refuses a `DevCommand` outright
//! unless the sidecar enabled dev commands for this session, and that check happens before
//! `command_index` advances, so a refused command never enters the replay log.
//!
//! Accepted dev commands *are* ordinary logged commands: they advance `command_index` and
//! Servatrice appends them to `ruled_command_log`, so `(seed, command log)` replay still
//! reproduces a dev-built board exactly.
//!
//! Everything here deliberately reuses the normal engine mutators rather than poking state
//! directly — [`move_object_to_zone`] for the CR 400.7 new-object reset, `fire_triggers` for
//! CR 603.6a entry — so a conjured permanent is indistinguishable from a legitimately cast one.

use super::events::{ev_log, finish_with_events};
use super::*;

/// What a placement verb hands to the tail the two of them share. A struct rather than seven
/// positional parameters, mirroring `EffectCx` in `engine::resolution`.
struct Placement<'a> {
    target: PlayerId,
    oid: ObjectId,
    zone: Zone,
    ready: bool,
    /// Oracle name, for the game log.
    name: &'a str,
    /// How the log should describe what happened ("conjures" / "moves").
    verb: &'a str,
}

impl GameEngine {
    /// Dispatch one dev command. Gated in [`GameEngine::apply_command`]; by the time we get here
    /// the session is known to allow dev commands.
    pub(super) fn apply_dev_command(
        &mut self,
        dc: &rv1::DevCommand,
    ) -> Result<RuledEventBatch, EngineError> {
        let target = dc.target_player_id;
        if self.state.player_idx(target).is_none() {
            return Err(EngineError::UnknownPlayer(target));
        }
        let mut ev = vec![];
        match dc.dev.as_ref() {
            None => return Err(EngineError::Illegal("empty dev command")),
            Some(rv1::dev_command::Dev::PutCardInZone(p)) => {
                self.dev_put_card_in_zone(target, p, &mut ev)?
            }
            Some(rv1::dev_command::Dev::MoveCard(m)) => self.dev_move_card(target, m, &mut ev)?,
            Some(rv1::dev_command::Dev::AddMana(m)) => self.dev_add_mana(target, m, &mut ev)?,
        }
        Ok(finish_with_events(self, ev))
    }

    /// Conjure a named card from outside the game into a zone.
    ///
    /// Always mints a new object, even when the player already owns a copy: assembling a board
    /// means asking for two Serra Angels and getting two. Relocating something that already
    /// exists is [`Self::dev_move_card`]'s job — overloading one verb with both made it
    /// impossible to express the first, and surprising when it silently did the second.
    fn dev_put_card_in_zone(
        &mut self,
        target: PlayerId,
        put: &rv1::DevPutCardInZone,
        ev: &mut Vec<RuledEvent>,
    ) -> Result<(), EngineError> {
        let name = put.card_name.trim().to_string();
        let card_id = self.resolve_card_id(&name)?;
        let zone = dev_zone_to_zone(put.zone());
        let oid = self.conjure_card(target, &card_id, zone, ev)?;
        self.finish_dev_placement(
            Placement {
                target,
                oid,
                zone,
                ready: put.ready,
                name: &name,
                verb: "conjures",
            },
            ev,
        );
        Ok(())
    }

    /// Relocate a card the player already owns. Creates nothing, and unlike conjuring it can reach
    /// the graveyard, exile and library.
    fn dev_move_card(
        &mut self,
        target: PlayerId,
        mv: &rv1::DevMoveCard,
        ev: &mut Vec<RuledEvent>,
    ) -> Result<(), EngineError> {
        let name = mv.card_name.trim().to_string();
        let card_id = self.resolve_card_id(&name)?;
        let zone = dev_zone_to_zone(mv.zone());
        // Falling back to a conjure here would collapse the two verbs back into one.
        let oid = self
            .find_owned_object(target, &card_id)
            .ok_or(EngineError::Illegal(
                "no copy of that card in any of your zones — use put to conjure one",
            ))?;
        move_object_to_zone(&mut self.state, oid, zone)?;
        ev.push(permanent_moved_event(
            &self.state,
            oid,
            target,
            zone_to_destination(zone),
        ));
        self.finish_dev_placement(
            Placement {
                target,
                oid,
                zone,
                ready: mv.ready,
                name: &name,
                verb: "moves",
            },
            ev,
        );
        Ok(())
    }

    /// The tail both placement verbs share: apply `ready`, log, and announce a battlefield entry.
    fn finish_dev_placement(&mut self, p: Placement<'_>, ev: &mut Vec<RuledEvent>) {
        let Placement {
            target,
            oid,
            zone,
            ready,
            name,
            verb,
        } = p;
        // CR 302.6 says a permanent that has not been controlled since its controller's turn began
        // is summoning sick, and both placement paths assert exactly that on battlefield entry.
        // Clearing it must therefore come *after* the placement, or it is immediately overwritten.
        // Deliberately not implemented as "grant haste": haste is a real keyword with layer-6
        // semantics a test may be trying to observe, so the cheat must not fake it.
        let readied = ready && zone == Zone::Battlefield;
        if readied {
            if let Some(o) = self.state.objects.get_mut(&oid) {
                o.summoning_sick = false;
            }
        }

        let suffix = if readied { ", ready" } else { "" };
        ev.push(ev_log(format!(
            "[dev] P{target} {verb} {name} into {}{suffix}.",
            zone_label(zone)
        )));

        // CR 603.6a: entering the battlefield fires ETB triggers however the permanent arrived.
        // This call is also the only path to `emit_static_abilities_on_enter`, so skipping it
        // would leave a conjured anthem on the battlefield granting nothing at all — silently.
        // No SpellCast event is fired: nothing was cast (CR 601), so cast triggers correctly stay
        // silent, matching every real put-onto-the-battlefield effect.
        if zone == Zone::Battlefield {
            self.fire_triggers(GameEvent::EntersBattlefield { object_id: oid }, ev);
        }
    }

    /// Oracle name -> engine card id. The name is engine-owned identity, never a client slug.
    fn resolve_card_id(&self, name: &str) -> Result<String, EngineError> {
        self.registry
            .id_for_name(name)
            .map(|id| id.to_string())
            .ok_or_else(|| EngineError::MissingCard(name.to_string()))
    }

    /// Mint a brand-new object for `card_id` from outside the game (CR 400.11-shaped, though it
    /// obeys none of the restrictions real wishes do).
    ///
    /// Restricted to hand and battlefield because those are the two zones Servatrice can mint a
    /// backing `Server_Card` into; graveyard, exile and library keep separate physical binding
    /// maps and are reachable in two steps (conjure to hand, then move).
    fn conjure_card(
        &mut self,
        target: PlayerId,
        card_id: &str,
        zone: Zone,
        ev: &mut Vec<RuledEvent>,
    ) -> Result<ObjectId, EngineError> {
        if !matches!(zone, Zone::Hand | Zone::Battlefield) {
            return Err(EngineError::Illegal(
                "conjuring supports hand and battlefield only (move it on from there)",
            ));
        }
        let def = self
            .registry
            .get(card_id)
            .ok_or_else(|| EngineError::MissingCard(card_id.to_string()))?;
        let face = def.primary_face();
        let is_creature = face.is_creature;
        let display_name = def.name.clone();

        let oid = self.state.next_object_id;
        self.state.next_object_id += 1;
        self.state
            .objects
            .insert(oid, new_object_from_card(oid, target, card_id, zone, face));
        let idx = self
            .state
            .player_idx(target)
            .ok_or(EngineError::UnknownPlayer(target))?;
        match zone {
            Zone::Hand => self.state.players[idx].hand.push(oid),
            Zone::Battlefield => self.state.players[idx].battlefield.push(oid),
            _ => unreachable!("conjure zone restricted above"),
        }

        // Servatrice resolves physical cards by translating their names through the session card
        // catalog, which is built once from the decklists — a conjured card is absent from it, so
        // the zone reconcile would find no match and abandon the whole sync with only a warning.
        // Re-emitting the catalog (now including this object) repairs that. Server-only, so no
        // client sees it.
        ev.push(self.ev_card_catalog());
        // And this is what tells Servatrice to create the backing Server_Card at all.
        ev.push(RuledEvent {
            ev: Some(rv1::ruled_event::Ev::DevCardConjured(
                rv1::DevCardConjured {
                    object_id: oid,
                    owner_player_id: target,
                    card_name: display_name,
                    zone: zone_to_dev_zone(zone) as i32,
                    is_creature,
                },
            )),
        });
        Ok(oid)
    }

    /// Add mana to a player's pool.
    ///
    /// Nothing is emitted here: the `dispatch_command` epilogue already pushes a
    /// `ManaPoolUpdated` for every player after every command, and that snapshot is absolute.
    /// The mana empties at the next step/phase change like any other (CR 106.4).
    fn dev_add_mana(
        &mut self,
        target: PlayerId,
        add: &rv1::DevAddMana,
        ev: &mut Vec<RuledEvent>,
    ) -> Result<(), EngineError> {
        let idx = self
            .state
            .player_idx(target)
            .ok_or(EngineError::UnknownPlayer(target))?;
        let pool = &mut self.state.players[idx].mana_pool;
        pool.white = pool.white.saturating_add(add.w);
        pool.blue = pool.blue.saturating_add(add.u);
        pool.black = pool.black.saturating_add(add.b);
        pool.red = pool.red.saturating_add(add.r);
        pool.green = pool.green.saturating_add(add.g);
        pool.colorless = pool.colorless.saturating_add(add.c);
        ev.push(ev_log(format!(
            "[dev] P{target} adds mana ({}).",
            mana_label(add)
        )));
        Ok(())
    }

    /// First object owned by `player` with this `card_id`, searching library, graveyard, exile,
    /// hand, then battlefield. Library first so the command prefers an unused copy over
    /// cannibalising the board the caller is in the middle of setting up.
    fn find_owned_object(&self, player: PlayerId, card_id: &str) -> Option<ObjectId> {
        let idx = self.state.player_idx(player)?;
        let p = &self.state.players[idx];
        let matches = |oid: &ObjectId| {
            self.state
                .objects
                .get(oid)
                .is_some_and(|o| o.card_id == card_id)
        };
        p.library
            .iter()
            .find(|o| matches(o))
            .or_else(|| p.graveyard.iter().find(|o| matches(o)))
            .or_else(|| p.exile.iter().find(|o| matches(o)))
            .or_else(|| p.hand.iter().find(|o| matches(o)))
            .or_else(|| p.battlefield.iter().find(|o| matches(o)))
            .copied()
    }
}

fn dev_zone_to_zone(z: rv1::DevZone) -> Zone {
    match z {
        rv1::DevZone::Hand => Zone::Hand,
        rv1::DevZone::Battlefield => Zone::Battlefield,
        rv1::DevZone::Graveyard => Zone::Graveyard,
        rv1::DevZone::Exile => Zone::Exile,
        rv1::DevZone::Library => Zone::Library,
    }
}

fn zone_to_dev_zone(z: Zone) -> rv1::DevZone {
    match z {
        Zone::Hand => rv1::DevZone::Hand,
        Zone::Battlefield => rv1::DevZone::Battlefield,
        Zone::Graveyard => rv1::DevZone::Graveyard,
        Zone::Exile => rv1::DevZone::Exile,
        // The stack is not a dev destination; report it as the closest thing rather than panic.
        Zone::Library | Zone::Stack => rv1::DevZone::Library,
    }
}

fn zone_to_destination(z: Zone) -> rv1::permanent_moved::Destination {
    use rv1::permanent_moved::Destination;
    match z {
        Zone::Hand => Destination::Hand,
        Zone::Battlefield => Destination::Battlefield,
        Zone::Graveyard => Destination::Graveyard,
        Zone::Exile => Destination::Exile,
        Zone::Library => Destination::Library,
        Zone::Stack => Destination::Unspecified,
    }
}

fn zone_label(z: Zone) -> &'static str {
    match z {
        Zone::Hand => "hand",
        Zone::Battlefield => "the battlefield",
        Zone::Graveyard => "the graveyard",
        Zone::Exile => "exile",
        Zone::Library => "the library",
        Zone::Stack => "the stack",
    }
}

/// Render an added-mana payload the way a mana cost reads (`{2}{R}{R}`), for the game log.
fn mana_label(add: &rv1::DevAddMana) -> String {
    let mut out = String::new();
    if add.c > 0 {
        out.push_str(&format!("{{{}}}", add.c));
    }
    for (count, sym) in [
        (add.w, 'W'),
        (add.u, 'U'),
        (add.b, 'B'),
        (add.r, 'R'),
        (add.g, 'G'),
    ] {
        for _ in 0..count {
            out.push_str(&format!("{{{sym}}}"));
        }
    }
    if out.is_empty() {
        out.push_str("nothing");
    }
    out
}
