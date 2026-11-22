//! Read-scope memo for derived per-object queries.
//!
//! Characteristics, static-source availability and active static abilities are pure functions
//! of `GameState`, the registry and the queried object. Per-permanent consumers (legal actions,
//! payment witnesses, trigger collection) each re-ask them for every permanent, so an unmemoized
//! command costs O(board²). A [`ReadMemoScope`] holds a shared borrow of the engine while it
//! lives, so the state cannot change under it; the answers are cached for exactly that long and
//! dropped when the outermost scope ends. Engine clones (payment-search probes) start with an
//! empty, inactive memo of their own.

use super::*;
use std::cell::{Cell, RefCell};
use std::sync::Arc;

/// A battlefield object whose printed/copied static abilities currently function.
pub(super) struct StaticAbilitySource {
    /// The object's layer-2 `controller` field, as the scans this replaces read it.
    pub controller: PlayerId,
    pub abilities: Vec<StaticAbilityDef>,
}

/// Board-wide facts behind every "does tapping this source only add mana?" proof. Each is a
/// scan of the whole battlefield, so it is computed once per scope instead of once per source.
#[derive(Clone, Copy)]
pub(super) struct TapManaBoard {
    /// No effect, emblem or static ability can change what a mana activation produces.
    pub stable: bool,
    /// Some battlefield trigger has an intervening-if clause, which can read tapped status.
    pub intervening_if: bool,
    /// Some battlefield trigger observes a permanent becoming tapped.
    pub tap_observers: bool,
}

#[derive(Default)]
pub(super) struct ReadMemo {
    depth: Cell<u32>,
    characteristics: RefCell<HashMap<ObjectId, Option<Characteristics>>>,
    static_source_available: RefCell<HashMap<ObjectId, bool>>,
    active_static_abilities:
        RefCell<HashMap<ObjectId, Vec<tricerules_card_model::IdentifiedStaticAbility>>>,
    tap_mana_only: RefCell<HashMap<(PlayerId, ObjectId), Option<bool>>>,
    tap_mana_board: Cell<Option<TapManaBoard>>,
    battlefield_static_sources: RefCell<Option<Arc<Vec<StaticAbilitySource>>>>,
}

impl Clone for ReadMemo {
    /// A clone is about to diverge from this engine's state, so it never inherits answers.
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl ReadMemo {
    fn cached<K: std::hash::Hash + Eq, T: Clone>(
        &self,
        table: &RefCell<HashMap<K, T>>,
        key: K,
        compute: impl FnOnce() -> T,
    ) -> T {
        if self.depth.get() == 0 {
            return compute();
        }
        if let Some(hit) = table.borrow().get(&key) {
            return hit.clone();
        }
        // Computing may re-enter the memo for other objects; no borrow is held across it.
        let value = compute();
        table.borrow_mut().insert(key, value.clone());
        value
    }

    fn clear(&self) {
        self.characteristics.borrow_mut().clear();
        self.static_source_available.borrow_mut().clear();
        self.active_static_abilities.borrow_mut().clear();
        self.tap_mana_only.borrow_mut().clear();
        self.tap_mana_board.take();
        self.battlefield_static_sources.borrow_mut().take();
    }
}

/// While alive, derived queries on this engine are memoized. The borrow it holds is what makes
/// the cache sound: no `&mut GameEngine` (and so no state change) can coexist with it.
pub(super) struct ReadMemoScope<'a> {
    engine: &'a GameEngine,
}

impl Drop for ReadMemoScope<'_> {
    fn drop(&mut self) {
        let memo = &self.engine.read_memo;
        memo.depth.set(memo.depth.get() - 1);
        if memo.depth.get() == 0 {
            memo.clear();
        }
    }
}

