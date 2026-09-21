//! Registry conformance for the issue #338/#376 reviewed direct-RON batch: Drag to the Roots and
//! My Precious // Allure of Power.
//!
//! Drag to the Roots (`ef4c478c-7019-4ec0-8edf-2a3078a8e97a`) and My Precious // Allure of Power
//! (`2e728381-6db0-4c66-883d-82d718fef833`) were promoted after complete-definition review against
//! the pinned Scryfall snapshot. The exact records and `rulings_uri` were fetched 2026-09-21.
//! Governance: CR 601.2b/f-h (announced additional costs and total cost), 701.4 (behold),
//! 701.7 (destroy), 700.2/608.2h (delirium card types), 301.5/702.6 (Equipment and equip),
//! 702.18 (hexproof), 509.1b (combat restrictions), and 601.3e/715 (adventure alternative
//! characteristics).

use tricerules_cards::primitives::{
    AbilityCost, Amount, CastCostOptionDef, EffectSubject, GameCondition, GraveyardAggregate,
    ObjectCastCostKind, PermanentTypeFilter, PlayerRecipient, RelativePlayerSet, SpellCostModifier,
    SpellEffectKind, StaticAbilityDef, TargetController, TargetFilter, TargetKind,
};
use tricerules_cards::{CardRegistry, Layout};

const DRAG_TO_THE_ROOTS_FINGERPRINT: &str =
    "d403de0ae46693ec080e2b5305233a88ea7ae5bff77110c8ffcce0bdec3ffba0";
const MY_PRECIOUS_FINGERPRINT: &str =
    "462d50becaf64869943d123f3a319b30d52e035afb26733eb9fbca08f267b2a8";
const ALLURE_OF_POWER_FINGERPRINT: &str =
    "c35fcea4edc49b0a5e42bbaec5073eb29a951b12fda9f6f99cf0bc35e4b68872";

