#ifndef RULED_UTILS_H
#define RULED_UTILS_H

#include <QString>
#include <libcockatrice/protocol/pb/ruled_v1.pb.h>
#include <string>

bool isRuledModeManaPoolCounterName(const QString &name);
int ruledPhaseLabelToCockatricePhase(const std::string &phase);

/// True when a resolution-choice kind exposes a zone concealed from the other players, so the
/// relay must strip the candidate ids/names from every participant but the deciding player.
/// Public kinds (revealed cards, battlefield targets, legend keep) pass through untouched.
bool isPrivateChoiceKind(ruled::v1::ChoiceKind kind);

#endif
