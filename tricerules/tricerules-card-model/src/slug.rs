//! Card-id derivation convention for RON authoring.

/// Derives a tricerules card id from an Oracle card name: lowercase, common Latin diacritics
/// folded to ASCII, apostrophes stripped, the `//` multi-face separator dropped, and whitespace collapsed
/// to single underscores (e.g. "Pharika's Chosen" -> "pharikas_chosen",
/// "Fire // Ice" -> "fire_ice"; CR 709/712/715). Other characters (e.g. hyphens) are kept
/// verbatim, matching the generated corpus ("One-Eyed Pass" -> "one-eyed_pass").
///
/// This is an id-derivation convention for file authoring and codegen, not a wire
/// contract: decks cross IPC as Oracle names, resolved through
/// [`CardRegistry::id_for_name`](crate::CardRegistry::id_for_name), and the engine
/// reports id<->name mappings back via the `CardCatalog` event.
pub fn slugify(name: &str) -> String {
    let mut folded = String::new();
    for ch in name.to_lowercase().chars() {
        match ch {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => folded.push('a'),
            'æ' => folded.push_str("ae"),
            'ç' => folded.push('c'),
            'è' | 'é' | 'ê' | 'ë' => folded.push('e'),
            'ì' | 'í' | 'î' | 'ï' => folded.push('i'),
            'ñ' => folded.push('n'),
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' => folded.push('o'),
            'ù' | 'ú' | 'û' | 'ü' => folded.push('u'),
            'ý' | 'ÿ' => folded.push('y'),
            'œ' => folded.push_str("oe"),
            _ => folded.push(ch),
        }
    }
    folded
        .replace('\'', "")
        // Drop the `//` separator so its surrounding spaces collapse to one boundary below.
        .replace('/', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("_")
}

/// Characters that cannot appear in a portable (Windows/NTFS) file name component.
pub const RESERVED_FILE_NAME_CHARS: [char; 9] = ['"', ':', '?', '*', '<', '>', '|', '\\', '/'];

/// Derives the portable file stem for a card id's RON definition and review map: the id with
/// [`RESERVED_FILE_NAME_CHARS`] dropped. Ids keep those characters (`id == slugify(name)`), so
/// `Kongming, "Sleeping Dragon"` has id `kongming,_"sleeping_dragon"` but lives in
/// `kongming,_sleeping_dragon.ron`, and `Circle of Protection: Red` lives in
/// `circle_of_protection_red.ron`. The registry never derives identity from file names.
pub fn portable_file_stem(id: &str) -> String {
    id.chars()
        .filter(|ch| !RESERVED_FILE_NAME_CHARS.contains(ch))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{portable_file_stem, slugify};

    #[test]
    fn portable_file_stem_drops_reserved_path_characters() {
        // A quoted name keeps its quotes in the id, never in the file stem.
        assert_eq!(
            slugify("Kongming, \"Sleeping Dragon\""),
            "kongming,_\"sleeping_dragon\""
        );
        assert_eq!(
            portable_file_stem("kongming,_\"sleeping_dragon\""),
            "kongming,_sleeping_dragon"
        );
        // Matches the shipped `esper_origins_summon_esper_maduin.ron` for a colon id.
        assert_eq!(
            portable_file_stem("esper_origins_summon:_esper_maduin"),
            "esper_origins_summon_esper_maduin"
        );
        assert_eq!(portable_file_stem("continue?"), "continue");
        assert_eq!(portable_file_stem("a*b<c>d|e\\f/g"), "abcdefg");
        // Portable ids, including commas, hyphens and `!`, are unchanged.
        for id in [
            "lightning_bolt",
            "ajani,_sleeper_agent",
            "one-eyed_pass",
            "yip_yip!",
        ] {
            assert_eq!(portable_file_stem(id), id);
        }
    }

    #[test]
    fn slugify_examples() {
        assert_eq!(slugify("Lightning Bolt"), "lightning_bolt");
        assert_eq!(slugify("Pharika's Chosen"), "pharikas_chosen");
        assert_eq!(slugify("Island"), "island");
        assert_eq!(slugify("Óin the Brave"), "oin_the_brave");
        // Multi-face `//` names collapse the separator (CR 709/712/715).
        assert_eq!(slugify("Fire // Ice"), "fire_ice");
        assert_eq!(
            slugify("Bonecrusher Giant // Stomp"),
            "bonecrusher_giant_stomp"
        );
    }
}
