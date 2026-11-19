//! Actual-card scenarios for the queue-v42 self-regeneration creature cohort.
//!
//! Expectations are printed Oracle values. CR 701.19a-b define regeneration as a replacement
//! effect created by the activated ability; the shield taps the creature, removes it from
//! combat and clears lethal damage when it is consumed. The printed keyword static is checked
//! in derived characteristics.
use super::helpers::*;
use tricerules_cards::primitives::Keyword;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    regenerate: ManaGift,
    partial: ManaGift,
    body: (u32, u32),
    keyword: Keyword,
}

fn cases() -> [Case; 6] {
    [
        Case {
            id: "living_airship",
            regenerate: ManaGift {
                g: 1,
                c: 2,
                ..Default::default()
            },
            partial: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            body: (2, 3),
            keyword: Keyword::Flying,
        },
        Case {
            id: "screeching_harpy",
            regenerate: ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
            partial: ManaGift {
                b: 1,
                ..Default::default()
            },
            body: (2, 2),
            keyword: Keyword::Flying,
        },
        Case {
            id: "carnassid",
            regenerate: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            partial: ManaGift {
                c: 1,
                ..Default::default()
            },
            body: (5, 4),
            keyword: Keyword::Trample,
        },
        Case {
            id: "manor_skeleton",
            regenerate: ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
            partial: ManaGift {
                b: 1,
                ..Default::default()
            },
            body: (1, 1),
            keyword: Keyword::Haste,
        },
        Case {
            id: "lim-duls_high_guard",
            regenerate: ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
            partial: ManaGift {
                b: 1,
                ..Default::default()
            },
            body: (2, 1),
            keyword: Keyword::FirstStrike,
        },
        Case {
            id: "sanguine_guard",
            regenerate: ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
            partial: ManaGift {
                b: 1,
                ..Default::default()
            },
            body: (2, 2),
            keyword: Keyword::FirstStrike,
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[card]), deck_with("island", &[])]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut e);
    e
}

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn activate_regeneration(e: &mut GameEngine, creature: u32, mana: ManaGift, id: &str) {
    give_mana(e, 0, mana);
    apply_ability(e, 0, creature, 0, vec![]).expect("activate regeneration");
    pass_both_players(e);
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{id}: exact regeneration cost");
    assert_eq!(
        e.state.objects[&creature].regeneration_shields, 1,
        "{id}: one shield"
    );
}

#[test]
fn q42_regenerate_self_canary_simplest_places_a_shield() {
    let case = cases()
        .into_iter()
        .find(|case| case.id == "manor_skeleton")
        .expect("Manor Skeleton case");
    let mut e = engine(42_020_001, case.id);
    let creature = move_ready_to_battlefield(&mut e, 0, case.id);
    assert_eq!(
        (e.effective_power(creature), e.effective_toughness(creature)),
        (Some(case.body.0), Some(case.body.1))
    );
    assert!(e.effective_has_keyword(creature, Keyword::Haste));
    activate_regeneration(&mut e, creature, case.regenerate, case.id);
    assert!(
        !e.state.objects[&creature].tapped,
        "placing a shield does not tap"
    );
    assert_eq!(e.state.objects[&creature].damage, 0);
}

#[test]
fn q42_regenerate_self_canary_hardest_survives_lethal_damage() {
    let case = cases()
        .into_iter()
        .find(|case| case.id == "living_airship")
        .expect("Living Airship case");
    let mut e = engine(42_020_002, case.id);
    let creature = move_ready_to_battlefield(&mut e, 0, case.id);
    assert!(e.effective_has_keyword(creature, Keyword::Flying));
    activate_regeneration(&mut e, creature, case.regenerate, case.id);
    e.state.objects.get_mut(&creature).expect("creature").damage = case.body.1;
    pass_both_players(&mut e);
    assert_eq!(
        e.state.objects[&creature].zone,
        Zone::Battlefield,
        "regeneration replaces destruction"
    );
    assert_eq!(
        e.state.objects[&creature].regeneration_shields, 0,
        "the shield is consumed"
    );
    assert!(
        e.state.objects[&creature].tapped,
        "regeneration taps the creature"
    );
    assert_eq!(e.state.objects[&creature].damage, 0);
}

#[test]
fn q42_regenerate_self_cards_pay_their_printed_cost_and_consume_the_shield() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(42_021_000 + index as u64, case.id);
        let creature = move_ready_to_battlefield(&mut e, 0, case.id);
        assert_eq!(
            (e.effective_power(creature), e.effective_toughness(creature)),
            (Some(case.body.0), Some(case.body.1)),
            "{}: printed body",
            case.id
        );
        assert!(
            e.effective_has_keyword(creature, case.keyword),
            "{}: printed keyword",
            case.id
        );
        activate_regeneration(&mut e, creature, case.regenerate, case.id);
        e.state.objects.get_mut(&creature).expect("creature").damage = case.body.1;
        pass_both_players(&mut e);
        assert_eq!(
            e.state.objects[&creature].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert_eq!(
            e.state.objects[&creature].regeneration_shields, 0,
            "{}",
            case.id
        );
        assert!(e.state.objects[&creature].tapped, "{}", case.id);
        assert_eq!(e.state.objects[&creature].damage, 0, "{}", case.id);
    }
}

#[test]
fn q42_regenerate_self_cards_reject_activation_without_the_printed_mana() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(42_022_000 + index as u64, case.id);
        let creature = move_ready_to_battlefield(&mut e, 0, case.id);
        give_mana(&mut e, 0, case.partial);
        let before = format!("{:?}", e.state);
        assert!(
            apply_ability(&mut e, 0, creature, 0, vec![]).is_err(),
            "{}: the exact mana is required",
            case.id
        );
        assert_eq!(format!("{:?}", e.state), before, "{}", case.id);
        assert_eq!(
            e.state.objects[&creature].regeneration_shields, 0,
            "{}",
            case.id
        );
    }
}

#[test]
fn q42_regenerate_self_cards_offer_no_other_activated_ability() {
    for case in cases() {
        let definition = tricerules_cards::registry::global()
            .get(case.id)
            .unwrap_or_else(|| panic!("missing {}", case.id));
        let face = definition.primary_face();
        assert_eq!(face.activated_abilities.len(), 1, "{}", case.id);
        assert_eq!(face.triggered_abilities.len(), 0, "{}", case.id);
        assert_eq!(face.static_abilities.len(), 0, "{}", case.id);
    }
}
