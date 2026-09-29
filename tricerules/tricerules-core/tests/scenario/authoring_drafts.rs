use super::helpers::{advance_to_main1_from_game_start, authoring_rows::run_rows};
use serde::Deserialize;
use std::path::PathBuf;
use tricerules_cards::CardRegistry;
use tricerules_core::{EngineDeck, GameEngine};

#[test]
fn draft_constructor_is_explicit_and_preserves_production_registry() {
    let draft = r#"(id:"authoring_test_land",name:"Authoring Test Land",face_id:"authoring_test_land",types:["Land"] )"#;
    let registry = Box::leak(Box::new(CardRegistry::from_authoring_draft(draft).unwrap()));
    let decks = vec![
        EngineDeck {
            mainboard: vec!["authoring_test_land".into(); 20],
            commanders: vec![]
        };
        2
    ];
    let engine =
        GameEngine::new_for_authoring(1, &[0, 1], 20, Some(decks), true, registry).unwrap();
    assert_eq!(engine.state.players[0].hand.len(), 7);
    assert!(CardRegistry::global().get("authoring_test_land").is_none());
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DraftBatch {
    drafts: Vec<PathBuf>,
    rows: serde_json::Value,
}

/// Explicit opt-in. Production and the default suite never read this environment variable.
#[test]
#[ignore = "offline drafts; invoke with scripts/test-card-drafts.ps1"]
fn external_drafts() {
    let manifest = PathBuf::from(
        std::env::var_os("TRICERULES_AUTHORING_BATCH").expect("explicit draft batch path"),
    );
    let mut batch: DraftBatch =
        serde_json::from_slice(&std::fs::read(&manifest).unwrap()).expect("draft batch manifest");
    assert!(!batch.drafts.is_empty(), "empty draft batch");
    for path in &mut batch.drafts {
        if path.is_relative() {
            *path = manifest.parent().unwrap().join(&path);
        }
    }
    let data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tricerules-cards/data");
    let registry = Box::leak(Box::new(
        tricerules_cards::authoring::load_registry(&data, &batch.drafts)
            .expect("validated fresh draft registry"),
    ));
    let exercised = run_rows(&batch.rows.to_string(), || {
        let decks = vec![
            EngineDeck {
                mainboard: vec!["island".into(); 40],
                commanders: vec![]
            };
            2
        ];
        let mut e =
            GameEngine::new_for_authoring(448_012, &[0, 1], 20, Some(decks), true, registry)
                .unwrap();
        advance_to_main1_from_game_start(&mut e);
        e
    });
    for path in &batch.drafts {
        let raw =
            tricerules_cards::authoring::raw_card(&std::fs::read_to_string(path).unwrap()).unwrap();
        assert!(
            exercised.contains(&raw.id),
            "draft {} has no executed semantic row",
            raw.id
        );
    }
}
