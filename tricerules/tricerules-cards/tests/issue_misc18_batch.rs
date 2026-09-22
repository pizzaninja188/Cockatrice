//! Registry conformance for the reviewed direct-RON entry/value batch.
//!
//! Magitek Armor, Dragoon's Wyvern, Fire Nation Warship, Knowledge Seeker, Disruptor of Currents,
//! Battle-Rattle Shaman and Bespoke Bō were promoted after complete-definition review against the
//! pinned Scryfall snapshot (exact records and `rulings_uri` fetched 2026-09-22). Governance:
//! CR 111.10 (Clue and Hero tokens), CR 115.3 (the "up to one other" bounce), CR 121.2/603.2 (the
//! second-draw ordinal), CR 301.5/702.6 (Equipment and equip), CR 301.7 (Vehicle), CR 508.1/603.2b
//! (beginning-of-combat timing), CR 603.6a (entry trigger), CR 603.6c (dies trigger), CR 702.9
//! (flying), CR 702.17 (reach), CR 702.20 (vigilance), CR 702.8 (flash), CR 702.51 (convoke),
//! and CR 702.122 (crew).

use tricerules_cards::primitives::{
    AbilityCost, Amount, CastTriggerPlayer, EffectSubject, ObjectContributionKind,
    ObjectPaymentConstraint, PermanentTypeFilter, SpellEffectKind, StaticAbilityDef, TargetKind,
    TriggerCondition,
};
use tricerules_cards::{CardRegistry, Keyword, Layout};

fn primary<'a>(registry: &'a CardRegistry, id: &str) -> &'a tricerules_cards::CardFace {
    registry
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
        .primary_face()
}

fn activated<'a>(
    registry: &'a CardRegistry,
    id: &str,
    index: usize,
) -> &'a tricerules_cards::primitives::ActivatedAbilityDef {
    &primary(registry, id).activated_abilities[index]
}

fn crew_minimum(costs: &[AbilityCost]) -> u32 {
    let [AbilityCost::TapPermanents {
        constraint:
            ObjectPaymentConstraint::AggregateMinimum {
                minimum,
                contribution: ObjectContributionKind::CurrentPower,
            },
        exclude_source: true,
        ..
    }] = costs
    else {
        panic!("crew cost shape: {costs:?}");
    };
    *minimum
}

fn chosen_bounce(filter: &tricerules_cards::primitives::TargetFilter) -> bool {
    filter.kind == TargetKind::AnyPermanent
        && filter.excluded_permanent_types == vec![PermanentTypeFilter::Land]
        && filter.excluded_objects.len() == 1
}

