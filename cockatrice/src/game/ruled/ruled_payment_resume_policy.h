#ifndef RULED_PAYMENT_RESUME_POLICY_H
#define RULED_PAYMENT_RESUME_POLICY_H

#include "ruled_client_state.h"

#include <string>

/// Identity of the ResolutionPayment offer saved with a suspended local payment.
/// `answerSemantics` comes from RuledClientState's normalized resolution-choice fingerprint, which
/// deliberately excludes readiness while retaining the answers the engine will accept.
struct RuledResolutionPaymentIdentity
{
    quint64 promptId = 0;
    std::string answerSemantics;
};

enum class RuledPaymentResumeDecision
{
    Resume,
    Wait,
    Discard,
};

/// Decide whether the top suspended ResolutionPayment can be restored into the current client
/// view. This is a small policy shared by the UI bridge and its headless state/model tests.
[[nodiscard]] inline RuledPaymentResumeDecision
ruledResolutionPaymentResumeDecision(const RuledClientState &state,
                                     const RuledResolutionPaymentIdentity &parent,
                                     bool localActivationPending,
                                     bool parentPaymentSubmitting)
{
    if (state.isEngineCommandPending() || localActivationPending || state.payment.submitting ||
        parentPaymentSubmitting) {
        return RuledPaymentResumeDecision::Wait;
    }

    if (state.pendingChoice.has_value()) {
        const auto &current = *state.pendingChoice;
        if (current.kind != RuledClientState::ChoiceKind::ResolutionPayment)
            return RuledPaymentResumeDecision::Wait;

        const bool sameSemantics = !parent.answerSemantics.empty() &&
                                   current.resolutionPromptSemantics == parent.answerSemantics &&
                                   state.resolutionPromptSemantics == parent.answerSemantics;
        if (parent.promptId != 0 && current.resolutionPromptId == parent.promptId) {
            if (sameSemantics && state.isCurrentResolutionPrompt(parent.promptId))
                return RuledPaymentResumeDecision::Resume;
            // An unchanged prompt identity cannot acquire different answer semantics. Retire the
            // stale receipt rather than restoring its staged payment onto an invalid offer.
            return RuledPaymentResumeDecision::Discard;
        }

        if (current.resolutionPromptId != 0 && sameSemantics &&
            state.isCurrentResolutionPrompt(current.resolutionPromptId)) {
            // A fresh ID starts a new offer even when its answer semantics are identical.
            return RuledPaymentResumeDecision::Discard;
        }

        if (current.resolutionPromptId == 0 && state.resolutionPromptSnapshotValid && state.resolutionPromptActive &&
            state.resolutionPromptId == 0) {
            // A malformed/changed same-ID offer is left active only as a blocker; it has no usable
            // prompt identity, so the saved payment cannot be restored onto it.
            return RuledPaymentResumeDecision::Discard;
        }

        // A different ID and signature can be a genuine nested ResolutionPayment. Keep the
        // suspended parent until that choice clears and its exact offer is published again.
        return RuledPaymentResumeDecision::Wait;
    }

    if (state.isWaitingForChoice())
        return RuledPaymentResumeDecision::Wait;

    // No current choice and no public waiting choice means this receipt no longer has a parent.
    return RuledPaymentResumeDecision::Discard;
}

#endif // RULED_PAYMENT_RESUME_POLICY_H
