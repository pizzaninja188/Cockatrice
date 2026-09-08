#include "ruled_resolution_choice_dialog.h"

#include <QDialog>
#include <QDialogButtonBox>
#include <QLabel>
#include <QListWidget>
#include <QPushButton>
#include <QSet>
#include <QVBoxLayout>
#include <memory>

QVector<quint32> askRuledResolutionChoice(const QString &prompt,
                                          const QVector<quint32> &oids,
                                          const QStringList &names,
                                          int minN,
                                          int maxN,
                                          bool ordered,
                                          bool uniqueNames)
{
    QDialog dlg;
    dlg.setWindowTitle(QObject::tr("Resolve"));
    // Resolution is mandatory (CR 608); disable the X button so the player
    // cannot dismiss the dialog without submitting a legal selection.
    dlg.setWindowFlags(dlg.windowFlags() & ~Qt::WindowCloseButtonHint);
    auto *layout = new QVBoxLayout(&dlg);
    layout->addWidget(new QLabel(prompt, &dlg));
    auto *list = new QListWidget(&dlg);
    for (int i = 0; i < names.size(); ++i) {
        new QListWidgetItem(names.value(i), list);
    }
    layout->addWidget(list);
    auto *buttons = new QDialogButtonBox(QDialogButtonBox::Ok, &dlg);
    layout->addWidget(buttons);

    auto chosen = std::make_shared<QVector<int>>(); // selected rows, in click order
    if (minN == 0) {
        auto *decline = buttons->addButton(QObject::tr("Decline"), QDialogButtonBox::ActionRole);
        decline->setObjectName("resolutionChoiceDeclineButton");
        QObject::connect(decline, &QPushButton::clicked, &dlg, [chosen, &dlg]() {
            chosen->clear();
            dlg.accept();
        });
    }
    auto refresh = [=]() {
        // Collect names already chosen (for uniqueNames enforcement).
        QSet<QString> chosenNameSet;
        if (uniqueNames) {
            for (int r : *chosen) {
                chosenNameSet.insert(names.value(r));
            }
        }
        for (int r = 0; r < list->count(); ++r) {
            const int pos = chosen->indexOf(r);
            if (pos < 0) {
                // Not yet chosen — grey out if it would violate unique-names.
                const bool blocked = uniqueNames && chosenNameSet.contains(names.value(r));
                list->item(r)->setText(names.value(r));
                list->item(r)->setFlags(blocked ? list->item(r)->flags() & ~Qt::ItemIsEnabled
                                                : list->item(r)->flags() | Qt::ItemIsEnabled);
            } else if (ordered) {
                list->item(r)->setText(QStringLiteral("%1. %2").arg(pos + 1).arg(names.value(r)));
                list->item(r)->setFlags(list->item(r)->flags() | Qt::ItemIsEnabled);
            } else {
                list->item(r)->setText(QStringLiteral("✓ %1").arg(names.value(r)));
                list->item(r)->setFlags(list->item(r)->flags() | Qt::ItemIsEnabled);
            }
        }
        buttons->button(QDialogButtonBox::Ok)->setEnabled(chosen->size() >= minN && chosen->size() <= maxN);
    };
    QObject::connect(list, &QListWidget::itemClicked, [=](QListWidgetItem *item) {
        const int r = list->row(item);
        const int pos = chosen->indexOf(r);
        if (pos >= 0) {
            chosen->remove(pos);
        } else if (chosen->size() < maxN) {
            // itemClicked fires even for visually-disabled items, so re-check uniqueness here.
            bool nameTaken = false;
            if (uniqueNames) {
                const QString clickedName = names.value(r);
                for (int cr : *chosen) {
                    if (names.value(cr) == clickedName) {
                        nameTaken = true;
                        break;
                    }
                }
            }
            if (!nameTaken) {
                chosen->append(r);
            }
        }
        refresh();
    });
    QObject::connect(buttons, &QDialogButtonBox::accepted, &dlg, &QDialog::accept);
    refresh();
    dlg.exec();

    QVector<quint32> out;
    for (int r : *chosen) {
        out.append(oids.value(r));
    }
    return out;
}
