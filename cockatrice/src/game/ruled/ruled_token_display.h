#ifndef RULED_TOKEN_DISPLAY_H
#define RULED_TOKEN_DISPLAY_H

#include <QString>
#include <QStringList>
#include <QtGlobal>
#include <libcockatrice/utility/card_ref.h>

class CardDatabaseQuerier;
class ServerInfo_Card;
namespace ruled::v1 { class TokenIdentity; }

namespace RuledTokenDisplay
{
QString describe(const ruled::v1::TokenIdentity &identity);
// Decorate a transient popup record; this creates no server physical card or engine object.
void applyProposal(ServerInfo_Card &card, const ruled::v1::TokenIdentity &identity, const CardDatabaseQuerier *db);
// Display-only resolver for engine-created tokens. The engine supplies the authoritative printed
// characteristics; this helper chooses an Oracle token entry only when those characteristics
// match exactly. An empty result means the caller must keep the self-described engine token.
CardRef resolve(const CardDatabaseQuerier *db,
                const QString &tokenName,
                const QString &basePt,
                const QString &color,
                const QStringList &keywords,
                const QStringList &abilityTexts,
                const QString &manaCost = {},
                quint32 manaValue = 0);
} // namespace RuledTokenDisplay

#endif
