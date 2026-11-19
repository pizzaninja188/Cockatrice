//! Actual-card scenarios for the queue-v39 targeted utility-land cohort.
//!
//! Expectations are printed Oracle values. CR 106.1 resolves the mana ability immediately; CR
//! 602.1/118.3 pay the utility activation; CR 701.26 taps the land; CR 611.2a/514.2 ends pumps and
//! grants at cleanup; CR 613.4c applies P/T; CR 613.1f grants keywords; and CR 608.2b fizzles a
//! stale target.
use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: (u32, u32, u32, u32, u32, u32),
    cost: ManaGift,
    power: i32,
    toughness: i32,
    keywords: &'static [Keyword],
    target: Target,
}

#[derive(Clone, Copy)]
enum Target {
    Soldier,
    Goblin,
    Legendary,
    Any,
}

fn cases() -> [Case; 6] {
    [
        Case {
            id: "daru_encampment",
            mana: (0, 0, 0, 0, 0, 1),
            cost: ManaGift {
                w: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 1,
            keywords: &[],
            target: Target::Soldier,
        },
        Case {
            id: "goblin_burrows",
            mana: (0, 0, 0, 0, 0, 1),
            cost: ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
            power: 2,
            toughness: 0,
            keywords: &[],
            target: Target::Goblin,
        },
        Case {
            id: "okina,_temple_to_the_grandfathers",
            mana: (0, 0, 0, 0, 1, 0),
            cost: ManaGift {
                g: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 1,
            keywords: &[],
            target: Target::Legendary,
        },
        Case {
            id: "shinka,_the_bloodsoaked_keep",
            mana: (0, 0, 0, 1, 0, 0),
            cost: ManaGift {
                r: 1,
                ..Default::default()
            },
            power: 0,
            toughness: 0,
            keywords: &[Keyword::FirstStrike],
            target: Target::Legendary,
        },
        Case {
            id: "slayers_stronghold",
            mana: (0, 0, 0, 0, 0, 1),
            cost: ManaGift {
                r: 1,
                w: 1,
                ..Default::default()
            },
            power: 2,
            toughness: 0,
            keywords: &[Keyword::Vigilance, Keyword::Haste],
            target: Target::Any,
        },
        Case {
            id: "sunhome,_fortress_of_the_legion",
            mana: (0, 0, 0, 0, 0, 1),
            cost: ManaGift {
                r: 1,
                w: 1,
                c: 2,
                ..Default::default()
            },
            power: 0,
            toughness: 0,
            keywords: &[Keyword::DoubleStrike],
            target: Target::Any,
        },
    ]
}

fn gift(m: &ManaGift) -> ManaGift {
    ManaGift {
        w: m.w,
        u: m.u,
        b: m.b,
        r: m.r,
        g: m.g,
        c: m.c,
    }
}

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("island", &[])]);
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

fn target_for(e: &mut GameEngine, target: Target) -> u32 {
    match target {
        Target::Soldier => inject_creature_on_battlefield(e, 0, "elite_vanguard"),
        Target::Goblin => inject_creature_on_battlefield(e, 0, "goblin_piker"),
        Target::Legendary => inject_creature_on_battlefield(e, 0, "rorix_bladewing"),
        Target::Any => inject_creature_with_stats(e, 0, "grizzly_bears", 2, 2),
    }
}

fn illegal_target(e: &mut GameEngine, target: Target) -> u32 {
    match target {
        Target::Soldier => inject_creature_on_battlefield(e, 0, "goblin_piker"),
        Target::Goblin => inject_creature_on_battlefield(e, 0, "elite_vanguard"),
        Target::Legendary => inject_creature_on_battlefield(e, 0, "grizzly_bears"),
        Target::Any => inject_permanent_on_battlefield(e, 0, "forest"),
    }
}

fn activate_mana(e: &mut GameEngine, land: u32, case: &Case) {
    e.apply_scenario_command(0, &activate_ability_for(e, land, 0, vec![]))
        .unwrap_or_else(|error| panic!("{}: mana: {error:?}", case.id));
    assert_eq!(pool(e), case.mana, "{}: exact printed mana", case.id);
    assert!(e.state.objects[&land].tapped, "{}", case.id);
}

fn activate_utility(e: &mut GameEngine, land: u32, case: &Case, target: u32) -> Result<(), String> {
    give_mana(e, 0, gift(&case.cost));
    apply_ability(e, 0, land, 1, target_object(target))
        .map(|_| ())
        .map_err(|error| format!("{error:?}"))
}

