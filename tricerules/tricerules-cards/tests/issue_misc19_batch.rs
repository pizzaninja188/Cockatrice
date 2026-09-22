//! Registry conformance for the reviewed direct-RON tutor, reanimation, and bounce batch.
//!
//! Rune-Scarred Demon, Starfield Shepherd, Scampering Surveyor, Peerless Ropemaster, Ragamuffin
//! Raptor, Defibrillating Current, Unsparing Boltcaster and Emerge from the Cocoon were promoted
//! after complete-definition review against the pinned Scryfall snapshot (exact records and
//! `rulings_uri` fetched 2026-09-22). Governance: CR 115.3 (the "up to one" bounce and reanimation
//! targets), CR 119.3 (life gain), CR 120.3 (damage), CR 400.7/608.2b (a returned card is a new
//! object), CR 603.6a (entry trigger), CR 608.2b (an illegal target fails the whole spell), CR 609.2
//! (the dealt-damage-this-turn predicate), CR 701.23 (search), CR 702.9 (flying), and CR 702.185
//! (warp).

use tricerules_cards::primitives::{
    Amount, EffectSubject, GraveyardDestination, SearchDestination, SpellEffectKind, TargetKind,
    TriggerCondition,
};
use tricerules_cards::{CardRegistry, Keyword, Layout};

fn primary<'a>(registry: &'a CardRegistry, id: &str) -> &'a tricerules_cards::CardFace {
    registry
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
        .primary_face()
}