#[test]
fn issue_338_direct_ron_batch3_maps_definitions() {
    let registry = CardRegistry::global();

    // Drag to the Roots: delirium generic reduction and nonland destroy.
    let drag = registry.get("drag_to_the_roots").expect("registered");
    assert_eq!(drag.name, "Drag to the Roots");
    assert_eq!(
        registry.id_for_name("Drag to the Roots"),
        Some("drag_to_the_roots")
    );
    assert_eq!(drag.layout, Layout::Normal);
    let face = drag.primary_face();
    assert_eq!(face.face_id.as_str(), "drag_to_the_roots");
    assert_eq!(face.mana_cost.to_string(), "{2}{B}{G}");
    assert_eq!(face.types, ["Instant"]);
    assert_eq!(
        face.cost_modifiers,
        [SpellCostModifier::ConditionalGenericReduction {
            amount: 2,
            condition: GameCondition::GraveyardAggregate {
                owners: RelativePlayerSet::Controller,
                aggregate: GraveyardAggregate::DistinctCardTypes,
                filter: None,
                min: Some(4),
                max: None,
            },
        }]
    );
    assert_eq!(
        face.spell_effect,
        [SpellEffectKind::Destroy {
            subject: EffectSubject::Chosen(Box::new(TargetFilter {
                kind: TargetKind::AnyPermanent,
                excluded_permanent_types: vec![PermanentTypeFilter::Land],
                ..TargetFilter::default()
            })),
        }]
    );
    let [group] = face
        .targeting
        .as_ref()
        .expect("Drag to the Roots targets")
        .groups
        .as_slice()
    else {
        panic!("Drag to the Roots has one target group");
    };
    assert_eq!(group.prompt, "Choose target nonland permanent");
    assert_eq!(group.effect_indices, [0]);

    // My Precious // Allure of Power: Adventure, attached hexproof/unblockable, equip with life.
    let precious = registry
        .get("my_precious_allure_of_power")
        .expect("registered");
    assert_eq!(precious.name, "My Precious // Allure of Power");
    assert_eq!(precious.layout, Layout::Adventure);
    assert_eq!(precious.face_count(), 2);

    let main = &precious.faces[0];
    assert_eq!(main.face_id.as_str(), "my_precious");
    assert_eq!(main.mana_cost.to_string(), "{3}");
    assert_eq!(main.types, ["Artifact", "Equipment"]);
    assert!(main.supertypes.iter().any(|s| s == "Legendary"));
    let [static_ability] = main.static_abilities.as_slice() else {
        panic!("My Precious has one static ability");
    };
    let StaticAbilityDef::AttachedModifier {
        keywords,
        restriction,
        ..
    } = &static_ability.definition
    else {
        panic!("My Precious grants an attached modifier");
    };
    assert_eq!(keywords, &[tricerules_cards::Keyword::Hexproof]);
    assert!(restriction.cant_be_blocked);
    let [equip] = main.activated_abilities.as_slice() else {
        panic!("My Precious has one equip ability");
    };
    assert_eq!(equip.costs.len(), 2);
    match &equip.costs[0] {
        AbilityCost::Mana(cost) => assert_eq!(cost.to_string(), "{2}"),
        other => panic!("expected a mana equip cost, got {other:?}"),
    }
    assert_eq!(equip.costs[1], AbilityCost::PayLife { amount: 2 });
    assert_eq!(
        equip.effect,
        [SpellEffectKind::Equip {
            target: TargetFilter {
                kind: TargetKind::Creature,
                controller: TargetController::You,
                ..TargetFilter::default()
            },
        }]
    );

    let adventure = &precious.faces[1];
    assert_eq!(adventure.face_id.as_str(), "allure_of_power");
    assert_eq!(adventure.mana_cost.to_string(), "{1}{B}");
    assert_eq!(adventure.types, ["Instant", "Adventure"]);
    let [group] = adventure.cast_cost_groups.as_slice() else {
        panic!("Allure of Power has one cast-cost group");
    };
    assert_eq!((group.min, group.max), (1, 1));
    let CastCostOptionDef::SacrificePermanent {
        option_id,
        filter,
        kind,
        ..
    } = &group.options[0]
    else {
        panic!("Allure of Power sacrifices a creature");
    };
    assert_eq!(option_id.as_str(), "sacrifice_creature");
    assert_eq!(*kind, ObjectCastCostKind::AdditionalPayment);
    assert_eq!(
        filter.as_ref(),
        &TargetFilter {
            kind: TargetKind::Creature,
            controller: TargetController::You,
            ..TargetFilter::default()
        }
    );
    assert_eq!(
        adventure.spell_effect,
        [SpellEffectKind::Draw {
            who: PlayerRecipient::Controller,
            count: Amount::Fixed(2),
        }]
    );
}

#[test]
fn issue_338_direct_ron_batch3_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");

    let row = fingerprints
        .lines()
        .find(|line| line.starts_with("drag_to_the_roots\t"))
        .expect("missing fingerprint row for drag_to_the_roots");
    let fields: Vec<&str> = row.split('\t').collect();
    assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
    assert_eq!(fields[0], "drag_to_the_roots");
    assert_eq!(fields[1], "Drag to the Roots");
    assert_eq!(
        fields[4], DRAG_TO_THE_ROOTS_FINGERPRINT,
        "fingerprint drift for drag_to_the_roots"
    );

    // The Adventure card has one fingerprint row per face.
    let rows: Vec<Vec<&str>> = fingerprints
        .lines()
        .filter(|line| line.starts_with("my_precious_allure_of_power\t"))
        .map(|line| line.split('\t').collect())
        .collect();
    assert_eq!(rows.len(), 2, "one fingerprint row per face");
    for fields in &rows {
        assert_eq!(fields.len(), 5, "fingerprint row shape: {fields:?}");
        assert_eq!(fields[1], "My Precious // Allure of Power");
        let expected = match fields[2] {
            "my_precious" => MY_PRECIOUS_FINGERPRINT,
            "allure_of_power" => ALLURE_OF_POWER_FINGERPRINT,
            other => panic!("unexpected face id {other}"),
        };
        assert_eq!(fields[4], expected, "fingerprint drift for {}", fields[2]);
    }
    let mut seen: Vec<&str> = rows.iter().map(|fields| fields[2]).collect();
    seen.sort_unstable();
    assert_eq!(seen, ["allure_of_power", "my_precious"]);
}
