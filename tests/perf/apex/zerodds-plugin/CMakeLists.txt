# CMakeLists.txt — ament_cmake-Package "performance_test_zerodds_plugin".
#
# Wird vom Apex.AI performance_test in Sub-Mode 'plugins' eingebunden:
#   colcon build --packages-select performance_test_zerodds_plugin
# bzw. via Apex' Plugin-Selector
#   -DPERFORMANCE_TEST_PLUGIN=ZERODDS

cmake_minimum_required(VERSION 3.16)
project(performance_test_zerodds_plugin
        VERSION 0.0.0
        LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)
set(CMAKE_POSITION_INDEPENDENT_CODE ON)

# Findet libzerodds.so + zerodds.h. Erwartung: ZEROZEROS_DIR zeigt auf
# das lokale Build-Output. Default: $ENV{ZERODDS_HOME}, sonst error.
set(ZERODDS_HOME "$ENV{ZERODDS_HOME}" CACHE PATH "Repo-Root von ZeroDDS")
if(NOT ZERODDS_HOME)
  message(FATAL_ERROR
    "ZERODDS_HOME nicht gesetzt. Setze CMake-Var oder env-Variable auf "
    "den Repo-Root von ZeroDDS (dort wo crates/dds-c-api liegt).")
endif()

set(ZERODDS_INCLUDE_DIR "${ZERODDS_HOME}/crates/dds-c-api/include")
set(ZERODDS_LIB_DIR     "${ZERODDS_HOME}/target/release")

if(NOT EXISTS "${ZERODDS_INCLUDE_DIR}/zerodds.h")
  message(FATAL_ERROR "zerodds.h nicht in ${ZERODDS_INCLUDE_DIR} — "
                      "vorher 'cargo build -p dds-c-api --release' laufen lassen.")
endif()

find_library(ZERODDS_LIB
             NAMES zerodds
             PATHS "${ZERODDS_LIB_DIR}"
             NO_DEFAULT_PATH
             REQUIRED)

# Apex.AI's CMake gibt's nur wenn das Plugin im Apex-Tree gebaut wird
# — wir machen es als optional damit es auch standalone kompiliert.
find_package(ament_cmake QUIET)

add_library(${PROJECT_NAME} SHARED
  src/zerodds_communicator.cpp)

target_include_directories(${PROJECT_NAME}
  PUBLIC
    $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/include>
    $<INSTALL_INTERFACE:include>
    ${ZERODDS_INCLUDE_DIR})

target_link_libraries(${PROJECT_NAME} PUBLIC ${ZERODDS_LIB})

# RPATH so dass das Plugin libzerodds.so in target/release/ findet.
set_target_properties(${PROJECT_NAME} PROPERTIES
  INSTALL_RPATH "${ZERODDS_LIB_DIR}"
  BUILD_RPATH   "${ZERODDS_LIB_DIR}")

# Optional in-process-Smoke (kein Apex/ROS noetig). Einschalten ueber
# -DBUILD_PLUGIN_SMOKE=ON.
option(BUILD_PLUGIN_SMOKE "Build plugin_smoke standalone" OFF)
if(BUILD_PLUGIN_SMOKE)
  find_package(Threads REQUIRED)
  add_executable(plugin_smoke smoke/plugin_smoke.cpp)
  target_link_libraries(plugin_smoke PRIVATE ${PROJECT_NAME} Threads::Threads)
  set_target_properties(plugin_smoke PROPERTIES
    INSTALL_RPATH "${ZERODDS_LIB_DIR};${CMAKE_BINARY_DIR}"
    BUILD_RPATH   "${ZERODDS_LIB_DIR};${CMAKE_BINARY_DIR}")
endif()

if(ament_cmake_FOUND)
  ament_export_include_directories(include)
  ament_export_libraries(${PROJECT_NAME})
  ament_export_dependencies()
  install(DIRECTORY include/ DESTINATION include)
  install(TARGETS ${PROJECT_NAME}
          EXPORT export_${PROJECT_NAME}
          LIBRARY DESTINATION lib)
  ament_package()
else()
  install(DIRECTORY include/ DESTINATION include)
  install(TARGETS ${PROJECT_NAME} LIBRARY DESTINATION lib)
endif()
