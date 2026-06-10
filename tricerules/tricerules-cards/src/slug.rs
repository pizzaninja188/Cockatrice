//! Card-id derivation convention for RON authoring.

/// Derives a tricerules card id from an Oracle card name: lowercase, ASCII apostrophes
/// stripped, spaces to underscores (e.g. "Pharika's Chosen" -> "pharikas_chosen").
///
/// This is an id-derivation convention for file authoring and codegen, not a wire
/// contract: decks cross IPC as Oracle names, resolved through
/// [`CardRegistry::id_for_name`](crate::CardRegistry::id_for_name), and the engine
/// reports id<->name mappings back via the `CardCatalog` event.
pub fn slugify(name: &str) -> String {
    name.to_lowercase().replace('\'', "").replace(' ', "_")
}

#[cfg(test)]
mod tests {
    use super::slugify;

    #[test]
    fn slugify_examples() {
        assert_eq!(slugify("Lightning Bolt"), "lightning_bolt");
        assert_eq!(slugify("Pharika's Chosen"), "pharikas_chosen");
        assert_eq!(slugify("Island"), "island");
    }
}
