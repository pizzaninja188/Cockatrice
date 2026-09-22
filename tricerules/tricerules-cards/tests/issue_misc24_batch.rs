//! Complete-card definition checks for five reviewed life-trigger Standard identities.

use tricerules_cards::primitives::{PlayerRecipient, SpellEffectKind, TriggerCondition};
use tricerules_cards::{CardRegistry, Keyword, Layout};

#[test]
fn issue_misc24_batch_maps_printed_cards_and_distinct_life_triggers() {
    let registry = CardRegistry::global();
    for (id, name, mana, types, stats) in [
        (
            "potioners_trove",
            "Potioner's Trove",
            "{3}",
            &["Artifact"][..],
            None,
        ),
        (
            "wylie_duke,_atiin_hero",
            "Wylie Duke, Atiin Hero",
            "{1}{G}{W}",
            &["Creature", "Human", "Ranger"][..],
            Some((4, 2)),
        ),
        (
            "jeskai_shrinekeeper",
            "Jeskai Shrinekeeper",
            "{2}{U}{R}{W}",
            &["Creature", "Dragon"][..],
            Some((3, 3)),
        ),
        (
            "pactdoll_terror",
            "Pactdoll Terror",
            "{3}{B}",
            &["Artifact", "Creature", "Toy"][..],
            Some((3, 4)),
        ),
        (
            "shroudstomper",
            "Shroudstomper",
            "{3}{W}{W}{B}{B}",
            &["Creature", "Elemental"][..],
            Some((5, 5)),
        ),
    ] {
        let card = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(card.layout, Layout::Normal, "{id}");
        let face = card.primary_face();
        assert_eq!(face.name, name, "{id}");
        assert_eq!(face.mana_cost.to_string(), mana, "{id}");
        assert_eq!(face.types, types, "{id}");
        assert_eq!(face.power.zip(face.toughness), stats, "{id}");
    }

    let trove = registry.get("potioners_trove").unwrap().primary_face();
    assert_eq!(trove.activated_abilities.len(), 2);
    assert_eq!(trove.activated_abilities[1].conditions.len(), 1);

    let wylie = registry
        .get("wylie_duke,_atiin_hero")
        .unwrap()
        .primary_face();
    assert_eq!(wylie.supertypes, ["Legendary"]);
    assert!(wylie.keywords.contains(&Keyword::Vigilance));
    assert_eq!(
        wylie.triggered_abilities[0].trigger,
        TriggerCondition::WheneverSelfBecomesTapped
    );
    assert!(matches!(
        &wylie.triggered_abilities[0].effect[..],
        [
            SpellEffectKind::GainLife { .. },
            SpellEffectKind::Draw { .. }
        ]
    ));

    let jeskai = registry.get("jeskai_shrinekeeper").unwrap().primary_face();
    assert!(
        jeskai.keywords.contains(&Keyword::Flying) && jeskai.keywords.contains(&Keyword::Haste)
    );
    assert_eq!(
        jeskai.triggered_abilities[0].trigger,
        TriggerCondition::WheneverSelfDealsCombatDamageToPlayer
    );

    let pactdoll = registry.get("pactdoll_terror").unwrap().primary_face();
    let TriggerCondition::WheneverPermanentEntersBattlefield { filter, .. } =
        &pactdoll.triggered_abilities[0].trigger
    else {
        panic!("Pactdoll trigger");
    };
    assert!(!filter.exclude_source, "Pactdoll must see itself enter");
    assert!(matches!(
        &pactdoll.triggered_abilities[0].effect[..],
        [
            SpellEffectKind::LoseLife {
                who: PlayerRecipient::EachOpponent,
                ..
            },
            SpellEffectKind::GainLife { .. }
        ]
    ));

    let shroud = registry.get("shroudstomper").unwrap().primary_face();
    assert!(shroud.keywords.contains(&Keyword::Deathtouch));
    assert_eq!(shroud.triggered_abilities.len(), 2);
    assert_eq!(
        shroud.triggered_abilities[0].trigger,
        TriggerCondition::WhenSelfEntersBattlefield
    );
    assert_eq!(
        shroud.triggered_abilities[1].trigger,
        TriggerCondition::WheneverSelfAttacks {
            minimum_other_attackers: 0
        }
    );
}
