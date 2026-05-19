#ifndef RULED_UTILS_H
#define RULED_UTILS_H

#include <QString>
#include <string>

bool isRuledModeManaPoolCounterName(const QString &name);
int ruledPhaseLabelToCockatricePhase(const std::string &phase);

/// Maps an Oracle card name to a tricerules id (lowercase_underscore, apostrophes stripped).
/// Must stay in sync with tricerules-cards/data/*.ron `id` fields.
QString cardNameToTricerulesId(const QString &cardName);

#endif
