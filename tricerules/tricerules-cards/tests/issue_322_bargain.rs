use tricerules_cards::primitives::{
    CastCostOptionDef, GameCondition, ObjectCastCostKind, SpellEffectKind,
};
use tricerules_cards::CardRegistry;

#[test]
fn bargain_self_entry_condition_deserializes_as_typed_rules_data() {
    let condition: GameCondition = ron::from_str("SelfWasBargained")
        .expect("self-entry bargain conditions should be authorable in card RON");
    assert_eq!(ron::to_string(&condition).unwrap(), "SelfWasBargained");
}

#[test]
fn bargain_is_a_typed_object_paid_cast_cost_kind() {
    let kind: ObjectCastCostKind =
        ron::from_str("Bargain").expect("cast-cost receipts should preserve the Bargain identity");
    assert_eq!(ron::to_string(&kind).unwrap(), "Bargain");
}

#[test]
fn the_four_complete_bargain_cards_register_with_optional_receipts_and_shared_targets() {
    let registry = CardRegistry::from_chunks_and_tokens(
        &[
            include_str!("../data/archons_glory.ron"),
            include_str!("../data/candy_grapple.ron"),
            include_str!("../data/kellans_lightblades.ron"),
            include_str!("../data/troublemaker_ouphe.ron"),
        ],
        &[],
    )
    .expect("the reviewed Bargain card definitions should register");

    for id in [
        "archons_glory",
        "candy_grapple",
        "kellans_lightblades",
        "troublemaker_ouphe",
    ] {
        let face = registry.get(id).unwrap().primary_face();
        let [group] = face.cast_cost_groups.as_slice() else {
            panic!("{id} should have one Bargain option group");
        };
        assert_eq!((group.min, group.max), (0, 1), "{id}");
        let [CastCostOptionDef::SacrificePermanent {
            kind: ObjectCastCostKind::Bargain,
            filter,
            ..
        }] = group.options.as_slice()
        else {
            panic!("{id} should use the Bargain sacrifice filter");
        };
        assert_eq!(filter.any_of.as_ref().unwrap().len(), 2, "{id}");
        if id == "troublemaker_ouphe" {
            assert!(face.triggered_abilities.iter().any(|ability| {
                ability.intervening_if == Some(GameCondition::SelfWasBargained)
            }));
        } else {
            assert!(
                face.spell_effect
                    .iter()
                    .any(|effect| matches!(effect, SpellEffectKind::ConditionalCastCost { .. })),
                "{id} should resolve a Bargain-linked effect"
            );
        }
    }
}

#[test]
fn bargain_cards_match_the_reviewed_oracle_identities_and_costs() {
    for (id, name, mana_cost, primary_type) in [
        ("archons_glory", "Archon's Glory", "{W}", "Instant"),
        ("candy_grapple", "Candy Grapple", "{1}{B}", "Instant"),
        (
            "kellans_lightblades",
            "Kellan's Lightblades",
            "{1}{W}",
            "Instant",
        ),
        (
            "troublemaker_ouphe",
            "Troublemaker Ouphe",
            "{1}{G}",
            "Creature",
        ),
    ] {
        let card = CardRegistry::global()
            .get(id)
            .unwrap_or_else(|| panic!("missing reviewed card {id}"));
        let face = card.primary_face();
        assert_eq!(card.name, name, "{id}");
        assert_eq!(face.mana_cost.to_string(), mana_cost, "{id}");
        assert_eq!(
            face.types.first().map(String::as_str),
            Some(primary_type),
            "{id}"
        );
    }
}

#[test]
fn bargain_card_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, expected) in [
        (
            "archons_glory",
            "Archon's Glory",
            "76f335ddafad86268ff5004209f7d92162a11c846292478ba6a259649792c960",
        ),
        (
            "candy_grapple",
            "Candy Grapple",
            "971f3ebe9da270543c3634d167256da78a78df0e3b28705ec138069908951bf2",
        ),
        (
            "kellans_lightblades",
            "Kellan's Lightblades",
            "938e351617101a652dd9fce0407da5f75966f7ff5b56ce82b06b13ce93bd227f",
        ),
        (
            "troublemaker_ouphe",
            "Troublemaker Ouphe",
            "ccd61c42ce555f4b57a55cb910cfc16d262e03108bda2fa334903b0862e49d50",
        ),
    ] {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[1], name);
        assert_eq!(fields[4], expected, "fingerprint drift for {id}");
    }
}

#[test]
fn bargain_rejects_nonmatching_sacrifice_filters() {
    let malformed = include_str!("../data/archons_glory.ron").replace(
        "permanent_types: [Artifact, Enchantment]",
        "permanent_types: [Creature]",
    );
    assert_ne!(
        malformed,
        include_str!("../data/archons_glory.ron"),
        "the negative fixture must change the Bargain filter"
    );
    assert!(CardRegistry::from_chunks_and_tokens(&[malformed.as_str()], &[]).is_err());
}
