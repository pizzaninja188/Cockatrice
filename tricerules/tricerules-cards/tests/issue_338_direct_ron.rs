//! Registry conformance for the issue #338 reviewed direct-RON additional-cost batch.
//!
//! Worthy Cost (`0f8ebf11-b5ec-4f21-8c49-90d569b5a4c5`), Eaten Alive
//! (`d437ecc2-2fd3-4ad3-b23e-217f55e58dae`), Seize the Spoils (`58f83528-9110-4895-b5ea-51b90af30a8d`),
//! Duty Beyond Death (`67e6ce99-4883-4713-9822-cb2334765b5b`) and Arbiter of Woe
//! (`540693f3-985c-4a4a-945c-c957c5aad395`) were promoted after complete-definition review against
//! the pinned Scryfall snapshot. The exact Scryfall records and `rulings_uri` were fetched
//! 2026-09-20; none returned rulings. Governance: CR 118.8/601.2b/f-h (announced additional costs
//! and total cost), 701.16 (discard), 701.9 (sacrifice), 119.3/119.4 (life loss and gain),
//! 121.1 (draw), 111.10a (Treasure), 613 layer 6 and 122.1 (keyword grants and counters) as
//! applicable per card.

use tricerules_cards::primitives::{
    Amount, CastCostGroupDef, CastCostOptionDef, CreatureScopeController, CreatureScopeFilter,
    DiscardQuantity, EffectSubject, LifeAmount, ManaCostChoiceKind, ObjectCastCostKind,
    PermanentTypeFilter, PlayerRecipient, SpellEffectKind, TargetController, TargetFilter,
    TargetGroupDef, TargetKind, TargetingDef, TriggerCondition,
};
use tricerules_cards::{
    AbilityPresentation, CardRegistry, ChoiceId, Color, CounterKind, Keyword, Layout,
};

const WORTHY_COST_FINGERPRINT: &str =
    "a5201b1a3fe71957d72ff1579f0ace7f49e06e040bebc2ad2b198ad2fbcbcbf9";
const EATEN_ALIVE_FINGERPRINT: &str =
    "5a23871dc7907762eabf96c427ff140d0b30c413ed288ffb3f6da112cc8ccdce";
const SEIZE_THE_SPOILS_FINGERPRINT: &str =
    "0d86cc4130ff72499a77bc7734de4b910d05ef573829a96dc74d2dbb58395ee8";
const DUTY_BEYOND_DEATH_FINGERPRINT: &str =
    "0221c9031e80863206599a45cb1223df52bf4d067dcb1ff0fded620d5fd2e2c9";
const ARBITER_OF_WOE_FINGERPRINT: &str =
    "48a0f30aa128149b2959e68bafa14cc18316386454141ce574139c2b6ee50e8b";

fn single_group(targeting: &TargetingDef) -> &TargetGroupDef {
    let [group] = targeting.groups.as_slice() else {
        panic!("expected exactly one authored target group");
    };
    group
}

fn sacrifice_option(
    group: &CastCostGroupDef,
    index: usize,
) -> (&ChoiceId, ObjectCastCostKind, &TargetFilter) {
    let CastCostOptionDef::SacrificePermanent {
        option_id,
        kind,
        filter,
        ..
    } = &group.options[index]
    else {
        panic!("expected a sacrifice cast-cost option at index {index}");
    };
    (option_id, *kind, filter.as_ref())
}

fn creature_scope(controller: CreatureScopeController) -> CreatureScopeFilter {
    CreatureScopeFilter {
        controller: Some(controller),
        ..CreatureScopeFilter::default()
    }
}

fn creature_or_planeswalker() -> TargetFilter {
    TargetFilter {
        kind: TargetKind::AnyPermanent,
        permanent_types: vec![
            PermanentTypeFilter::Creature,
            PermanentTypeFilter::Planeswalker,
        ],
        ..TargetFilter::default()
    }
}

