#ifndef COCKATRICE_RULED_RESOLUTION_CHOICE_DIALOG_H
#define COCKATRICE_RULED_RESOLUTION_CHOICE_DIALOG_H
#include <QStringList>
#include <QVector>

QVector<quint32> askRuledResolutionChoice(const QString &prompt,
                                          const QVector<quint32> &oids,
                                          const QStringList &names,
                                          int minN,
                                          int maxN,
                                          bool ordered,
                                          bool uniqueNames);
#endif