impl GameEngine {
    pub(super) fn read_memo_scope(&self) -> ReadMemoScope<'_> {
        self.read_memo.depth.set(self.read_memo.depth.get() + 1);
        ReadMemoScope { engine: self }
    }

    pub(super) fn memoized_characteristics(
        &self,
        oid: ObjectId,
        compute: impl FnOnce() -> Option<Characteristics>,
    ) -> Option<Characteristics> {
        self.read_memo
            .cached(&self.read_memo.characteristics, oid, compute)
    }

    /// [`super::characteristics::printed_static_source_is_available`] for this engine's state.
    pub(super) fn printed_static_source_available(&self, oid: ObjectId) -> bool {
        self.read_memo
            .cached(&self.read_memo.static_source_available, oid, || {
                super::characteristics::printed_static_source_is_available(
                    &self.state,
                    self.registry,
                    oid,
                )
            })
    }

    pub(super) fn memoized_active_static_abilities(
        &self,
        oid: ObjectId,
        compute: impl FnOnce() -> Vec<tricerules_card_model::IdentifiedStaticAbility>,
    ) -> Vec<tricerules_card_model::IdentifiedStaticAbility> {
        self.read_memo
            .cached(&self.read_memo.active_static_abilities, oid, compute)
    }

    /// Every battlefield object with functioning static abilities, in object-id order. Board-wide
    /// static scans walk this instead of asking each permanent, and lands or vanilla creatures
    /// (most of a long-game board) drop out once per scope rather than once per query.
    pub(super) fn battlefield_static_sources(&self) -> Arc<Vec<StaticAbilitySource>> {
        if self.read_memo.depth.get() > 0 {
            if let Some(sources) = self.read_memo.battlefield_static_sources.borrow().as_ref() {
                return Arc::clone(sources);
            }
        }
        let mut objects = self
            .state
            .objects
            .values()
            .filter(|object| object.zone == Zone::Battlefield)
            .map(|object| (object.id, object.controller))
            .collect::<Vec<_>>();
        objects.sort_unstable();
        let sources = Arc::new(
            objects
                .into_iter()
                .filter_map(|(oid, controller)| {
                    let abilities = self.active_static_ability_definitions(oid);
                    (!abilities.is_empty()).then_some(StaticAbilitySource {
                        controller,
                        abilities,
                    })
                })
                .collect::<Vec<_>>(),
        );
        if self.read_memo.depth.get() > 0 {
            *self.read_memo.battlefield_static_sources.borrow_mut() = Some(Arc::clone(&sources));
        }
        sources
    }

    /// [`TapManaBoard`] for this engine's state.
    pub(super) fn memoized_tap_mana_board(
        &self,
        compute: impl FnOnce() -> TapManaBoard,
    ) -> TapManaBoard {
        let memo = &self.read_memo;
        if memo.depth.get() == 0 {
            return compute();
        }
        if let Some(board) = memo.tap_mana_board.get() {
            return board;
        }
        let board = compute();
        memo.tap_mana_board.set(Some(board));
        board
    }

    /// Whether tapping `oid` for ordinary mana does nothing but add mana, for `player`'s
    /// payment proofs. See `action_availability::proofs::mana_only`.
    pub(super) fn memoized_tap_mana_only(
        &self,
        player: PlayerId,
        oid: ObjectId,
        compute: impl FnOnce() -> Option<bool>,
    ) -> Option<bool> {
        self.read_memo
            .cached(&self.read_memo.tap_mana_only, (player, oid), compute)
    }
}

#[cfg(test)]
thread_local! {
    static EVALUATIONS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// Record one full evaluation of a derived per-object query (characteristics, static-source
/// availability, or active static abilities). Compiled out of non-test builds.
#[inline]
pub(super) fn count_evaluation() {
    #[cfg(test)]
    EVALUATIONS.with(|count| count.set(count.get() + 1));
}

/// Full derived-query evaluations performed on this thread so far.
#[cfg(test)]
pub(super) fn evaluations() -> u64 {
    EVALUATIONS.with(std::cell::Cell::get)
}

/// Engine-cost probes the board-scaling tests count. Compiled out of non-test builds.
#[derive(Clone, Copy)]
pub(super) enum Probe {
    /// An engine clone made to test whether tapping one source causes a trigger.
    TapCheckClone,
    /// An engine clone made by the bounded mana-activation search.
    PaymentSearchClone,
    /// A full trigger-source snapshot of one battlefield permanent.
    TriggerSourceSnapshot,
}

#[cfg(test)]
thread_local! {
    static PROBES: std::cell::Cell<[u64; 3]> = const { std::cell::Cell::new([0; 3]) };
}

#[inline]
pub(super) fn count_probe(_probe: Probe) {
    #[cfg(test)]
    PROBES.with(|counts| {
        let mut value = counts.get();
        value[_probe as usize] += 1;
        counts.set(value);
    });
}

/// Probes of this kind performed on this thread so far.
#[cfg(test)]
pub(super) fn probes(probe: Probe) -> u64 {
    PROBES.with(|counts| counts.get()[probe as usize])
}
