add_executable(parse_cipt_test ../../oracle/src/parsehelpers.cpp parse_cipt_test.cpp)

if(NOT GTEST_FOUND)
  add_dependencies(parse_cipt_test gtest)
endif()

target_link_libraries(parse_cipt_test Threads::Threads ${GTEST_BOTH_LIBRARIES} ${TEST_QT_MODULES})

add_test(NAME parse_cipt_test COMMAND parse_cipt_test)

if(WIN32 AND TARGET oracle)
  add_test(
    NAME oracle_runtime_test
    COMMAND powershell.exe -NoProfile -File "${CMAKE_SOURCE_DIR}/tests/scripts/oracle_runtime_test.ps1"
            -OracleExecutable "$<TARGET_FILE:oracle>"
            -QtVersionName "${COCKATRICE_QT_VERSION_NAME}" -Configuration "$<CONFIG>"
  )
endif()
