//! Scripted command sequences (M2), split into themed submodules.
//!
//! `tests/scenario.rs` is the crate root of the `scenario` integration-test binary, so a bare
//! `mod foo;` would resolve to `tests/foo.rs` (which Cargo would then compile as its own test
//! binary). The `#[path]` attributes keep every submodule under `tests/scenario/` while
//! preserving a single `scenario` test binary.

#[path = "scenario/casting_and_lands.rs"]
mod casting_and_lands;
#[path = "scenario/combat.rs"]
mod combat;
#[path = "scenario/combat_keywords.rs"]
mod combat_keywords;
#[path = "scenario/counters_and_pump.rs"]
mod counters_and_pump;
#[path = "scenario/custom_resolution.rs"]
mod custom_resolution;
#[path = "scenario/helpers.rs"]
mod helpers;
#[path = "scenario/mana.rs"]
mod mana;
#[path = "scenario/multi_face.rs"]
mod multi_face;
#[path = "scenario/opening.rs"]
mod opening;
#[path = "scenario/priority_and_turns.rs"]
mod priority_and_turns;
#[path = "scenario/spell_effects.rs"]
mod spell_effects;
#[path = "scenario/stack_and_counterspells.rs"]
mod stack_and_counterspells;
#[path = "scenario/targeting.rs"]
mod targeting;
#[path = "scenario/tokens.rs"]
mod tokens;
#[path = "scenario/triggers.rs"]
mod triggers;

mod llanowar_elves_tests {
    use super::helpers::*;

    /// CR 605.1a: Llanowar Elves' `{T}: Add {G}` is a mana ability — resolves immediately, no
    /// stack, no priority change. Tapping a non-summoning-sick elf produces {G} in the pool.
    #[test]
    fn llanowar_elves_taps_for_green() {
        let mut e = GameEngine::new(92000, &[0, 1], 20, None, true).expect("new");
        advance_to_main1_from_game_start(&mut e);
        let elf = inject_permanent_on_battlefield(&mut e, 0, "llanowar_elves");
        e.apply_command(0, &activate_ability(elf, 0, vec![]))
            .expect("tap Llanowar Elves for {G}");
        assert_eq!(e.state.players[0].mana_pool.green, 1, "produced {{G}}");
        assert!(e.state.objects[&elf].tapped, "elf tapped as cost");
        assert!(
            e.state.stack.is_empty(),
            "mana ability never uses the stack"
        );
    }

    /// CR 602.5 / 302.6: Llanowar Elves cannot activate its tap ability a second time — its
    /// source is already tapped; the engine must reject the second activation with no extra mana.
    #[test]
    fn llanowar_elves_cannot_tap_twice() {
        let mut e = GameEngine::new(92001, &[0, 1], 20, None, true).expect("new");
        advance_to_main1_from_game_start(&mut e);
        let elf = inject_permanent_on_battlefield(&mut e, 0, "llanowar_elves");
        e.apply_command(0, &activate_ability(elf, 0, vec![]))
            .expect("first tap");
        assert_eq!(e.state.players[0].mana_pool.green, 1);
        let err = e
            .apply_command(0, &activate_ability(elf, 0, vec![]))
            .expect_err("already-tapped Llanowar Elves cannot tap again");
        assert!(
            format!("{err:?}").contains("already tapped"),
            "unexpected error: {err:?}"
        );
        assert_eq!(e.state.players[0].mana_pool.green, 1, "no extra mana");
    }
}
