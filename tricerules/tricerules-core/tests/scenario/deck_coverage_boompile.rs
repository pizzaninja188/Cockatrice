//! Exact Boompile identity and coin-flip destruction behavior.

use super::helpers::*;
use rand::{Rng, SeedableRng};
use tricerules_cards::primitives::{AbilityCost, CounterKind, SpellEffectKind};
use tricerules_cards::ManaCost;
use tricerules_core::{GameEngine, Zone};

const BOOMPILE: &str = "boompile";
const BOOMPILE_RANDOM_DOMAIN: u64 = 0x424F_4F4D_5049_4C45;

fn engine(seed: u64) -> GameEngine {
    let deck = deck_with("forest", &[]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        Some(vec![deck.clone(), deck]),
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

#[test]
fn boompile_has_a_complete_card_definition() {
    let card = tricerules_cards::registry::global()
        .get(BOOMPILE)
        .expect("Boompile needs a complete definition");
    assert_eq!(card.face_count(), 1);
    let face = card.primary_face();
    assert_eq!(face.mana_cost, ManaCost::parse("{4}").unwrap());
    assert_eq!(face.types, ["Artifact"]);
    assert_eq!(face.activated_abilities.len(), 1);
    let ability = &face.activated_abilities[0];
    assert_eq!(ability.costs, [AbilityCost::Tap]);
    assert_eq!(
        ability.effect,
        [SpellEffectKind::BoompileFlipCoinAndDestroyNonlands]
    );
}

fn expected_coin_win(seed: u64, command_index: u64, controller: i32, source: u32) -> bool {
    let mix = seed
        ^ command_index.wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ u64::from(source).wrapping_mul(0xD1B5_4A32_D192_ED03)
        ^ (controller as u64).rotate_left(32)
        ^ BOOMPILE_RANDOM_DOMAIN;
    rand::rngs::StdRng::seed_from_u64(mix).gen::<bool>()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolutionResult {
    expected_win: bool,
    flip_logs: Vec<String>,
    zones: Vec<Zone>,
    source_tapped: bool,
    command_index: u64,
}

fn resolve_boompile(seed: u64) -> ResolutionResult {
    let mut engine = engine(seed);
    let source = inject_permanent_on_battlefield(&mut engine, 0, BOOMPILE);
    let artifact = inject_permanent_on_battlefield(&mut engine, 0, "bonesplitter");
    let creature = inject_permanent_on_battlefield(&mut engine, 1, "grizzly_bears");
    let enchantment = inject_permanent_on_battlefield(&mut engine, 1, "bad_moon");
    let planeswalker = inject_permanent_on_battlefield(&mut engine, 1, "jace_beleren");
    engine
        .state
        .objects
        .get_mut(&planeswalker)
        .expect("Jace on battlefield")
        .set_counter(CounterKind::Loyalty, 3);
    let protected_creature = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    inject_card_into_hand(&mut engine, 0, "indestructibility");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 3,
            ..Default::default()
        },
    );
    let aura_slot = hand_index_for_card(&engine, 0, "indestructibility");
    engine
        .apply_command(0, &cast_spell(aura_slot, target_object(protected_creature)))
        .expect("cast Indestructibility on an opponent's creature");
    resolve_entire_stack_two_player(&mut engine);
    let aura = battlefield_object_for_card(&engine, 0, "indestructibility");
    assert!(engine.effective_has_keyword(
        protected_creature,
        tricerules_cards::Keyword::Indestructible
    ));
    let regenerating_creature = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    engine
        .state
        .objects
        .get_mut(&regenerating_creature)
        .unwrap()
        .regeneration_shields = 1;
    let land = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    let artifact_land = inject_permanent_on_battlefield(&mut engine, 1, "seat_of_the_synod");
    let indestructible = inject_permanent_on_battlefield(&mut engine, 1, "darksteel_myr");

    let activation = activate_ability_for(&engine, source, 0, vec![]);
    engine
        .apply_command(0, &activation)
        .expect("activate Boompile's tap ability");
    assert!(
        engine.state.objects[&source].tapped,
        "the activation pays its tap cost"
    );
    assert_eq!(engine.state.stack.len(), 1);

    let first_passer = engine.state.priority_player_id();
    engine
        .apply_command(first_passer, &pass())
        .expect("first player passes");
    let resolving_command_index = engine.state.command_index;
    let expected_win = expected_coin_win(seed, resolving_command_index, 0, source);
    let resolving_passer = engine.state.priority_player_id();
    let resolved = engine
        .apply_command(resolving_passer, &pass())
        .expect("second player passes and resolves Boompile");

    let flip_logs = resolved
        .events
        .iter()
        .filter_map(|event| match event.ev.as_ref() {
            Some(Ev::Log(log)) if log.text.contains("Boompile coin flip") => Some(log.text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        flip_logs,
        [format!(
            "P0 {} the Boompile coin flip.",
            if expected_win { "wins" } else { "loses" }
        )],
        "the result is public and independently predicted from the replay seed"
    );
    assert!(engine.state.stack.is_empty());

    let nonlands = [source, artifact, creature, enchantment, planeswalker];
    for object in nonlands {
        assert_eq!(
            engine.state.objects[&object].zone,
            if expected_win {
                Zone::Graveyard
            } else {
                Zone::Battlefield
            },
            "a win destroys every nonland permanent, including Boompile; a loss destroys none"
        );
    }
    assert_eq!(
        engine.state.objects[&land].zone,
        Zone::Battlefield,
        "a basic land survives"
    );
    assert_eq!(
        engine.state.objects[&artifact_land].zone,
        Zone::Battlefield,
        "an artifact land is still a land"
    );
    assert_eq!(
        engine.state.objects[&indestructible].zone,
        Zone::Battlefield,
        "an indestructible nonland survives the sweep"
    );
    assert_eq!(
        engine.state.objects[&aura].zone,
        if expected_win {
            Zone::Graveyard
        } else {
            Zone::Battlefield
        },
        "the Aura is itself a nonland permanent"
    );
    assert_eq!(
        engine.state.objects[&protected_creature].zone,
        Zone::Battlefield,
        "the simultaneous sweep snapshots indestructibility before its Aura leaves"
    );
    assert_eq!(
        engine.state.objects[&regenerating_creature].zone,
        Zone::Battlefield,
        "DestroyAll allows the shield to regenerate"
    );
    assert_eq!(
        engine.state.objects[&regenerating_creature].tapped, expected_win,
        "a used regeneration shield taps its creature"
    );

    ResolutionResult {
        expected_win,
        flip_logs,
        zones: [
            source,
            artifact,
            creature,
            enchantment,
            planeswalker,
            aura,
            protected_creature,
            regenerating_creature,
            land,
            artifact_land,
            indestructible,
        ]
        .into_iter()
        .map(|object| engine.state.objects[&object].zone)
        .collect(),
        source_tapped: engine.state.objects[&source].tapped,
        command_index: engine.state.command_index,
    }
}

#[test]
fn coin_result_is_fair_replay_stable_and_sweeps_only_nonlands_on_a_win() {
    let mut saw_loss = false;
    let mut saw_win = false;
    for seed in 1..=64 {
        let result = resolve_boompile(seed);
        saw_win |= result.expected_win;
        saw_loss |= !result.expected_win;
        if saw_win && saw_loss {
            break;
        }
    }
    assert!(saw_win, "fixed seeds include a winning fair coin result");
    assert!(saw_loss, "fixed seeds include a losing fair coin result");

    let first = resolve_boompile(0x00B0_0B1E);
    let replay = resolve_boompile(0x00B0_0B1E);
    assert_eq!(
        first, replay,
        "same seed and accepted commands replay exactly"
    );
}

#[test]
fn tapped_boompile_rejects_activation_without_advancing_the_replay_index() {
    let mut engine = engine(0x00B0_0B1F);
    let source = inject_permanent_on_battlefield(&mut engine, 0, BOOMPILE);
    engine.state.objects.get_mut(&source).unwrap().tapped = true;
    let command_index = engine.state.command_index;
    assert!(engine
        .apply_command(0, &activate_ability_for(&engine, source, 0, vec![]))
        .is_err());
    assert_eq!(engine.state.command_index, command_index);
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.objects[&source].tapped);
}

#[test]
fn boompile_ability_resolves_after_its_source_leaves_the_battlefield() {
    let mut saw_win = false;
    for seed in 1..=64 {
        let mut engine = engine(seed);
        let source = inject_permanent_on_battlefield(&mut engine, 0, BOOMPILE);
        let other_artifact = inject_permanent_on_battlefield(&mut engine, 1, "bonesplitter");
        engine
            .apply_command(0, &activate_ability_for(&engine, source, 0, vec![]))
            .expect("activate Boompile");
        engine.apply_command(0, &pass()).expect("controller passes");

        inject_card_into_hand(&mut engine, 1, "disenchant");
        give_mana(
            &mut engine,
            1,
            ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
        );
        let disenchant_slot = hand_index_for_card(&engine, 1, "disenchant");
        engine
            .apply_command(1, &cast_spell(disenchant_slot, target_object(source)))
            .expect("destroy Boompile while its ability remains on the stack");
        pass_both_players(&mut engine);
        assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
        assert_eq!(engine.state.stack.len(), 1, "the activated ability remains");

        let first_passer = engine.state.priority_player_id();
        engine
            .apply_command(first_passer, &pass())
            .expect("first player passes before Boompile resolves");
        let resolving_command_index = engine.state.command_index;
        let expected_win = expected_coin_win(seed, resolving_command_index, 0, source);
        let second_passer = engine.state.priority_player_id();
        let resolved = engine
            .apply_command(second_passer, &pass())
            .expect("resolve the source-independent ability");
        assert!(resolved.events.iter().any(|event| matches!(
            event.ev.as_ref(),
            Some(Ev::Log(log)) if log.text == format!(
                "P0 {} the Boompile coin flip.",
                if expected_win { "wins" } else { "loses" }
            )
        )));
        assert_eq!(
            engine.state.objects[&other_artifact].zone,
            if expected_win {
                Zone::Graveyard
            } else {
                Zone::Battlefield
            },
            "a winning ability still sweeps after its source leaves"
        );
        if expected_win {
            saw_win = true;
            break;
        }
    }
    assert!(saw_win, "the fixed seed set includes a winning coin result");
}
