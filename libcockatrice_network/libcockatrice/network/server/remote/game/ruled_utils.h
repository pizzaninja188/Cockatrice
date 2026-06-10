#ifndef RULED_UTILS_H
#define RULED_UTILS_H

#include <QString>
#include <string>

bool isRuledModeManaPoolCounterName(const QString &name);
int ruledPhaseLabelToCockatricePhase(const std::string &phase);

#endif
