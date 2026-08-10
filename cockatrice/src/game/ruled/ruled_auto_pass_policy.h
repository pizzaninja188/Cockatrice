/**
 * @file ruled_auto_pass_policy.h
 * @brief Pure mapping from Cockatrice's phase-toolbar stops to the ruled protocol policy.
 */

#ifndef COCKATRICE_RULED_AUTO_PASS_POLICY_H
#define COCKATRICE_RULED_AUTO_PASS_POLICY_H

#include <array>
#include <libcockatrice/protocol/pb/ruled_v1.pb.h>

namespace RuledAutoPassPolicy
{
inline ruled::v1::SetAutoPassPolicy fromToolbarStops(const std::array<bool, 11> &ownTurn,
                                                     const std::array<bool, 11> &opponentTurn)
{
    ruled::v1::SetAutoPassPolicy policy;
    auto append = [](const std::array<bool, 11> &stops,
                     google::protobuf::RepeatedField<int> *phases) {
        const auto add = [phases](ruled::v1::PhaseId phase) { phases->Add(static_cast<int>(phase)); };
        if (stops[1])
            add(ruled::v1::PHASE_ID_UPKEEP);
        if (stops[2])
            add(ruled::v1::PHASE_ID_DRAW);
        if (stops[3])
            add(ruled::v1::PHASE_ID_MAIN1);
        if (stops[4])
            add(ruled::v1::PHASE_ID_BEGIN_COMBAT);
        if (stops[5])
            add(ruled::v1::PHASE_ID_DECLARE_ATTACKERS);
        if (stops[6])
            add(ruled::v1::PHASE_ID_DECLARE_BLOCKERS);
        if (stops[7]) {
            // One toolbar stop covers both CR 510.4 combat-damage steps.
            add(ruled::v1::PHASE_ID_FIRST_STRIKE_DAMAGE);
            add(ruled::v1::PHASE_ID_COMBAT_DAMAGE);
        }
        if (stops[8])
            add(ruled::v1::PHASE_ID_END_COMBAT);
        if (stops[9])
            add(ruled::v1::PHASE_ID_MAIN2);
        if (stops[10])
            add(ruled::v1::PHASE_ID_END_STEP);
    };
    append(ownTurn, policy.mutable_stop_on_own_turn());
    append(opponentTurn, policy.mutable_stop_on_opponent_turn());
    return policy;
}
} // namespace RuledAutoPassPolicy

#endif // COCKATRICE_RULED_AUTO_PASS_POLICY_H
