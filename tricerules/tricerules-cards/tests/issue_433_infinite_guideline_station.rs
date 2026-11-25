//! Registry conformance for Infinite Guideline Station (#433).
//!
//! Both counted clauses use the same "multicolored permanent you control" filter: two or more
//! distinct derived colors (CR 105.2b) among permanents the resolving controller controls. The
//! attack trigger is printed in the 12+ striation, so it is granted by that station symbol's
//! static ability (CR 722.2). The fingerprint is SHA-256 of the exact pinned Oracle lines.

use tricerules_cards::primitives::{
    Amount, BattlefieldPermanentFilter, CountExpression, GameCondition, PermanentTypeFilter,
    RelativePlayerSet, SpellEffectKind, StaticAbilityDef, TriggerCondition,
};
use tricerules_cards::{AbilityPresentation, CounterKind, Keyword, Layout};

const ID: &str = "infinite_guideline_station";
const NAME: &str = "Infinite Guideline Station";
const FINGERPRINT: &str = "4ab14b01e16f40fa6b093d5a934ae4b1fd5ffe477dd9900f9fab5b8ef41aaf2a";

fn multicolored_you_control() -> Amount {
    Amount::Count(CountExpression::BattlefieldPermanents {
        filter: BattlefieldPermanentFilter {
            any_of: None,
            controllers: RelativePlayerSet::Controller,
            exclude_source: false,
            matching: Box::new(tricerules_cards::primitives::ObjectFilter {
                kind: tricerules_cards::primitives::TargetKind::AnyPermanent,
                min_colors: Some(2),
                ..tricerules_cards::primitives::ObjectFilter::default()
            }),
        },
    })
}

#[test]
fn issue_433_infinite_guideline_station_maps_definition() {
    let registry = tricerules_cards::registry::global();
    let definition = registry.get(ID).expect("Infinite Guideline Station");
    assert_eq!(definition.layout, Layout::Normal);
    assert_eq!(registry.id_for_name(NAME), Some(ID));
    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), ID);
    assert_eq!(face.name, NAME);
    assert_eq!(face.mana_cost.to_string(), "{W}{U}{B}{R}{G}");
    assert_eq!(face.types, ["Artifact", "Spacecraft"]);
    assert_eq!(face.supertypes, ["Legendary"]);
    assert_eq!((face.power, face.toughness), (Some(7), Some(15)));
    assert!(face.keywords.is_empty(), "Flying is only granted at 12+");
    assert!(face.spell_effect.is_empty());

    let [entry] = face.triggered_abilities.as_slice() else {
        panic!("one printed entry trigger");
    };
    assert_eq!(entry.ability_id.as_str(), "triggered_01");
    assert_eq!(
        entry.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(entry.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert!(entry.targeting.is_none());
    assert!(
        matches!(
            entry.effect.as_slice(),
            [SpellEffectKind::CreateTokens { token, count, tapped: true, sacrifice_timing: None, .. }]
                if token == "robot_c_2_2" && *count == multicolored_you_control()
        ),
        "a tapped Robot for each multicolored permanent you control: {:?}",
        entry.effect
    );

    let [station] = face.activated_abilities.as_slice() else {
        panic!("one Station ability");
    };
    assert_eq!(
        station.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert!(matches!(
        station.effect.as_slice(),
        [SpellEffectKind::PutCounters {
            counter: CounterKind::Charge,
            ..
        }]
    ));

    let [threshold] = face.static_abilities.as_slice() else {
        panic!("one 12+ station symbol");
    };
    assert_eq!(
        threshold.presentation,
        AbilityPresentation::OracleLines(vec![3, 4])
    );
    let StaticAbilityDef::ConditionalSelfModifier {
        condition,
        add_types,
        base_power,
        base_toughness,
        keywords,
        triggered_abilities,
        activated_abilities,
        ..
    } = &threshold.definition
    else {
        panic!(
            "12+ is a conditional self-modifier: {:?}",
            threshold.definition
        );
    };
    assert_eq!(
        *condition,
        GameCondition::SourceCounterCount {
            counter: CounterKind::Charge,
            min: Some(12),
            max: None,
        }
    );
    assert_eq!(add_types.card_types, [PermanentTypeFilter::Creature]);
    assert_eq!((*base_power, *base_toughness), (Some(7), Some(15)));
    assert_eq!(keywords, &[Keyword::Flying]);
    assert!(activated_abilities.is_empty());
    let [attack] = triggered_abilities.as_slice() else {
        panic!("one 12+ attack trigger");
    };
    assert_eq!(
        attack.presentation,
        AbilityPresentation::OracleLines(vec![4])
    );
    assert!(matches!(
        attack.trigger,
        TriggerCondition::WheneverSelfAttacks {
            minimum_other_attackers: 0
        }
    ));
    assert!(
        matches!(
            attack.effect.as_slice(),
            [SpellEffectKind::Draw { count, .. }] if *count == multicolored_you_control()
        ),
        "draw a card for each multicolored permanent you control: {:?}",
        attack.effect
    );
}

#[test]
fn issue_433_infinite_guideline_station_fingerprint_matches_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    let row = fingerprints
        .lines()
        .find(|line| line.starts_with(&format!("{ID}\t")))
        .expect("fingerprint row");
    let fields: Vec<&str> = row.split('\t').collect();
    assert_eq!(fields, [ID, NAME, ID, NAME, FINGERPRINT]);
}