#[test]
fn q39_targeted_utility_lands_canary_simplest() {
    let case = &cases()[0];
    let mut e = engine(39_090_001);
    let mana_land = inject_permanent_on_battlefield(&mut e, 0, case.id);
    activate_mana(&mut e, mana_land, case);
    let land = inject_permanent_on_battlefield(&mut e, 0, case.id);
    let target = target_for(&mut e, case.target);
    activate_utility(&mut e, land, case, target).expect("activate Daru Encampment");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        (e.effective_power(target), e.effective_toughness(target)),
        (Some(3), Some(3))
    );
    assert!(e.state.objects[&land].tapped);
}

#[test]
fn q39_targeted_utility_lands_canary_hardest() {
    let case = &cases()[5];
    let mut e = engine(39_090_002);
    let mana_land = inject_permanent_on_battlefield(&mut e, 0, case.id);
    activate_mana(&mut e, mana_land, case);
    let land = inject_permanent_on_battlefield(&mut e, 0, case.id);
    let target = target_for(&mut e, case.target);
    activate_utility(&mut e, land, case, target).expect("activate Sunhome");
    resolve_entire_stack_two_player(&mut e);
    assert!(e.effective_has_keyword(target, Keyword::DoubleStrike));
    assert_eq!(
        (e.effective_power(target), e.effective_toughness(target)),
        (Some(2), Some(2))
    );
}

#[test]
fn q39_targeted_utility_lands_cards_produce_their_printed_mana() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(39_091_000 + index as u64);
        let land = inject_permanent_on_battlefield(&mut e, 0, case.id);
        activate_mana(&mut e, land, case);
    }
}

#[test]
fn q39_targeted_utility_lands_cards_apply_their_printed_pump_or_keyword() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(39_092_000 + index as u64);
        let land = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let target = target_for(&mut e, case.target);
        let bystander = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        activate_utility(&mut e, land, case, target).expect("activate utility");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            (e.effective_power(target), e.effective_toughness(target)),
            (
                Some((2 + case.power) as u32),
                Some((2 + case.toughness) as u32)
            ),
            "{}",
            case.id
        );
        for keyword in case.keywords {
            assert!(
                e.effective_has_keyword(target, *keyword),
                "{}: {:?}",
                case.id,
                keyword
            );
            assert!(!e.effective_has_keyword(bystander, *keyword), "{}", case.id);
        }
        assert_eq!(
            (
                e.effective_power(bystander),
                e.effective_toughness(bystander)
            ),
            (Some(2), Some(2)),
            "{}",
            case.id
        );
        assert!(e.state.objects[&land].tapped, "{}", case.id);
    }
}

#[test]
fn q39_targeted_utility_lands_cards_reject_an_illegal_target_and_missing_mana() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(39_093_000 + index as u64);
        let land = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let wrong = illegal_target(&mut e, case.target);
        give_mana(&mut e, 0, gift(&case.cost));
        assert!(
            apply_ability(&mut e, 0, land, 1, target_object(wrong)).is_err(),
            "{}: illegal target",
            case.id
        );
        assert!(
            !e.state.objects[&land].tapped,
            "{}: no tap on rejected activation",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
        e.state.players[0].mana_pool = Default::default();
        let target = target_for(&mut e, case.target);
        assert!(
            apply_ability(&mut e, 0, land, 1, target_object(target)).is_err(),
            "{}: missing mana",
            case.id
        );
        assert_eq!(
            (e.effective_power(target), e.effective_toughness(target)),
            (Some(2), Some(2)),
            "{}: no effect",
            case.id
        );
    }
}

#[test]
fn q39_targeted_utility_lands_cards_fizzle_when_the_target_left() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(39_094_000 + index as u64);
        let land = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let target = target_for(&mut e, case.target);
        give_mana(&mut e, 0, gift(&case.cost));
        apply_ability(&mut e, 0, land, 1, target_object(target)).expect("activate utility");
        e.state.players[0]
            .battlefield
            .retain(|object| *object != target);
        e.state.players[0].graveyard.push(target);
        e.state.objects.get_mut(&target).expect("target").zone = Zone::Graveyard;
        *e.state.zone_change_generation.entry(target).or_default() += 1;
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.players[0].life, 20, "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}
