//! Registry conformance for the queue-v52 destroy-artifact-or-land cohort.
#![allow(clippy::type_complexity)]
use tricerules_cards::primitives::{
    AbilityCost, Amount, LifeAmount, PermanentTypeFilter, SpellEffectKind, TargetKind,
};
use tricerules_cards::{registry, CardDefinition, CardFace, Layout, ManaCost};

fn definition(id: &str) -> &'static CardDefinition {
    registry::global()
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
}

fn face_id(name: &str) -> String {
    name.to_lowercase().replace(' ', "_").replace('\'', "")
}

fn identity(id: &str, name: &str, mana: &str, types: &[&str]) -> &'static CardFace {
    let definition = definition(id);
    assert_eq!(definition.layout, Layout::Normal, "{id}");
    assert_eq!(registry::global().id_for_name(name), Some(id), "{id}");
    let f = definition.primary_face();
    assert_eq!(f.face_id.as_str(), face_id(name), "{id} face id");
    assert_eq!(f.name, name, "{id} name");
    assert_eq!(f.mana_cost.to_string(), mana, "{id} mana");
    assert_eq!(
        f.types.iter().map(String::as_str).collect::<Vec<_>>(),
        types,
        "{id} types"
    );
    f
}

fn artifact_or_land(filter: &tricerules_cards::primitives::TargetFilter) -> bool {
    let branches = filter.any_of.as_ref().expect("disjunctive target filter");
    branches.len() == 2
        && branches.iter().any(|branch| {
            branch.kind == TargetKind::AnyPermanent
                && branch.permanent_types == [PermanentTypeFilter::Artifact]
        })
        && branches.iter().any(|branch| {
            branch.kind == TargetKind::AnyPermanent
                && branch.permanent_types == [PermanentTypeFilter::Land]
        })
}

#[test]
fn q52_maps_definitions_and_target_unions() {
    for (id, name, mana, types, prevent, drains, cycling) in [
        (
            "demolish",
            "Demolish",
            "{3}{R}",
            &["Sorcery"][..],
            false,
            false,
            false,
        ),
        (
            "sunder_from_within",
            "Sunder from Within",
            "{2}{R}{R}",
            &["Sorcery", "Arcane"][..],
            false,
            false,
            false,
        ),
        (
            "pillage",
            "Pillage",
            "{1}{R}{R}",
            &["Sorcery"][..],
            true,
            false,
            false,
        ),
        (
            "victorious_destruction",
            "Victorious Destruction",
            "{4}{R}",
            &["Sorcery"][..],
            false,
            true,
            false,
        ),
        (
            "violent_impact",
            "Violent Impact",
            "{3}{R}",
            &["Sorcery"][..],
            false,
            false,
            true,
        ),
    ] {
        let f = identity(id, name, mana, types);
        let destroy = f.spell_effect.first().expect("destroy effect");
        match destroy {
            SpellEffectKind::Destroy { subject } => {
                assert!(!prevent, "{id}: plain destroy");
                match subject {
                    tricerules_cards::primitives::EffectSubject::Chosen(filter) => {
                        assert!(artifact_or_land(filter), "{id}: artifact or land");
                    }
                    other => panic!("{id}: unexpected subject {other:?}"),
                }
            }
            SpellEffectKind::DestroyPreventingRegeneration { subject } => {
                assert!(prevent, "{id}: regeneration prevention");
                match subject {
                    tricerules_cards::primitives::EffectSubject::Chosen(filter) => {
                        assert!(artifact_or_land(filter), "{id}: artifact or land");
                    }
                    other => panic!("{id}: unexpected subject {other:?}"),
                }
            }
            other => panic!("{id}: unexpected first effect {other:?}"),
        }
        if drains {
            assert!(
                matches!(
                    f.spell_effect.get(1),
                    Some(SpellEffectKind::LoseLife {
                        amount: LifeAmount::Fixed(1),
                        who:
                            tricerules_cards::primitives::PlayerRecipient::ControllerOfTargetGroup {
                                group_index: 0
                            },
                    })
                ),
                "{id}: controller loses 1"
            );
        } else {
            assert_eq!(f.spell_effect.len(), 1, "{id}: one effect");
        }
        if cycling {
            let ability = &f.activated_abilities[0];
            assert_eq!(ability.costs.len(), 2, "{id}: mana and discard");
            assert_eq!(
                ability.costs[0],
                AbilityCost::Mana(ManaCost::parse("{2}").unwrap()),
                "{id}"
            );
            assert_eq!(ability.costs[1], AbilityCost::DiscardSelf, "{id}");
            assert_eq!(
                ability.source_zone,
                tricerules_cards::primitives::AbilitySourceZone::Hand,
                "{id}: hand-zone cycling"
            );
            assert!(
                matches!(
                    ability.effect.as_slice(),
                    [SpellEffectKind::Draw {
                        count: Amount::Fixed(1),
                        ..
                    }]
                ),
                "{id}: draw one"
            );
        } else {
            assert!(
                f.activated_abilities.is_empty(),
                "{id}: no activated ability"
            );
        }
    }
}

#[test]
fn q52_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, digest) in [
        (
            "demolish",
            "Demolish",
            "201a63119d5504eecbbfebdb7854bb188c6f109536c85a7776e65243fada6764",
        ),
        (
            "sunder_from_within",
            "Sunder from Within",
            "201a63119d5504eecbbfebdb7854bb188c6f109536c85a7776e65243fada6764",
        ),
        (
            "pillage",
            "Pillage",
            "d0c712d44e5ac17fd6b01b34eb0b8bac2f774a7d7d2d17eb4e0d2b7c9a344289",
        ),
        (
            "victorious_destruction",
            "Victorious Destruction",
            "4b99f979941f44323c101473e5a474f09d85984b4bfcfa5646949f36b41e2e92",
        ),
        (
            "violent_impact",
            "Violent Impact",
            "7e506dd5f4c8a17bec520727d79f45d34f653e43cbcbd8fddd2e87cf65dab9e0",
        ),
    ] {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "{id}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[2], face_id(name), "face id drift for {id}");
        assert_eq!(fields[3], name);
        assert_eq!(fields[4], digest, "pinned fingerprint digest for {id}");
    }
}
