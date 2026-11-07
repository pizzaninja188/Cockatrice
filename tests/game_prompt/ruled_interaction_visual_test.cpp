#include "game/ruled/ruled_interaction_visual.h"

#include <QImage>
#include <gtest/gtest.h>

namespace
{
QImage render(const RuledInteractionVisual &visual, int strength = 45)
{
    QImage image(80, 100, QImage::Format_ARGB32_Premultiplied);
    image.fill(Qt::white);
    QPainter painter(&image);
    QPainterPath shape;
    shape.addRoundedRect(QRectF(10, 10, 60, 80), 8, 8);
    RuledInteractionPainter::paint(&painter, shape, visual, strength);
    return image;
}
} // namespace

TEST(RuledInteractionVisual, DarkensIneligibleArtWithoutCoveringOutsideTheCard)
{
    const auto image = render({true});
    EXPECT_NEAR(image.pixelColor(40, 50).red(), 140, 1);
    EXPECT_EQ(image.pixelColor(0, 0), QColor(Qt::white));
    EXPECT_EQ(image.pixelColor(10, 10), QColor(Qt::white));
}

TEST(RuledInteractionVisual, CandidatesStayBrightAndHaveAVisibleBoundary)
{
    const auto image = render({false, true});
    EXPECT_EQ(image.pixelColor(40, 50), QColor(Qt::white));
    EXPECT_NE(image.pixelColor(10, 50), QColor(Qt::white));
    EXPECT_EQ(render({}).pixelColor(10, 50), QColor(Qt::white));
}

TEST(RuledInteractionVisual, AffordableCardsUseWhiteWithDarkBackingAndSelectionTakesPrecedence)
{
    const auto image = render({false, false, true});
    EXPECT_EQ(image.pixelColor(10, 50), QColor(Qt::white));
    EXPECT_LT(image.pixelColor(13, 50).red(), 80);
    EXPECT_EQ(image.pixelColor(40, 50), QColor(Qt::white));
    EXPECT_EQ(render({true, false, true}), render({true}));
    EXPECT_EQ(render({false, true, true}), render({false, true}));
}

TEST(RuledInteractionVisual, DimStrengthIsBoundedAndPainterStateIsRestored)
{
    EXPECT_EQ(render({true}, -10).pixelColor(40, 50), QColor(Qt::white));
    EXPECT_EQ(render({true}, 150).pixelColor(40, 50), QColor(Qt::black));
    QImage image(80, 100, QImage::Format_ARGB32_Premultiplied);
    image.fill(Qt::white);
    QPainter painter(&image);
    painter.setPen(QPen(Qt::magenta, 7));
    painter.setBrush(Qt::green);
    const auto pen = painter.pen();
    const auto brush = painter.brush();
    QPainterPath shape;
    shape.addRect(QRectF(0, 0, 80, 100));
    RuledInteractionPainter::paint(&painter, shape, {true, true});
    EXPECT_EQ(painter.pen(), pen);
    EXPECT_EQ(painter.brush(), brush);
    // Selection badges and counters painted afterwards retain their full contrast.
    painter.fillRect(QRect(30, 40, 20, 20), Qt::yellow);
    EXPECT_EQ(image.pixelColor(40, 50), QColor(Qt::yellow));
}
