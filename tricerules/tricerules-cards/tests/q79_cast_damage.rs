//! Registry conformance for the queue-v79 02-cast-damage cohort.
//!
//! Every card is one cast trigger that deals fixed damage to each opponent. The parameter is the
//! printed noncreature spell filter (instant or sorcery vs noncreature) and the printed damage.
//! Fingerprints are checked after the presentation metadata refresh.

use tricerules_cards::primitives::{
    Amount, CardTypeFilter, CastTriggerPlayer, PlayerRecipient, SpellEffectKind, TriggerCondition,
};
use tricerules_cards::{Keyword, Layout};

struct Card {
    id: &'static str,
    face_id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    power: u32,
    toughness: u32,
    keywords: &'static [Keyword],
    filter: CardTypeFilter,
    damage: u32,
    fingerprint: &'static str,
}

fn cases() -> Vec<Card> {
    vec![
        Card {
            id: "erebor_flamesmith",
            face_id: "erebor_flamesmith",
            name: "Erebor Flamesmith",
            mana_cost: "{1}{R}",
            types: &["Creature", "Dwarf", "Artificer"],
            power: 2,
            toughness: 1,
            keywords: &[],
            filter: CardTypeFilter::InstantOrSorcery,
            damage: 1,
            fingerprint: "3bd92b3ad8c71eecc9296ba1a70140da5aaba5ff1f785c214a6c56e824acdb74",
        },
        Card {
            id: "electrostatic_field",
            face_id: "electrostatic_field",
            name: "Electrostatic Field",
            mana_cost: "{1}{R}",
            types: &["Creature", "Wall"],
            power: 0,
            toughness: 4,
            keywords: &[Keyword::Defender],
            filter: CardTypeFilter::InstantOrSorcery,
            damage: 1,
            fingerprint: "8f4712237bdbc1e3fe05a42238b7e8aa317ad3d0ad80ad8330f12fe27142c29f",
        },
        Card {
            id: "kessig_flamebreather",
            face_id: "kessig_flamebreather",
            name: "Kessig Flamebreather",
            mana_cost: "{1}{R}",
            types: &["Creature", "Human", "Shaman"],
            power: 1,
            toughness: 3,
            keywords: &[],
            filter: CardTypeFilter::Noncreature,
            damage: 1,
            fingerprint: "66cc1ff3419c25bf9ae97c4e80110611c1b518302bfbe85c2d4dcb1e90981885",
        },
        Card {
            id: "black_waltz_no._3",
            face_id: "black_waltz_no_3",
            name: "Black Waltz No. 3",
            mana_cost: "{2}{B}{R}",
            types: &["Creature", "Wizard"],
            power: 2,
            toughness: 2,
            keywords: &[Keyword::Flying, Keyword::Deathtouch],
            filter: CardTypeFilter::Noncreature,
            damage: 2,
            fingerprint: "25799eb1526a7fc9fc833eb82d21bc26bc4264ecb4f8ac3b7a34ea85ce38bea7",
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
fn q79_cast_damage_map_definitions() {
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
        assert_eq!(
            (f.power, f.toughness),
            (Some(card.power), Some(card.toughness)),
            "{} P/T",
            card.id
        );
        assert_eq!(f.keywords, card.keywords, "{} keywords", card.id);
        assert!(f.activated_abilities.is_empty(), "{}", card.id);
        assert!(f.static_abilities.is_empty(), "{}", card.id);
        assert!(f.spell_effect.is_empty(), "{}", card.id);

        let [trigger] = f.triggered_abilities.as_slice() else {
            panic!("{}: exactly one triggered ability", card.id);
        };
        assert_eq!(trigger.ability_id.as_str(), "triggered_01", "{}", card.id);
        match &trigger.trigger {
            TriggerCondition::WheneverPlayerCastsSpell { caster, filter, .. } => {
                assert_eq!(*caster, CastTriggerPlayer::Controller, "{} caster", card.id);
                assert_eq!(filter.card_type, Some(card.filter), "{} filter", card.id);
            }
            other => panic!("{}: unexpected trigger {other:?}", card.id),
        }
        assert!(matches!(
            trigger.effect.as_slice(),
            [SpellEffectKind::DamagePlayer {
                amount: Amount::Fixed(damage),
                who: PlayerRecipient::EachOpponent,
            }] if *damage == card.damage
        ));
    }
}

#[test]
fn q79_cast_damage_fingerprints_match_the_pinned_oracle_text() {
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
