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
    if (entryOrder && (count < 2 || !choice.ordered() || choice.has_public_reveal() ||
                       choice.min() != static_cast<unsigned>(count) || choice.max() != choice.min() ||
                       choice.candidate_names_size() != count || choice.candidate_card_ids_size() != count))
        return false;
    if (entryOrder) {
        std::unordered_set<unsigned> ids;
        for (const auto id : choice.candidate_object_ids())
            if (id == 0 || !ids.insert(id).second)
                return false;
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
