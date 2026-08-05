//! Brainstorm (CR 601/120 — draw, then a mid-resolution choice over the resulting hand).
//!
//! Oracle: "Draw three cards, then put two cards from your hand on top of your library in any
//! order." The "put back" is a choice over *live objects* (the just-drawn-into hand), which no
//! `(effect_kind, parameters)` primitive can express — this is the canonical tier-3 case.
//!
//! Algorithm: `begin` draws three, then asks the controller to choose two hand cards in order;
//! `resume` puts them on top of the library (last chosen = top, matching "place one then the
//! other on top" intuition). When the hand holds fewer than two cards after drawing (a short
//! library), it puts back as many as it can (CR 120: "as many as you can" when an exact count
//! is impossible).

use super::{
    CardEffect, ChoiceKind, ResolutionChoice, ResolutionCtx, ResolutionInterrupt, ResolutionStep,
};

pub struct Brainstorm;

/// Registration handle. `build.rs` wires this into the effect table by file stem (= card id);
/// the name `EFFECT` is the convention every file under `src/custom/` follows.
pub(crate) static EFFECT: &dyn CardEffect = &Brainstorm;

impl CardEffect for Brainstorm {
    fn begin(&self, ctx: &mut ResolutionCtx) -> ResolutionStep {
        let controller = ctx.controller;
        ctx.draw(controller, 3);
        ctx.log(format!("P{controller} draws 3 cards (Brainstorm)."));

        let hand = ctx.hand(controller);
        let count = hand.len().min(2) as u32;
        if count == 0 {
            return ResolutionStep::Done;
        }
        ResolutionStep::NeedsChoice(ResolutionInterrupt {
            deciding_player: controller,
            prompt: format!(
                "Brainstorm: choose {count} card{} to put back on top of your library (last chosen = top).",
                if count == 1 { "" } else { "s" }
            ),
            choice_kind: ChoiceKind::HandCards,
            candidates: hand,
            min: count,
            max: count,
            ordered: true,
            unique_names: false,
        })
    }

    fn resume(&self, ctx: &mut ResolutionCtx, choice: &ResolutionChoice) -> ResolutionStep {
        // The engine has already validated `choice.object_ids` against the interrupt
        // (subset of the hand, exactly `count`, distinct). Reverse so last-chosen ends on top:
        // click order 1 = bottom-most, click order N = top card (physical "place A, then B on
        // top of A" intuition).
        let mut ordered = choice.object_ids.clone();
        ordered.reverse();
        ctx.put_on_top_of_library(&ordered);
        let n = ordered.len();
        ctx.log(format!(
            "P{} puts {n} card{} on top of library (Brainstorm).",
            ctx.controller,
            if n == 1 { "" } else { "s" }
        ));
        ResolutionStep::Done
    }
}
