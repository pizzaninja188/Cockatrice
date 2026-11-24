//! Registry conformance for Run Away Together (#485) and the typed target-group relation it
//! shares with the "from a single graveyard" cards. CR 115.1a/601.2c govern the relation; the
//! printed Oracle values are independent expectations.
use tricerules_cards::primitives::{
    EffectSubject, SpellEffectKind, TargetChooser, TargetGroupRelation, TargetKind, TargetSchema,
};
use tricerules_cards::{CardDefinition, Layout};

fn definition(id: &str) -> &'static CardDefinition {
    tricerules_cards::registry::global()
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
}

#[test]
fn issue_485_run_away_together_maps_two_creatures_controlled_by_different_players() {
    let definition = definition("run_away_together");
    assert_eq!(definition.layout, Layout::Normal);
    assert_eq!(
        tricerules_cards::registry::global().id_for_name("Run Away Together"),
        Some("run_away_together")
    );
    let face = definition.primary_face();
    assert_eq!(face.name, "Run Away Together");
    assert_eq!(face.mana_cost.to_string(), "{1}{U}");
    assert_eq!(face.types, ["Instant"]);
    assert!(matches!(
        face.spell_effect.as_slice(),
        [SpellEffectKind::ReturnToOwnersHand { subject: EffectSubject::Chosen(filter) }]
            if filter.kind == TargetKind::Creature
                && **filter == tricerules_cards::primitives::TargetFilter::default_creature()
    ));
    let targeting = face.targeting.as_ref().expect("targeting");
    let [group] = targeting.groups.as_slice() else {
        panic!("one target group returns both creatures in one instruction");
    };
    assert_eq!((group.min, group.max), (2, 2));
    assert_eq!(group.chooser, TargetChooser::Controller);
    assert_eq!(
        group.relation,
        Some(TargetGroupRelation::DifferentControllers)
    );
    assert!(group.distinct_from.is_empty());
    let schema = TargetSchema::compile(&face.spell_effect, Some(targeting)).unwrap();
    assert_eq!(
        schema.groups[0].prompt,
        "Choose 2 target creatures controlled by different players"
    );
}

#[test]
fn issue_485_single_graveyard_cards_keep_their_relation_after_migration() {
    let registry = tricerules_cards::registry::global();
    for id in [
        "arashin_sunshield",
        "griffnaut_tracker",
        "qutrub_forayer",
        "soul-shackled_zombie",
    ] {
        let definition = definition(id);
        let mut relations = Vec::new();
        let mut collect = |targeting: Option<&tricerules_cards::primitives::TargetingDef>| {
            relations.extend(
                targeting
                    .into_iter()
                    .flat_map(|targeting| &targeting.groups)
                    .filter_map(|group| group.relation),
            );
        };
        for face in definition.faces_iter() {
            collect(face.targeting.as_ref());
            for ability in &face.triggered_abilities {
                collect(ability.targeting.as_ref());
                if let Some(modal) = &ability.modal {
                    for mode in &modal.modes {
                        collect(mode.targeting.as_ref());
                    }
                }
            }
            for ability in &face.activated_abilities {
                collect(ability.targeting.as_ref());
            }
        }
        assert_eq!(
            relations,
            [TargetGroupRelation::SameGraveyard],
            "{id} keeps exactly one single-graveyard group"
        );
        assert!(registry.get(id).is_some());
    }
}

#[test]
fn issue_485_fingerprint_matches_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    let row = fingerprints
        .lines()
        .find(|line| line.starts_with("run_away_together\t"))
        .expect("fingerprint row");
    let fields: Vec<&str> = row.split('\t').collect();
    assert_eq!(
        fields[..4],
        [
            "run_away_together",
            "Run Away Together",
            "run_away_together",
            "Run Away Together"
        ]
    );
    assert_eq!(fields[4].len(), 64);
    assert!(fields[4].chars().all(|ch| ch.is_ascii_hexdigit()));
}
