#ifndef COCKATRICE_RULED_AUTOMATION_UI_H
#define COCKATRICE_RULED_AUTOMATION_UI_H
#include <QPoint>
class CardItem;
class QMenu;
class TabGame;
struct RuledAutomationUi
{
    static void configure(TabGame &tab);
    static void registerDock(TabGame &tab);
    static void retranslateUi(TabGame &tab);
    static void resetLayout(TabGame &tab);
    static void refresh(TabGame &tab);
    static QMenu *stackMenu(CardItem *card);
    static bool tryStackMenu(CardItem *card, const QPoint &position);
};
#endif
