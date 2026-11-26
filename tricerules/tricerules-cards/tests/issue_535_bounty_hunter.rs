//! Registry conformance for Bounty Hunter (#535).
use tricerules_cards::primitives::{
    AbilityCost, Amount, CounterKind, EffectSubject, ObjectFilter, SpellEffectKind,
    TargetController, TargetKind,
};
use tricerules_cards::{registry, AbilityPresentation, Color, Layout};

fn single_target_filter(subject: &EffectSubject) -> &ObjectFilter {
    let EffectSubject::Chosen(filter) = subject else {
        panic!("Bounty Hunter's abilities each choose one target");
    };
    filter
}

#[test]
fn bounty_hunter_has_its_printed_identity_and_two_tap_abilities() {
    let definition = registry::global()
        .get("bounty_hunter")
        .expect("Bounty Hunter must be registered");
    assert_eq!(definition.layout, Layout::Normal);
    assert_eq!(
        registry::global().id_for_name("Bounty Hunter"),
        Some("bounty_hunter")
    );
    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), "bounty_hunter");
    assert_eq!(face.name, "Bounty Hunter");
    assert_eq!(face.mana_cost.to_string(), "{2}{B}{B}");
    assert_eq!(face.types, ["Creature", "Human", "Archer", "Minion"]);
    assert!(face.supertypes.is_empty());
    assert_eq!((face.power, face.toughness), (Some(2), Some(2)));
    assert!(face.keywords.is_empty());
    assert!(face.triggered_abilities.is_empty());
    assert!(face.static_abilities.is_empty());

    let [put_bounty, destroy] = face.activated_abilities.as_slice() else {
        panic!("Bounty Hunter has exactly two activated abilities");
    };
    for (line, ability) in [(1, put_bounty), (2, destroy)] {
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![line])
        );
        assert!(
            matches!(ability.costs.as_slice(), [AbilityCost::Tap]),
            "line {line}: the only cost is {{T}}"
        );
        assert!(
            ability.program.is_none(),
            "line {line}: primitive effect list"
        );
        let groups = &ability.targeting.as_ref().expect("targeted").groups;
        assert_eq!(groups.len(), 1, "line {line}: one target group");
        assert_eq!(
            (
                groups[0].min,
                groups[0].max,
                groups[0].effect_indices.as_slice()
            ),
            (1, 1, &[0][..]),
            "line {line}: exactly one target"
        );
    }

    let [SpellEffectKind::PutCounters {
        counter: CounterKind::Bounty,
        count: Amount::Fixed(1),
        subject,
    }] = put_bounty.effect.as_slice()
    else {
        panic!("line 1 puts one bounty counter");
    };
    let filter = single_target_filter(subject);
    assert_eq!(filter.kind, TargetKind::Creature);
    assert_eq!(filter.controller, TargetController::Any);
    assert_eq!(filter.not_color, Some(Color::Black), "nonblack creature");
    assert_eq!(filter.required_counter, None);

    let [SpellEffectKind::Destroy { subject, .. }] = destroy.effect.as_slice() else {
        panic!("line 2 destroys its target");
    };
    let filter = single_target_filter(subject);
    assert_eq!(filter.kind, TargetKind::Creature);
    assert_eq!(filter.controller, TargetController::Any);
    assert_eq!(filter.not_color, None, "any creature with a bounty counter");
    assert_eq!(filter.required_counter, Some(CounterKind::Bounty));
}

#[test]
fn bounty_hunter_fingerprint_matches_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    let row = fingerprints
        .lines()
        .find(|line| line.starts_with("bounty_hunter\t"))
        .expect("Bounty Hunter has a fingerprint row");
    assert_eq!(
        row.split('\t').collect::<Vec<_>>(),
        [
            "bounty_hunter",
            "Bounty Hunter",
            "bounty_hunter",
            "Bounty Hunter",
            "9da90c784ec2f58dad52d5751561c7f454f6d224c4c2ddd84b92fc7877e0947f",
        ]
    );
}
