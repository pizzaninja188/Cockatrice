#ifndef RULED_CHOICE_METADATA_H
#define RULED_CHOICE_METADATA_H

#include <libcockatrice/protocol/pb/ruled_v1.pb.h>
#include <algorithm>
#include <unordered_set>

// Structural validation only. The engine owns candidate identity and selection legality.
inline bool ruledTokenChoiceMetadataValid(const ruled::v1::ResolutionChoiceRequired &choice)
{
    const int count = choice.candidate_object_ids_size();
    const bool entryOrder = choice.choice_kind() == ruled::v1::CHOICE_KIND_SIMULTANEOUS_ENTRY_ORDER;
    if (entryOrder && (count < 2 || !choice.ordered() || choice.min() != static_cast<unsigned>(count) ||
                       choice.max() != choice.min() ||
                       choice.candidate_names_size() != count || choice.candidate_card_ids_size() != count))
        return false;
    if (entryOrder) {
        std::unordered_set<unsigned> ids;
        for (const auto id : choice.candidate_object_ids())
            if (id == 0 || !ids.insert(id).second)
                return false;

        // A public reveal may contain a larger cohort than the graveyard subset whose order is
        // being chosen (for example Gifts Ungiven after its partition). Bind every order
        // candidate to exactly one card in that immutable receipt; do not require equal counts.
        if (choice.has_public_reveal()) {
            const auto &reveal = choice.public_reveal();
            if (reveal.reveal_id().empty() || reveal.cards_size() == 0)
                return false;
            std::unordered_set<unsigned> revealIds;
            for (const auto &card : reveal.cards()) {
                if (card.object_id() == 0 || card.card_id().empty() || card.card_name().empty() ||
                    !revealIds.insert(card.object_id()).second) {
                    return false;
                }
            }
            for (int candidateIndex = 0; candidateIndex < count; ++candidateIndex) {
                int matches = 0;
                for (const auto &card : reveal.cards()) {
                    if (card.object_id() == choice.candidate_object_ids(candidateIndex) &&
                        card.card_id() == choice.candidate_card_ids(candidateIndex) &&
                        card.card_name() == choice.candidate_names(candidateIndex)) {
                        ++matches;
                    }
                }
                if (matches != 1)
                    return false;
            }
        }
    }
    if (choice.candidate_token_identities_size() == 0)
        return true;
    if (!entryOrder || choice.candidate_token_identities_size() != count)
        return false;
    for (int i = 0; i < count; ++i) {
        const auto &identity = choice.candidate_token_identities(i);
        if (identity.ByteSizeLong() == 0)
            continue; // An ordinary card in a mixed cohort.
        const bool creature = std::find(identity.types().begin(), identity.types().end(), "Creature") !=
                              identity.types().end();
        if (identity.name().empty() || identity.name() != choice.candidate_names(i) || identity.types_size() == 0 ||
            identity.is_creature() != creature || (creature && identity.pt().empty()))
            return false;
    }
    return true;
}

#endif
