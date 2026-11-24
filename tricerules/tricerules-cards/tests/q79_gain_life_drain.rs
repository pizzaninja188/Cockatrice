//! Registry conformance for the queue-v79 01-gain-life-drain cohort.
//!
//! Every card is one "whenever you gain life, each opponent loses 1 life" trigger. Dina also
//! has a sacrifice-pump activated ability whose X is the sacrificed creature's last-known
//! battlefield power. Fingerprints are checked after the presentation metadata refresh.

use tricerules_cards::primitives::{
    AbilityCost, Amount, BattlefieldQuantityCharacteristic, CardResultAction, CardResultFilter,
    CardResultSource, CastTriggerPlayer, CountExpression, EffectSubject, LifeAmount,
    PlayerRecipient, RelativePlayerSet, SpellEffectKind, TargetController, TargetKind,
    TriggerCondition,
};
use tricerules_cards::Layout;

struct Card {
    id: &'static str,
    face_id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    supertypes: &'static [&'static str],
    power: u32,
    toughness: u32,
    keywords: &'static [&'static str],
    activated: bool,
    fingerprint: &'static str,
}

fn cases() -> Vec<Card> {
    vec![
        Card {
            id: "epicure_of_blood",
            face_id: "epicure_of_blood",
            name: "Epicure of Blood",
            mana_cost: "{4}{B}",
            types: &["Creature", "Vampire"],
            supertypes: &[],
            power: 4,
            toughness: 4,
            keywords: &[],
            activated: false,
            fingerprint: "616a3b7a8f2656839f794f9e9d3de5ad6f643b3891a879798991dbb182ab3b46",
        },
        Card {
            id: "cliffhaven_vampire",
            face_id: "cliffhaven_vampire",
            name: "Cliffhaven Vampire",
            mana_cost: "{2}{W}{B}",
            types: &["Creature", "Vampire", "Warrior", "Ally"],
            supertypes: &[],
            power: 2,
            toughness: 4,
            keywords: &["Flying"],
            activated: false,
            fingerprint: "0095e5c8d1f4c4d96e0547e46307fa19b5e6a9ec896bdf3c3e68d4ff835a7723",
        },
        Card {
            id: "dina,_soul_steeper",
            face_id: "dina_soul_steeper",
            name: "Dina, Soul Steeper",
            mana_cost: "{B}{G}",
            types: &["Creature", "Dryad", "Druid"],
            supertypes: &["Legendary"],
            power: 1,
            toughness: 3,
            keywords: &[],
            activated: true,
            fingerprint: "b1f5e4c1634bb1a328484a50c0b7945a69892174acbc5739242fa3442ea378d4",
        },
    ]
}

fn face<'a>(
    registry: &'a tricerules_cards::CardRegistry,
    id: &str,
) -> &'a tricerules_cards::CardFace {
    registry
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
        .primary_face()
}

#[test]
fn q79_gain_life_drain_map_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in cases() {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing {}", card.id));
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert_eq!(
            registry.id_for_name(card.name),
            Some(card.id),
            "{}",
            card.id
        );
        let f = face(registry, card.id);
        assert_eq!(f.face_id.as_str(), card.face_id, "{} face id", card.id);
        assert_eq!(f.name, card.name, "{} name", card.id);
        assert_eq!(f.mana_cost.to_string(), card.mana_cost, "{} mana", card.id);
        assert_eq!(f.types, card.types, "{} types", card.id);
        assert_eq!(f.supertypes, card.supertypes, "{} supertypes", card.id);
        assert_eq!(
            (f.power, f.toughness),
            (Some(card.power), Some(card.toughness)),
            "{} P/T",
            card.id
        );
        assert_eq!(
            f.keywords,
            card.keywords
                .iter()
                .map(|keyword| match *keyword {
                    "Flying" => tricerules_cards::Keyword::Flying,
                    other => panic!("{}: unexpected keyword {other}", card.id),
                })
                .collect::<Vec<_>>(),
            "{} keywords",
            card.id
        );

        let [trigger] = f.triggered_abilities.as_slice() else {
            panic!("{}: exactly one triggered ability", card.id);
        };
        assert_eq!(trigger.ability_id.as_str(), "triggered_01", "{}", card.id);
        match &trigger.trigger {
            TriggerCondition::WheneverPlayerGainsLife { player, .. } => {
                assert_eq!(*player, CastTriggerPlayer::Controller, "{} player", card.id);
            }
            other => panic!("{}: unexpected trigger {other:?}", card.id),
        }
        assert!(matches!(
            trigger.effect.as_slice(),
            [SpellEffectKind::LoseLife {
                amount: LifeAmount::Fixed(1),
                who: PlayerRecipient::EachOpponent,
            }]
        ));

        if !card.activated {
            assert!(f.activated_abilities.is_empty(), "{}", card.id);
            continue;
        }
        let [ability] = f.activated_abilities.as_slice() else {
            panic!("{}: exactly one activated ability", card.id);
        };
        assert_eq!(ability.ability_id.as_str(), "activated_01", "{}", card.id);
        let [AbilityCost::Mana(mana), AbilityCost::SacrificePermanent { filter, count: 1 }] =
            ability.costs.as_slice()
        else {
            panic!("{}: mana plus one selected sacrifice", card.id);
        };
        assert_eq!(mana.to_string(), "{1}", "{}", card.id);
        assert_eq!(filter.kind, TargetKind::Creature, "{}", card.id);
        assert_eq!(filter.controller, TargetController::You, "{}", card.id);
        assert!(
            filter
                .excluded_objects
                .contains(&tricerules_cards::TargetObjectExclusion::Source),
            "{}: another creature",
            card.id
        );
        let [SpellEffectKind::PumpTarget {
            power: 0,
            toughness: 0,
            scale: Some(scale),
            subject: EffectSubject::Source,
        }] = ability.effect.as_slice()
        else {
            panic!("{}: source pump with a scale", card.id);
        };
        assert_eq!(scale.power_per_unit, 1, "{}", card.id);
        assert_eq!(scale.toughness_per_unit, 0, "{}", card.id);
        match &scale.basis {
            tricerules_cards::primitives::PtScaleBasis::Amount(Amount::Count(
                CountExpression::CardResultCharacteristicSum {
                    filter:
                        CardResultFilter {
                            source: CardResultSource::Payment,
                            action: CardResultAction::Sacrifice,
                            players: RelativePlayerSet::All,
                            card_type: Some(tricerules_cards::primitives::CardTypeFilter::Creature),
                        },
                    characteristic: BattlefieldQuantityCharacteristic::Power,
                },
            )) => {}
            other => panic!("{}: unexpected pump scale {other:?}", card.id),
        }
    }
}

#[test]
fn q79_gain_life_drain_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for card in cases() {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{}\t", card.id)))
            .unwrap_or_else(|| panic!("missing fingerprint row for {}", card.id));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], card.id);
        assert_eq!(fields[1], card.name);
        assert_eq!(fields[2], card.face_id, "face id drift for {}", card.id);
        assert_eq!(
            fields[4], card.fingerprint,
            "fingerprint drift for {}",
            card.id
        );
    }
}
