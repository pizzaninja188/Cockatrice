#ifndef RULED_TOKEN_DISPLAY_H
#define RULED_TOKEN_DISPLAY_H

#include <QString>
#include <QStringList>
#include <libcockatrice/utility/card_ref.h>

class CardDatabaseQuerier;

namespace RuledTokenDisplay
{
// Display-only resolver for engine-created tokens. The engine supplies the authoritative printed
// characteristics; this helper chooses an Oracle token entry only when those characteristics
// match exactly. An empty result means the caller must keep the self-described engine token.
CardRef resolve(const CardDatabaseQuerier *db,
                const QString &tokenName,
                const QString &basePt,
                const QString &color,
                const QStringList &keywords,
                const QStringList &triggeredAbilityTexts);
} // namespace RuledTokenDisplay

#endif
