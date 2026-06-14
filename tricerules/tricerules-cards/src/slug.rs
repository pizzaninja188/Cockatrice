//! Card-id derivation convention for RON authoring.

/// Derives a tricerules card id from an Oracle card name: lowercase, ASCII apostrophes
/// stripped, the `//` multi-face separator dropped, and remaining whitespace runs collapsed
/// to single underscores (e.g. "Pharika's Chosen" -> "pharikas_chosen",
/// "Fire // Ice" -> "fire_ice"; CR 709/712/715). Other characters (e.g. hyphens) are kept
/// verbatim, matching the generated corpus ("One-Eyed Pass" -> "one-eyed_pass").
///
/// This is an id-derivation convention for file authoring and codegen, not a wire
/// contract: decks cross IPC as Oracle names, resolved through
/// [`CardRegistry::id_for_name`](crate::CardRegistry::id_for_name), and the engine
/// reports id<->name mappings back via the `CardCatalog` event.
pub fn slugify(name: &str) -> String {
    name.to_lowercase()
        .replace('\'', "")
        // Drop the `//` separator so its surrounding spaces collapse to one boundary below.
        .replace('/', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("_")
}

#[cfg(test)]
mod tests {
    use super::slugify;

    #[test]
    fn slugify_examples() {
        assert_eq!(slugify("Lightning Bolt"), "lightning_bolt");
        assert_eq!(slugify("Pharika's Chosen"), "pharikas_chosen");
        assert_eq!(slugify("Island"), "island");
        // Multi-face `//` names collapse the separator (CR 709/712/715).
        assert_eq!(slugify("Fire // Ice"), "fire_ice");
        assert_eq!(
            slugify("Bonecrusher Giant // Stomp"),
            "bonecrusher_giant_stomp"
        );
    }
}
