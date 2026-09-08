// One CTest executable: real server E2E scenarios and focused harness characterization.
#include <QCoreApplication>
#include <gtest/gtest.h>

int main(int argc, char **argv)
{
    QCoreApplication app(argc, argv);
    ::testing::InitGoogleTest(&argc, argv);
    return RUN_ALL_TESTS();
}