#[test]
fn issue_misc18_batch_maps_definitions() {
    let registry = CardRegistry::global();

    for (id, name, mana, types, power, toughness) in [
        (
            "magitek_armor",
            "Magitek Armor",
            "{3}{W}",
            &["Artifact", "Vehicle"][..],
            Some(4),
            Some(4),
        ),
        (
            "dragoons_wyvern",
            "Dragoon's Wyvern",
            "{2}{U}",
            &["Creature", "Drake"][..],
            Some(2),
            Some(1),
        ),
        (
            "fire_nation_warship",
            "Fire Nation Warship",
            "{3}",
            &["Artifact", "Vehicle"][..],
            Some(4),
            Some(4),
        ),
        (
            "knowledge_seeker",
            "Knowledge Seeker",
            "{1}{U}",
            &["Creature", "Fox", "Spirit"][..],
            Some(2),
            Some(1),
        ),
        (
            "disruptor_of_currents",
            "Disruptor of Currents",
            "{3}{U}{U}",
            &["Creature", "Merfolk", "Wizard"][..],
            Some(3),
            Some(3),
        ),
        (
            "battle-rattle_shaman",
            "Battle-Rattle Shaman",
            "{3}{R}",
            &["Creature", "Goblin", "Shaman"][..],
            Some(2),
            Some(2),
        ),
        (
            "bespoke_bō",
            "Bespoke Bō",
            "{2}{U}",
            &["Artifact", "Equipment"][..],
            None,
            None,
        ),
    ] {
        let definition = registry.get(id).expect("registered");
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        let f = definition.primary_face();
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(f.types, types, "{id} types");
        assert_eq!((f.power, f.toughness), (power, toughness), "{id} p/t");
    }

    // Magitek Armor: entry Hero token and Crew 1.
    let armor = primary(registry, "magitek_armor");
    assert!(matches!(
        &armor.triggered_abilities[0].effect[0],
        SpellEffectKind::CreateTokens { token, count: Amount::Fixed(1), .. }
            if token == "hero_c_1_1"
    ));
    assert_eq!(
        crew_minimum(&activated(registry, "magitek_armor", 0).costs),
        1
    );

    // Dragoon's Wyvern: flying and an entry Hero token.
    let wyvern = primary(registry, "dragoons_wyvern");
    assert!(wyvern.keywords.contains(&Keyword::Flying));
    assert!(matches!(
        &wyvern.triggered_abilities[0].effect[0],
        SpellEffectKind::CreateTokens { token, .. } if token == "hero_c_1_1"
    ));

    // Fire Nation Warship: reach, a dies Clue token, and Crew 2.
    let warship = primary(registry, "fire_nation_warship");
    assert!(warship.keywords.contains(&Keyword::Reach));
    assert_eq!(
        warship.triggered_abilities[0].trigger,
        TriggerCondition::WhenSelfDies
    );
    assert!(matches!(
        &warship.triggered_abilities[0].effect[0],
        SpellEffectKind::CreateTokens { token, .. } if token == "clue"
    ));
    assert_eq!(
        crew_minimum(&activated(registry, "fire_nation_warship", 0).costs),
        2
    );

    // Knowledge Seeker: vigilance, the second-draw counter, and a dies Clue token.
    let seeker = primary(registry, "knowledge_seeker");
    assert!(seeker.keywords.contains(&Keyword::Vigilance));
    assert!(matches!(
        seeker.triggered_abilities[0].trigger,
        TriggerCondition::WheneverPlayerDrawsNthCard { ordinal: 2, .. }
    ));
    assert!(matches!(
        &seeker.triggered_abilities[0].effect[0],
        SpellEffectKind::PutCounters {
            counter: tricerules_cards::CounterKind::PlusOnePlusOne,
            ..
        }
    ));
    assert_eq!(
        seeker.triggered_abilities[1].trigger,
        TriggerCondition::WhenSelfDies
    );
    assert!(matches!(
        &seeker.triggered_abilities[1].effect[0],
        SpellEffectKind::CreateTokens { token, .. } if token == "clue"
    ));

    // Disruptor of Currents: flash, convoke, and the "up to one other" nonland bounce.
    let disruptor = primary(registry, "disruptor_of_currents");
    assert!(disruptor.keywords.contains(&Keyword::Flash));
    assert!(disruptor.keywords.contains(&Keyword::Convoke));
    let [SpellEffectKind::ReturnToOwnersHand { subject }] =
        disruptor.triggered_abilities[0].effect.as_slice()
    else {
        panic!("{:?}", disruptor.triggered_abilities[0].effect);
    };
    let EffectSubject::Chosen(filter) = subject else {
        panic!("{subject:?}");
    };
    assert!(chosen_bounce(filter), "{filter:?}");
    let groups = disruptor.triggered_abilities[0]
        .targeting
        .as_ref()
        .expect("bounce targeting");
    assert_eq!((groups.groups[0].min, groups.groups[0].max), (0, 1));

    // Battle-Rattle Shaman: beginning-of-combat optional targeted +2/+0.
    let shaman = primary(registry, "battle-rattle_shaman");
    assert!(matches!(
        shaman.triggered_abilities[0].trigger,
        TriggerCondition::AtBeginningOfCombat {
            player: CastTriggerPlayer::Controller
        }
    ));
    assert!(matches!(
        &shaman.triggered_abilities[0].effect[0],
        SpellEffectKind::PumpTarget {
            power: 2,
            toughness: 0,
            ..
        }
    ));
    assert!(shaman.triggered_abilities[0].may);

    // Bespoke Bō: the "up to one other" bounce, the +2/+1 vigilance modifier, and equip {3}.
    let bo = primary(registry, "bespoke_bō");
    let [SpellEffectKind::ReturnToOwnersHand { subject }] =
        bo.triggered_abilities[0].effect.as_slice()
    else {
        panic!("{:?}", bo.triggered_abilities[0].effect);
    };
    let EffectSubject::Chosen(filter) = subject else {
        panic!("{subject:?}");
    };
    assert!(chosen_bounce(filter), "{filter:?}");
    assert!(matches!(
        &bo.static_abilities[0].definition,
        StaticAbilityDef::AttachedModifier {
            delta_power: 2,
            delta_toughness: 1,
            keywords,
            ..
        } if keywords == &vec![Keyword::Vigilance]
    ));
    assert!(matches!(
        activated(registry, "bespoke_bō", 0).costs.as_slice(),
        [AbilityCost::Mana(c)] if c.to_string() == "{3}"
    ));
}

#[test]
fn issue_misc18_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, fingerprint) in [
        (
            "magitek_armor",
            "Magitek Armor",
            "3f8d800a1dc7a93b147f82374592b5e84da86ae05b8bf45986eb84b40eaa89e8",
        ),
        (
            "dragoons_wyvern",
            "Dragoon's Wyvern",
            "22e9c218d35a91915b73ee17a207e0f2de782bc653726fe9fa959fa988e5fe98",
        ),
        (
            "fire_nation_warship",
            "Fire Nation Warship",
            "31b0834e7d3b03b43df2129a916aa0d64e28d11aa15547b4c4b53c3bde7781b1",
        ),
        (
            "knowledge_seeker",
            "Knowledge Seeker",
            "2f31cd3285c2c7e4ba5ca689dc75b008a19304b96e995f5cbadfdf455c2cea2d",
        ),
        (
            "disruptor_of_currents",
            "Disruptor of Currents",
            "8ca5bf9088b1aa3e84b2376965b8dcb99b309e1b393bd4141c4618f67b77b3ed",
        ),
        (
            "battle-rattle_shaman",
            "Battle-Rattle Shaman",
            "1d2ae13221e983657d9e80bd9505838fa38967b561a187c920e7730e76b5d0e2",
        ),
        (
            "bespoke_bō",
            "Bespoke Bō",
            "fe39e0740409193a4b1f264b620df7afdb60d628e78f1a198ce8b876aa56428a",
        ),
    ] {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[4], fingerprint, "fingerprint drift for {id}");
    }
}
