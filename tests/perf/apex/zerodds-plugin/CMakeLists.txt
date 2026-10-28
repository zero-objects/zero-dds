# CMakeLists.txt — ament_cmake-Package "performance_test_zerodds_plugin".
#
# Included by the Apex.AI performance_test in sub-mode 'plugins':
#   colcon build --packages-select performance_test_zerodds_plugin
# or via Apex' plugin selector
#   -DPERFORMANCE_TEST_PLUGIN=ZERODDS

cmake_minimum_required(VERSION 3.16)
project(performance_test_zerodds_plugin
        VERSION 0.0.0
        LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)
set(CMAKE_POSITION_INDEPENDENT_CODE ON)

# Finds libzerodds.so + zerodds.h. Expectation: ZEROZEROS_DIR points at
# the local build output. Default: $ENV{ZERODDS_HOME}, otherwise an error.
set(ZERODDS_HOME "$ENV{ZERODDS_HOME}" CACHE PATH "Repository root of ZeroDDS")
if(NOT ZERODDS_HOME)
  message(FATAL_ERROR
    "ZERODDS_HOME is not set. Set the CMake variable or the environment variable to "
    "the repository root of ZeroDDS (where crates/dds-c-api lives).")
endif()

set(ZERODDS_INCLUDE_DIR "${ZERODDS_HOME}/crates/dds-c-api/include")
set(ZERODDS_LIB_DIR     "${ZERODDS_HOME}/target/release")

if(NOT EXISTS "${ZERODDS_INCLUDE_DIR}/zerodds.h")
  message(FATAL_ERROR "zerodds.h not in ${ZERODDS_INCLUDE_DIR} — "
                      "run 'cargo build -p dds-c-api --release' first.")
endif()

find_library(ZERODDS_LIB
             NAMES zerodds
             PATHS "${ZERODDS_LIB_DIR}"
             NO_DEFAULT_PATH
             REQUIRED)

# Apex.AI's CMake exists only when the plugin is built in the Apex tree
# — we make it optional so that it also compiles standalone.
find_package(ament_cmake QUIET)

add_library(${PROJECT_NAME} SHARED
  src/zerodds_communicator.cpp)

target_include_directories(${PROJECT_NAME}
  PUBLIC
    $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/include>
    $<INSTALL_INTERFACE:include>
    ${ZERODDS_INCLUDE_DIR})

target_link_libraries(${PROJECT_NAME} PUBLIC ${ZERODDS_LIB})

# RPATH so that the plugin finds libzerodds.so in target/release/.
set_target_properties(${PROJECT_NAME} PROPERTIES
  INSTALL_RPATH "${ZERODDS_LIB_DIR}"
  BUILD_RPATH   "${ZERODDS_LIB_DIR}")

# Optional in-process smoke test (no Apex/ROS needed). Switch it on with
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
