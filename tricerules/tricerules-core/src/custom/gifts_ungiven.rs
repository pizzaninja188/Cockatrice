//! Gifts Ungiven (CR 701.19 search + CR 608 — an *interdependent two-player* resolution).
//!
//! Oracle: "Search your library for up to four cards with different names and reveal them. Target
//! opponent chooses two of those cards. Put the chosen cards into your graveyard and the rest into
//! your hand. Then shuffle." The distinguishing tier-3 trait is that the *opponent* — not the
//! spell's controller — decides part of the resolution (which two of a set the controller
//! revealed). The generic [`ResolutionInterrupt::deciding_player`] carries that, so this needs no
//! card-specific proto: the same interrupt/choice machinery as Brainstorm, only the decider differs.
//!
//! Algorithm:
//!   `begin`        — ask the controller to choose up to four library cards (the search).
//!   `resume` (1)   — record them (revealed, still in the library), ask the *opponent* to choose two.
//!   `resume` (2)   — opponent's choices to the controller's graveyard, the rest to hand, then shuffle.
//!
//! Simplification (tracked via `partial`): in 1v1 the single opponent is the automatic target
//! (no separate target step). The "different names" restriction is enforced at search submission
//! via [`ResolutionInterrupt::unique_names`].

use super::{
    CardEffect, ChoiceKind, ResolutionChoice, ResolutionCtx, ResolutionInterrupt, ResolutionStep,
};
use crate::state::Zone;

pub struct GiftsUngiven;

impl CardEffect for GiftsUngiven {
    fn begin(&self, ctx: &mut ResolutionCtx) -> ResolutionStep {
        let controller = ctx.controller;
        let library = ctx.library(controller);
        if library.is_empty() {
            ctx.log(format!("P{controller} Gifts Ungiven: empty library."));
            return ResolutionStep::Done;
        }
        let max = library.len().min(4) as u32;
        ResolutionStep::NeedsChoice(ResolutionInterrupt {
            deciding_player: controller,
            prompt: format!(
                "Gifts Ungiven: search your library for up to {max} card(s) with different names."
            ),
            // Private: the searcher's library must not leak to the opponent. Only the cards the
            // controller chooses become public (revealed) in the next step.
            choice_kind: ChoiceKind::LibrarySearch,
            candidates: library,
            min: 0,
            max,
            ordered: false,
            // Enforce Oracle "different names" restriction at submission.
            unique_names: true,
        })
    }

    fn resume(&self, ctx: &mut ResolutionCtx, choice: &ResolutionChoice) -> ResolutionStep {
        match ctx.step {
            // Step 1: the controller has chosen the cards to reveal. They stay in the library
            // (revealed); the opponent now chooses which two go to the graveyard.
            1 => {
                let revealed = choice.object_ids.clone();
                if revealed.is_empty() {
                    ctx.log(format!("P{} Gifts Ungiven: found nothing.", ctx.controller));
                    shuffle(ctx);
                    return ResolutionStep::Done;
                }
                let names: Vec<String> = revealed.iter().map(|&o| ctx.card_name(o)).collect();
                ctx.log(format!(
                    "P{} reveals: {} (Gifts Ungiven).",
                    ctx.controller,
                    names.join(", ")
                ));
                ctx.scratch = revealed.clone();

                let Some(opponent) = ctx.opponent_of(ctx.controller) else {
                    // No single opponent: with no one to choose, all revealed go to hand.
                    distribute(ctx, &[], &revealed);
                    shuffle(ctx);
                    return ResolutionStep::Done;
                };
                let pick = revealed.len().min(2) as u32;
                ResolutionStep::NeedsChoice(ResolutionInterrupt {
                    deciding_player: opponent,
                    prompt: format!(
                        "Gifts Ungiven: choose {pick} of the revealed card(s) to put into \
                         P{}'s graveyard (the rest go to their hand).",
                        ctx.controller
                    ),
                    choice_kind: ChoiceKind::Revealed,
                    candidates: revealed,
                    min: pick,
                    max: pick,
                    ordered: false,
                    unique_names: false,
                })
            }
            // Step 2: the opponent has chosen which revealed cards go to the graveyard; the
            // remaining revealed cards go to the controller's hand. Then shuffle.
            _ => {
                let to_graveyard = choice.object_ids.clone();
                let to_hand: Vec<u32> = ctx
                    .scratch
                    .iter()
                    .copied()
                    .filter(|o| !to_graveyard.contains(o))
                    .collect();
                distribute(ctx, &to_graveyard, &to_hand);
                shuffle(ctx);
                ResolutionStep::Done
            }
        }
    }
}

/// Move the opponent-chosen cards to the controller's graveyard and the rest to their hand.
fn distribute(ctx: &mut ResolutionCtx, to_graveyard: &[u32], to_hand: &[u32]) {
    for &oid in to_graveyard {
        ctx.move_to_zone(oid, Zone::Graveyard);
    }
    for &oid in to_hand {
        ctx.move_to_zone(oid, Zone::Hand);
    }
    ctx.log(format!(
        "Gifts Ungiven: {} card(s) to graveyard, {} to hand (P{}).",
        to_graveyard.len(),
        to_hand.len(),
        ctx.controller
    ));
}

/// "Then shuffle" the controller's library (CR 701.19).
fn shuffle(ctx: &mut ResolutionCtx) {
    let controller = ctx.controller;
    ctx.shuffle_library(controller);
    ctx.log(format!("P{controller} shuffles (Gifts Ungiven)."));
}
