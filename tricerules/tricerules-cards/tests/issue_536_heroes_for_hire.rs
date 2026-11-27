//! Registry conformance for Heroes for Hire (#536).
use tricerules_cards::primitives::{
    AbilityCost, Amount, ExilePlayPermissionDuration, PlayerRecipient, SpellEffectKind, TargetKind,
};
use tricerules_cards::{registry, AbilityPresentation, Layout};

#[test]
fn heroes_for_hire_has_its_printed_identity_and_abilities() {
    let definition = registry::global()
        .get("heroes_for_hire")
        .expect("Heroes for Hire must be registered");
    assert_eq!(definition.layout, Layout::Normal);
    assert_eq!(
        registry::global().id_for_name("Heroes for Hire"),
        Some("heroes_for_hire")
    );
    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), "heroes_for_hire");
    assert_eq!(face.name, "Heroes for Hire");
    assert_eq!(face.mana_cost.to_string(), "{3}{R}{R}");
    assert_eq!(face.types, ["Enchantment"]);
    assert!(face.supertypes.is_empty());
    assert_eq!((face.power, face.toughness), (None, None));
    assert!(face.keywords.is_empty());
    assert!(face.static_abilities.is_empty());

    let [enters] = face.triggered_abilities.as_slice() else {
        panic!("Heroes for Hire has exactly one triggered ability");
    };
    assert_eq!(
        enters.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    let [SpellEffectKind::CreateTokens { token, count, .. }] = enters.effect.as_slice() else {
        panic!("the enters trigger creates tokens");
    };
    assert_eq!(token, "treasure");
    assert_eq!(*count, Amount::Fixed(3));

    let [exile_top] = face.activated_abilities.as_slice() else {
        panic!("Heroes for Hire has exactly one activated ability");
    };
    assert_eq!(
        exile_top.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    let [AbilityCost::SacrificePermanent { filter, count }] = exile_top.costs.as_slice() else {
        panic!("the only cost is sacrificing a permanent");
    };
    assert_eq!(*count, 1, "a Treasure");
    assert_eq!(filter.kind, TargetKind::AnyPermanent);
    assert_eq!(filter.required_subtypes, ["Treasure"]);
    assert!(exile_top.targeting.is_none(), "the ability has no target");
    assert!(exile_top.program.is_none(), "primitive effect list");
    let [SpellEffectKind::ExileTopWithPlayPermission {
        player: PlayerRecipient::Controller,
        count: 1,
        count_by_cast_cost: None,
        duration: ExilePlayPermissionDuration::ThisTurn,
    }] = exile_top.effect.as_slice()
    else {
        panic!("line 2 exiles the top card with a this-turn play permission");
    };
}

#[test]
fn heroes_for_hire_fingerprint_matches_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    let row = fingerprints
        .lines()
        .find(|line| line.starts_with("heroes_for_hire\t"))
        .expect("Heroes for Hire has a fingerprint row");
    assert_eq!(
        row.split('\t').collect::<Vec<_>>(),
        [
            "heroes_for_hire",
            "Heroes for Hire",
            "heroes_for_hire",
            "Heroes for Hire",
            "09a21a01e43153bcf5e64644ba86b38bb2830e136950ac7a7d10aa3be59f36c4",
        ]
    );
}
