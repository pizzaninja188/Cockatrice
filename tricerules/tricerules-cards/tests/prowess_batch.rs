//! Registry conformance for the reviewed Prowess creature family.
//!
//! Every included card is a normal-layout creature with Prowess, one printed
//! evergreen keyword, and the shared noncreature-cast trigger. Each included
//! card is executed by the shared actual-card scenarios.
use tricerules_cards::primitives::{
    CardTypeFilter, CastTriggerPlayer, EffectSubject, SpellCastFilter, SpellEffectKind,
};
use tricerules_cards::{AbilityPresentation, Keyword, Layout, TriggerCondition};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    &'static [&'static str],
    Option<(u32, u32)>,
    &'static [Keyword],
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "monastery_swiftspear",
        "Monastery Swiftspear",
        "{R}",
        &["Creature", "Human", "Monk"],
        &[],
        Some((1, 2)),
        &[Keyword::Haste],
        "24c6fe54607364423e3c142b8904d63b24ae5ead80654b683b5defad6e996476",
    ),
    (
        "dragon-style_twins",
        "Dragon-Style Twins",
        "{3}{R}{R}",
        &["Creature", "Human", "Monk"],
        &[],
        Some((3, 3)),
        &[Keyword::DoubleStrike],
        "eddee745c76825e564a3dbad8c9cf825a30c04a351d71a4c7be8d6c078a98e05",
    ),
    (
        "ringwarden_owl",
        "Ringwarden Owl",
        "{3}{U}{U}",
        &["Creature", "Bird"],
        &[],
        Some((3, 3)),
        &[Keyword::Flying],
        "e17e45657dfbef34cc4a953574449f40a69f4769484a344ba11badd7148301dc",
    ),
    (
        "whirlwind_adept",
        "Whirlwind Adept",
        "{4}{U}",
        &["Creature", "Djinn", "Monk"],
        &[],
        Some((4, 2)),
        &[Keyword::Hexproof],
        "30e7bd34ed943cac22d03c602082198dc2535f425f7babf6814214ca56f5dbc6",
    ),
    (
        "dragon_bell_monk",
        "Dragon Bell Monk",
        "{2}{W}",
        &["Creature", "Human", "Monk"],
        &[],
        Some((2, 2)),
        &[Keyword::Vigilance],
        "e8c2110ab484f1d306f8e071517f89a4b7d74067a6f555bf6f3ba844e0a98f59",
    ),
];

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
fn prowess_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, types, supertypes, body, keywords, _) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let f = face(registry, id);
        assert_eq!(f.face_id.as_str(), id.replace('-', "_"), "{id} face id");
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(
            f.types.iter().map(String::as_str).collect::<Vec<_>>(),
            types,
            "{id} types"
        );
        assert_eq!(
            f.supertypes.iter().map(String::as_str).collect::<Vec<_>>(),
            supertypes,
            "{id} supertypes"
        );
        assert_eq!(
            (f.power, f.toughness),
            body.map_or((None, None), |(p, t)| (Some(p), Some(t))),
            "{id} body"
        );
        assert_eq!(f.keywords, keywords, "{id} keywords");
        assert!(f.activated_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
        assert!(f.spell_effect.is_empty(), "{id}");
        assert!(f.targeting.is_none(), "{id}");
        assert!(f.protections.is_empty(), "{id}");

        let [ability] = f.triggered_abilities.as_slice() else {
            panic!("{id}: exactly one triggered ability");
        };
        assert_eq!(ability.ability_id.as_str(), "triggered_01", "{id}");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{id} presentation"
        );
        assert!(
            matches!(
                &ability.trigger,
                TriggerCondition::WheneverPlayerCastsSpell {
                    caster: CastTriggerPlayer::Controller,
                    filter: SpellCastFilter {
                        card_type: Some(CardTypeFilter::Noncreature),
                        ..
                    },
                    ordinal: None,
                    ..
                }
            ),
            "{id}: controller noncreature cast trigger"
        );
        assert!(
            matches!(
                ability.effect.as_slice(),
                [SpellEffectKind::PumpTarget {
                    power: 1,
                    toughness: 1,
                    subject: EffectSubject::Source,
                    ..
                }]
            ),
            "{id}: +1/+1 source pump"
        );
        assert!(ability.targeting.is_none(), "{id}");
    }
}

#[test]
fn prowess_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _, _, _, _, _, expected) in CARDS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[2], id.replace('-', "_"));
        assert_eq!(fields[4], expected, "fingerprint drift for {id}");
    }
}