#[test]
fn issue_338_direct_ron_batch_maps_definitions() {
    let registry = CardRegistry::global();

    // Worthy Cost: mandatory creature sacrifice, then exile a creature or planeswalker.
    let worthy = registry.get("worthy_cost").expect("registered");
    assert_eq!(worthy.name, "Worthy Cost");
    assert_eq!(registry.id_for_name("Worthy Cost"), Some("worthy_cost"));
    assert_eq!(worthy.layout, Layout::Normal);
    assert_eq!(worthy.face_count(), 1);
    let face = worthy.primary_face();
    assert_eq!(face.face_id.as_str(), "worthy_cost");
    assert_eq!(face.mana_cost.to_string(), "{B}");
    assert_eq!(face.types, ["Sorcery"]);
    assert_eq!(face.colors(), vec![Color::Black]);
    assert!(face.spell_effect.len() == 1);
    {
        let [group] = face.cast_cost_groups.as_slice() else {
            panic!("Worthy Cost has exactly one cast-cost group");
        };
        assert_eq!(group.group_id, ChoiceId::new("additional_cost").unwrap());
        assert_eq!((group.min, group.max), (1, 1));
        assert_eq!(
            group.presentation,
            AbilityPresentation::OracleLines(vec![1])
        );
        let (option_id, kind, filter) = sacrifice_option(group, 0);
        assert_eq!(*option_id, ChoiceId::new("sacrifice_creature").unwrap());
        assert_eq!(kind, ObjectCastCostKind::AdditionalPayment);
        assert_eq!(
            filter,
            &TargetFilter {
                kind: TargetKind::Creature,
                controller: TargetController::You,
                ..TargetFilter::default()
            }
        );
        assert_eq!(
            group.options[0].presentation(),
            &AbilityPresentation::OracleLines(vec![1])
        );
    }
    assert_eq!(
        face.spell_effect[0],
        SpellEffectKind::Exile {
            subject: EffectSubject::Chosen(Box::new(creature_or_planeswalker())),
        }
    );
    let group = single_group(face.targeting.as_ref().expect("Worthy Cost targets"));
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(group.prompt, "Choose target creature or planeswalker");
    assert_eq!(group.effect_indices, [0]);

    // Eaten Alive: creature sacrifice or {3}{B}, then exile a creature or planeswalker.
    let eaten = registry.get("eaten_alive").expect("registered");
    assert_eq!(eaten.name, "Eaten Alive");
    assert_eq!(registry.id_for_name("Eaten Alive"), Some("eaten_alive"));
    let face = eaten.primary_face();
    assert_eq!(face.face_id.as_str(), "eaten_alive");
    assert_eq!(face.mana_cost.to_string(), "{B}");
    assert_eq!(face.types, ["Sorcery"]);
    assert_eq!(face.colors(), vec![Color::Black]);
    {
        let [group] = face.cast_cost_groups.as_slice() else {
            panic!("Eaten Alive has exactly one cast-cost group");
        };
        assert_eq!((group.min, group.max), (1, 1));
        let (option_id, kind, filter) = sacrifice_option(group, 0);
        assert_eq!(*option_id, ChoiceId::new("sacrifice_creature").unwrap());
        assert_eq!(kind, ObjectCastCostKind::AdditionalPayment);
        assert_eq!(
            filter,
            &TargetFilter {
                kind: TargetKind::Creature,
                controller: TargetController::You,
                ..TargetFilter::default()
            }
        );
        let CastCostOptionDef::Mana {
            option_id,
            kind,
            cost,
            ..
        } = &group.options[1]
        else {
            panic!("Eaten Alive's second option is the {{3}}{{B}} alternative");
        };
        assert_eq!(*option_id, ChoiceId::new("pay_mana").unwrap());
        assert_eq!(*kind, ManaCostChoiceKind::AdditionalPayment);
        assert_eq!(cost.to_string(), "{3}{B}");
    }
    assert_eq!(
        face.spell_effect[0],
        SpellEffectKind::Exile {
            subject: EffectSubject::Chosen(Box::new(creature_or_planeswalker())),
        }
    );
    let group = single_group(face.targeting.as_ref().expect("Eaten Alive targets"));
    assert_eq!(group.prompt, "Choose target creature or planeswalker");
    assert_eq!(group.effect_indices, [0]);

    // Seize the Spoils: discard a card, then draw two and make a Treasure.
    let seize = registry.get("seize_the_spoils").expect("registered");
    assert_eq!(seize.name, "Seize the Spoils");
    assert_eq!(
        registry.id_for_name("Seize the Spoils"),
        Some("seize_the_spoils")
    );
    let face = seize.primary_face();
    assert_eq!(face.face_id.as_str(), "seize_the_spoils");
    assert_eq!(face.mana_cost.to_string(), "{2}{R}");
    assert_eq!(face.types, ["Sorcery"]);
    assert_eq!(face.colors(), vec![Color::Red]);
    {
        let [group] = face.cast_cost_groups.as_slice() else {
            panic!("Seize the Spoils has exactly one cast-cost group");
        };
        let CastCostOptionDef::DiscardCard { option_id, .. } = &group.options[0] else {
            panic!("Seize the Spoils discards a card as its additional cost");
        };
        assert_eq!(*option_id, ChoiceId::new("discard_card").unwrap());
    }
    assert_eq!(
        face.spell_effect,
        [
            SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(2),
            },
            SpellEffectKind::CreateTokens {
                token: "treasure".into(),
                count: Amount::Fixed(1),
                who: PlayerRecipient::Controller,
                tapped: false,
                sacrifice_timing: None,
            },
        ]
    );
    assert!(face.targeting.is_none());
    assert!(registry.is_token("treasure"));

    // Duty Beyond Death: sacrifice a creature, then mass indestructible and a counter.
    let duty = registry.get("duty_beyond_death").expect("registered");
    assert_eq!(duty.name, "Duty Beyond Death");
    assert_eq!(
        registry.id_for_name("Duty Beyond Death"),
        Some("duty_beyond_death")
    );
    let face = duty.primary_face();
    assert_eq!(face.face_id.as_str(), "duty_beyond_death");
    assert_eq!(face.mana_cost.to_string(), "{1}{W}");
    assert_eq!(face.types, ["Instant"]);
    assert_eq!(face.colors(), vec![Color::White]);
    {
        let [group] = face.cast_cost_groups.as_slice() else {
            panic!("Duty Beyond Death has exactly one cast-cost group");
        };
        let (option_id, kind, filter) = sacrifice_option(group, 0);
        assert_eq!(*option_id, ChoiceId::new("sacrifice_creature").unwrap());
        assert_eq!(kind, ObjectCastCostKind::AdditionalPayment);
        assert_eq!(
            filter,
            &TargetFilter {
                kind: TargetKind::Creature,
                controller: TargetController::You,
                ..TargetFilter::default()
            }
        );
    }
    assert_eq!(
        face.spell_effect,
        [
            SpellEffectKind::GrantKeywordsAll {
                filter: creature_scope(CreatureScopeController::YouControl),
                keywords: vec![Keyword::Indestructible],
            },
            SpellEffectKind::PutCountersAll {
                counter: CounterKind::PlusOnePlusOne,
                count: Amount::Fixed(1),
                filter: creature_scope(CreatureScopeController::YouControl),
            },
        ]
    );
    assert!(face.targeting.is_none());

    // Arbiter of Woe: sacrifice a creature, Flying, and an entry drain over each opponent.
    let arbiter = registry.get("arbiter_of_woe").expect("registered");
    assert_eq!(arbiter.name, "Arbiter of Woe");
    assert_eq!(
        registry.id_for_name("Arbiter of Woe"),
        Some("arbiter_of_woe")
    );
    let face = arbiter.primary_face();
    assert_eq!(face.face_id.as_str(), "arbiter_of_woe");
    assert_eq!(face.mana_cost.to_string(), "{4}{B}{B}");
    assert_eq!(face.types, ["Creature", "Demon"]);
    assert_eq!(face.colors(), vec![Color::Black]);
    assert_eq!((face.power, face.toughness), (Some(5), Some(4)));
    assert_eq!(face.keywords, [Keyword::Flying]);
    assert!(face.spell_effect.is_empty());
    {
        let [group] = face.cast_cost_groups.as_slice() else {
            panic!("Arbiter of Woe has exactly one cast-cost group");
        };
        let (option_id, kind, filter) = sacrifice_option(group, 0);
        assert_eq!(*option_id, ChoiceId::new("sacrifice_creature").unwrap());
        assert_eq!(kind, ObjectCastCostKind::AdditionalPayment);
        assert_eq!(
            filter,
            &TargetFilter {
                kind: TargetKind::Creature,
                controller: TargetController::You,
                ..TargetFilter::default()
            }
        );
    }
    let [trigger] = face.triggered_abilities.as_slice() else {
        panic!("Arbiter of Woe has exactly one entry trigger");
    };
    assert_eq!(trigger.ability_id.as_str(), "triggered_01");
    assert_eq!(
        trigger.presentation,
        AbilityPresentation::OracleLines(vec![3])
    );
    assert_eq!(trigger.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert_eq!(
        trigger.effect,
        [
            SpellEffectKind::Discard {
                who: PlayerRecipient::EachOpponent,
                quantity: DiscardQuantity::Exact(1),
            },
            SpellEffectKind::LoseLife {
                amount: LifeAmount::Fixed(2),
                who: PlayerRecipient::EachOpponent,
            },
            SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(1),
            },
            SpellEffectKind::GainLife {
                amount: Amount::Fixed(2),
            },
        ]
    );
    assert!(trigger.targeting.is_none());
}

#[test]
fn issue_338_direct_ron_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, expected) in [
        ("worthy_cost", "Worthy Cost", WORTHY_COST_FINGERPRINT),
        ("eaten_alive", "Eaten Alive", EATEN_ALIVE_FINGERPRINT),
        (
            "seize_the_spoils",
            "Seize the Spoils",
            SEIZE_THE_SPOILS_FINGERPRINT,
        ),
        (
            "duty_beyond_death",
            "Duty Beyond Death",
            DUTY_BEYOND_DEATH_FINGERPRINT,
        ),
        (
            "arbiter_of_woe",
            "Arbiter of Woe",
            ARBITER_OF_WOE_FINGERPRINT,
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
        assert_eq!(fields[2], id);
        assert_eq!(fields[3], name);
        assert_eq!(fields[4], expected, "fingerprint drift for {id}");
    }
}