#[test]
fn issue_misc19_batch_maps_definitions() {
    let registry = CardRegistry::global();

    for (id, name, mana, types, power, toughness) in [
        (
            "rune-scarred_demon",
            "Rune-Scarred Demon",
            "{5}{B}{B}",
            &["Creature", "Demon"][..],
            Some(6),
            Some(6),
        ),
        (
            "starfield_shepherd",
            "Starfield Shepherd",
            "{3}{W}{W}",
            &["Creature", "Angel"][..],
            Some(3),
            Some(2),
        ),
        (
            "scampering_surveyor",
            "Scampering Surveyor",
            "{4}",
            &["Artifact", "Creature", "Gnome"][..],
            Some(3),
            Some(2),
        ),
        (
            "peerless_ropemaster",
            "Peerless Ropemaster",
            "{4}{U}",
            &["Creature", "Human", "Rogue"][..],
            Some(4),
            Some(4),
        ),
        (
            "ragamuffin_raptor",
            "Ragamuffin Raptor",
            "{4}{G}",
            &["Creature", "Dinosaur"][..],
            Some(4),
            Some(3),
        ),
        (
            "defibrillating_current",
            "Defibrillating Current",
            "{2/R}{2/W}{2/B}",
            &["Sorcery"][..],
            None,
            None,
        ),
        (
            "unsparing_boltcaster",
            "Unsparing Boltcaster",
            "{2}{R}",
            &["Creature", "Ogre", "Wizard"][..],
            Some(3),
            Some(3),
        ),
        (
            "emerge_from_the_cocoon",
            "Emerge from the Cocoon",
            "{4}{W}",
            &["Sorcery"][..],
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

    // Rune-Scarred Demon: flying and an unrestricted entry tutor.
    let demon = primary(registry, "rune-scarred_demon");
    assert!(demon.keywords.contains(&Keyword::Flying));
    assert!(matches!(
        &demon.triggered_abilities[0].effect[0],
        SpellEffectKind::SearchLibrary {
            filter: None,
            destination: SearchDestination::Hand,
            ..
        }
    ));

    // Starfield Shepherd: flying, Warp {1}{W}, and a two-branch filtered entry search.
    let shepherd = primary(registry, "starfield_shepherd");
    assert!(shepherd.keywords.contains(&Keyword::Flying));
    assert_eq!(
        shepherd.warp_cost.as_ref().map(|c| c.to_string()),
        Some("{1}{W}".to_string())
    );
    let SpellEffectKind::SearchLibrary { filter, .. } = &shepherd.triggered_abilities[0].effect[0]
    else {
        panic!("{:?}", shepherd.triggered_abilities[0].effect);
    };
    let branches = filter
        .as_ref()
        .and_then(|f| f.any_of.as_ref())
        .expect("two-branch search filter");
    assert_eq!(branches.len(), 2);

    // Scampering Surveyor: entry search onto the battlefield tapped.
    assert!(matches!(
        &primary(registry, "scampering_surveyor").triggered_abilities[0].effect[0],
        SpellEffectKind::SearchLibrary {
            destination: SearchDestination::Battlefield { tapped: true },
            ..
        }
    ));

    // Peerless Ropemaster: bounce up to one tapped creature.
    let ropemaster = primary(registry, "peerless_ropemaster");
    let [SpellEffectKind::ReturnToOwnersHand { subject }] =
        ropemaster.triggered_abilities[0].effect.as_slice()
    else {
        panic!("{:?}", ropemaster.triggered_abilities[0].effect);
    };
    let EffectSubject::Chosen(filter) = subject else {
        panic!("{subject:?}");
    };
    assert_eq!(filter.kind, TargetKind::Creature);
    assert_eq!(filter.tapped, Some(true));

    // Ragamuffin Raptor: return a creature or Food card from the graveyard to hand.
    let raptor = primary(registry, "ragamuffin_raptor");
    let [SpellEffectKind::MoveGraveyardCards {
        filter,
        destination: GraveyardDestination::Hand,
        ..
    }] = raptor.triggered_abilities[0].effect.as_slice()
    else {
        panic!("{:?}", raptor.triggered_abilities[0].effect);
    };
    assert_eq!(
        filter
            .card
            .as_ref()
            .and_then(|c| c.any_of.as_ref())
            .map(Vec::len),
        Some(2)
    );

    // Defibrillating Current: 4 damage to a creature or planeswalker plus 2 life.
    let current = primary(registry, "defibrillating_current");
    assert!(matches!(
        &current.spell_effect[0],
        SpellEffectKind::DamageTarget { amount: Amount::Fixed(4), target }
            if target.any_of.as_ref().map(Vec::len) == Some(2)
    ));
    assert!(matches!(
        &current.spell_effect[1],
        SpellEffectKind::GainLife {
            amount: Amount::Fixed(2)
        }
    ));

    // Unsparing Boltcaster: entry damage only to a damaged opponent creature.
    let boltcaster = primary(registry, "unsparing_boltcaster");
    assert!(matches!(
        &boltcaster.triggered_abilities[0].effect[0],
        SpellEffectKind::DamageTarget { amount: Amount::Fixed(5), target }
            if target.was_dealt_damage_this_turn == Some(true)
    ));

    // Emerge from the Cocoon: reanimate a creature card and gain 3 life.
    let emerge = primary(registry, "emerge_from_the_cocoon");
    assert!(matches!(
        &emerge.spell_effect[0],
        SpellEffectKind::MoveGraveyardCards {
            destination: GraveyardDestination::Battlefield { .. },
            ..
        }
    ));
    assert!(matches!(
        &emerge.spell_effect[1],
        SpellEffectKind::GainLife {
            amount: Amount::Fixed(3)
        }
    ));

    // No trigger carries an unexpected intervening condition.
    for id in [
        "rune-scarred_demon",
        "starfield_shepherd",
        "scampering_surveyor",
        "peerless_ropemaster",
        "ragamuffin_raptor",
        "unsparing_boltcaster",
    ] {
        let ability = &primary(registry, id).triggered_abilities[0];
        assert_eq!(
            ability.trigger,
            TriggerCondition::WhenSelfEntersBattlefield,
            "{id}"
        );
    }
}

#[test]
fn issue_misc19_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, fingerprint) in [
        (
            "rune-scarred_demon",
            "Rune-Scarred Demon",
            "773cb77919c14fc556fd924172f18f472253a6ecfc14f358f1bc805943316fed",
        ),
        (
            "starfield_shepherd",
            "Starfield Shepherd",
            "03a97285b16bb8e1111a87db6fee91fea9718016e2558fd33fec1dc1a91b0625",
        ),
        (
            "scampering_surveyor",
            "Scampering Surveyor",
            "8463b8747204223dc51d6905ffbbc6fcded932ae6a6541920e84bfaa9b05d562",
        ),
        (
            "peerless_ropemaster",
            "Peerless Ropemaster",
            "13937d2ea5db7285d3a046cb92ce0b939f530e8b08a5b2186fd3e6f945cb45e8",
        ),
        (
            "ragamuffin_raptor",
            "Ragamuffin Raptor",
            "4acf74e664726ce1368ef43e6a7db2bf958840eab0cecc56b1e4fe19b4a34afd",
        ),
        (
            "defibrillating_current",
            "Defibrillating Current",
            "ee9ae00c851311f69277120dc5f82cf1d7a370a104cdfc25abaa3cd3f227fc15",
        ),
        (
            "unsparing_boltcaster",
            "Unsparing Boltcaster",
            "ecc04a019778c6638fab9acbced30f665461f609881861bbeafb6dd743277ef8",
        ),
        (
            "emerge_from_the_cocoon",
            "Emerge from the Cocoon",
            "9e41779014c2a866e58a4e43be4a6057be6ca1564b07e4d2fb3fe7fb75a8e1da",
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
