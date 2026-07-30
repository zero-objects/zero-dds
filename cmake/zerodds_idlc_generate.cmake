cmake_minimum_required(VERSION 3.21 FATAL_ERROR)

# The IDLC build contract (tools/idlc) writes a Make-/Ninja-compatible depfile,
# a JSON output manifest, and a single success stamp. This generator drives the
# compiler through that contract instead of guessing outputs from file
# extensions: exactly ONE `add_custom_command` per IDL × backend set whose
# OUTPUT is the stamp (stable, independent of how many files a backend emits)
# and whose transitive `#include` dependencies come from the DEPFILE. No
# extension logic, no per-language file enumeration, no separate run per backend
# for `all`.

# cmake-format: off
# Function wrapping the zerodds-idlc executable and generating source files from IDL
#
#   zerodds_idlc_generate( LIBRARY_NAME <generated_idl_library_name>
#                          IDL_FILES <list_of_idl_files>
#                          GENERATOR_BACKEND <cpp|c|rust|csharp|java|python|ts|...|all>
#                          [OUTPUT_DIR <destination_folder>]
#                          [BASE_DIR <subfolder_of_idl>]
#                          [IDL_INCLUDE_DIRS <list_of_folders_to_search_for_idl>]
#                          [LIBRARY_NAMESPACE <namespace added to LIBRARY_NAME>]
#                          [GENERATOR_FLAGS <list_of_flags_for_generator>]
#                          [CARGO_FEATURES <list_of_cargo_features_for_the_zerodds-idlc_build>]
#   )
#
# - LIBRARY_NAME: name of the library that will be created by CMake
# - IDL_FILES: list of IDL files that will be compiled and associated to LIBRARY_NAME
# - GENERATOR_BACKEND: which generator to use. A single backend (`cpp`, `c`,
#                      `rust`, `csharp`, `java`, `python`, `ts`, `go`, `ada`, …)
#                      or `all` for every backend in ONE compiler run.
# - OUTPUT_DIR: folder in which compiled IDL files will be put, and also folder to which the
#               library will have access thanks to target_include_directory directive.
#               Default: `${CMAKE_CURRENT_BINARY_DIR}/zerodds/idl`
# - BASE_DIR: subfolder beloning to IDL_FILES since which the directory tree will be preserved, thus
#             organization of compiled IDL files will be unchanged in OUTPUT_DIR folder
# - IDL_INCLUDE_DIRS: directories in which to search IDL files addressed by #include directive
#                     during IDL parsing (forwarded to `zerodds-idlc` as repeated `-I <dir>`)
# - LIBRARY_NAMESPACE: namespace for the generated library. Default is `zerodds-idlc`
# - GENERATOR_FLAGS: further generator flags not handled directly by CMake that will
#                    be forwarded to zerodds-idlc
# - CARGO_FEATURES: Cargo feature list forwarded to the ONE-TIME `cargo rustc --package
#                    zerodds-idlc --features <...>` build (e.g. `ext-default-final` to
#                    switch the compiled-in default extensibility). No-op once
#                    `zerodds::zerodds-idlc` already exists as a target — set it on the
#                    FIRST `zerodds_idlc_generate()` call in the whole configure run.
#
# After configuring, the created library carries a `ZERODDS_IDLC_GENERATED_FILES`
# target property listing every file the compiler reported in its manifest on the
# PREVIOUS build (empty before the first build). Downstream code that needs the
# concrete file set (e.g. a Java sources list) reads that property; the build
# dependency itself is always carried by the stamp, so nothing depends on the
# property being populated.
# cmake-format: on
function(zerodds_idlc_generate LIBRARY_NAME)
    include(CMakeParseArguments)

    set(_options "")
    set(_singleargs OUTPUT_DIR LIBRARY_NAMESPACE GENERATOR_BACKEND BASE_DIR)
    set(_multiargs IDL_FILES GENERATOR_FLAGS IDL_INCLUDE_DIRS CARGO_FEATURES)
    cmake_parse_arguments(PARSE_ARGV 1 "" "${_options}" "${_singleargs}" "${_multiargs}")

    # If output dir is not given, use the project binary one followed by include/zerodds/idl
    # thus allowing #include<zerodds/idl/*.idl>
    if(NOT _OUTPUT_DIR)
        set(_OUTPUT_DIR "${CMAKE_CURRENT_BINARY_DIR}/zerodds/idl")
    endif()

    if(NOT _GENERATOR_BACKEND)
        message(FATAL_ERROR "GENERATOR_BACKEND is a compulsory flag")
    endif()

    if(NOT _IDL_FILES)
        message(FATAL_ERROR "IDL_FILES is a compulsory flag")
    endif()

    # If zerodds-idlc target is not defined, call cargo to build it
    if(NOT TARGET zerodds::zerodds-idlc)
        # CARGO_EXECUTABLE should have been defined in the root CMakeLists.txt
        # anyway search it again and throw error if not found
        if(NOT CARGO_EXECUTABLE)
            find_program(CARGO_EXECUTABLE NAMES cargo)
            if("${CARGO_EXECUTABLE}" STREQUAL "CARGO_EXECUTABLE-NOTFOUND")
            message(FATAL_ERROR "Cargo not found. Look at: https://rust-lang.org/learn/get-started")
            endif()
        endif()

        # Set OS-Specific executable extension
        if(WIN32)
            set(ZERODDS_IDLC_EXE_EXTENSION ".exe")
        endif()

        # `--features a,b,c` — comma-joined, cargo's own separator (bug: an earlier
        # revision forwarded neither this nor GENERATOR_FLAGS below because both
        # referenced undefined variable names instead of the cmake_parse_arguments
        # `_`-prefixed ones actually populated above).
        set(_zerodds_idlc_cargo_features_flag)
        if(_CARGO_FEATURES)
            list(JOIN _CARGO_FEATURES "," _zerodds_idlc_cargo_features_joined)
            set(_zerodds_idlc_cargo_features_flag "--features" "${_zerodds_idlc_cargo_features_joined}")
        endif()

        # HOST-TOOL split (spec P0-2): zerodds-idlc is a build-time code generator
        # invoked by CMake during this build, so it must be built for the HOST
        # triple even when the consuming project is cross-compiling its own
        # target artifacts. When the root CMakeLists.txt configured this build it
        # already resolved ZERODDS_CARGO_HOST_TARGET / ZERODDS_IDLC_HOST_FLAG /
        # ZERODDS_IDLC_HOST_TRIPLE_SUBDIR; reuse them. In a pure find_package()
        # consumer where zerodds itself is not the project, re-derive the host
        # triple from `rustc -vV` and pin it whenever a cross is in play (an
        # ambient CARGO_BUILD_TARGET is the tell), so an SDK env cannot cross-build
        # the generator into an unrunnable target-arch binary.
        if(NOT DEFINED ZERODDS_IDLC_HOST_FLAG)
            set(ZERODDS_IDLC_HOST_FLAG "")
            set(ZERODDS_IDLC_HOST_TRIPLE_SUBDIR "")
            if(NOT DEFINED ZERODDS_CARGO_HOST_TARGET OR ZERODDS_CARGO_HOST_TARGET STREQUAL "")
                find_program(RUSTC_EXECUTABLE NAMES rustc)
                set(ZERODDS_CARGO_HOST_TARGET "")
                if(RUSTC_EXECUTABLE)
                    execute_process(
                        COMMAND "${RUSTC_EXECUTABLE}" -vV
                        OUTPUT_VARIABLE _zerodds_rustc_vv
                        OUTPUT_STRIP_TRAILING_WHITESPACE ERROR_QUIET
                    )
                    string(REGEX MATCH "host: ([^\n\r]+)" _zerodds_host_match "${_zerodds_rustc_vv}")
                    if(_zerodds_host_match)
                        set(ZERODDS_CARGO_HOST_TARGET "${CMAKE_MATCH_1}")
                    endif()
                endif()
            endif()
            if(ZERODDS_CARGO_HOST_TARGET AND DEFINED ENV{CARGO_BUILD_TARGET}
               AND NOT "$ENV{CARGO_BUILD_TARGET}" STREQUAL ZERODDS_CARGO_HOST_TARGET)
                set(ZERODDS_IDLC_HOST_FLAG "--target" "${ZERODDS_CARGO_HOST_TARGET}")
                set(ZERODDS_IDLC_HOST_TRIPLE_SUBDIR "${ZERODDS_CARGO_HOST_TARGET}/")
            endif()
        endif()

        # Run rustc for building zerodds-idlc in Release, for the host triple.
        # The build executable will be put in the caller project binary directory.
        add_custom_target(zerodds_idlc_build ALL
            ${CARGO_EXECUTABLE} "rustc" "--manifest-path" "${CMAKE_CURRENT_FUNCTION_LIST_DIR}/../Cargo.toml"
                                "--package" "zerodds-idlc" "--release"
                                ${ZERODDS_IDLC_HOST_FLAG}
                                ${_zerodds_idlc_cargo_features_flag}
                                "--target-dir" "${CMAKE_CURRENT_BINARY_DIR}"
        )

        # Define zerodds-idlc as imported executable generated by `zerodds_idlc_build`
        add_executable(zerodds::zerodds-idlc IMPORTED GLOBAL)
        add_dependencies(zerodds::zerodds-idlc zerodds_idlc_build)
        set_target_properties(zerodds::zerodds-idlc PROPERTIES
            IMPORTED_LOCATION "${CMAKE_CURRENT_BINARY_DIR}/${ZERODDS_IDLC_HOST_TRIPLE_SUBDIR}release/zerodds-idlc${ZERODDS_IDLC_EXE_EXTENSION}"
        )
    endif()

    # Backend flag: a single `--<backend>`, or `--all` for every backend in one run.
    set(_zerodds_idlc_backend_flag "--${_GENERATOR_BACKEND}")

    # `-I <dir>` per include directory, in order — plain configure-time list (the
    # directories are known when CMake runs, not at build time, so no generator
    # expression is needed; an earlier revision guarded this behind an undefined
    # variable `IDLC_INCLUDE_DIRS_FLAG` and passed the whole list as ONE quoted
    # `"-I <dir1>;<dir2>"` string, so `-I` was never actually emitted).
    set(_zerodds_idlc_include_flags)
    foreach(_idl_include_dir IN LISTS _IDL_INCLUDE_DIRS)
        list(APPEND _zerodds_idlc_include_flags "-I" "${_idl_include_dir}")
    endforeach()

    # Build-contract artifacts live next to the generated sources, in a private
    # subfolder so they never collide with real outputs.
    set(_zerodds_idlc_meta_dir "${_OUTPUT_DIR}/.zerodds-idlc")

    # One add_custom_command per IDL file. The stamp is the single declared
    # OUTPUT; transitive #include dependencies are carried by the DEPFILE.
    set(_zerodds_idlc_stamps)
    set(_zerodds_idlc_generated_files)
    foreach(idl_source IN LISTS _IDL_FILES)
        get_filename_component(idl_filename "${idl_source}" NAME)
        get_filename_component(idl_directory_tree "${idl_source}" DIRECTORY)

        # Preserve the sub-tree below BASE_DIR in the output layout.
        set(expected_directory "")
        if(_BASE_DIR)
            string(REPLACE "${_BASE_DIR}" "" expected_directory "${idl_directory_tree}")
        endif()
        set(_idl_out_dir "${_OUTPUT_DIR}/${expected_directory}")

        # A stable, collision-free basename per (library, idl) — the IDL's
        # path below BASE_DIR flattened into a filesystem-safe token.
        set(_idl_rel "${expected_directory}/${idl_filename}")
        string(REGEX REPLACE "^/+" "" _idl_rel "${_idl_rel}")
        string(MAKE_C_IDENTIFIER "${LIBRARY_NAME}__${_idl_rel}" _idl_token)

        set(_stamp "${_zerodds_idlc_meta_dir}/${_idl_token}.stamp")
        set(_depfile "${_zerodds_idlc_meta_dir}/${_idl_token}.d")
        set(_manifest "${_zerodds_idlc_meta_dir}/${_idl_token}.manifest.json")

        add_custom_command(
            OUTPUT "${_stamp}"
            COMMAND zerodds::zerodds-idlc "generate" "${idl_source}"
                                          "${_zerodds_idlc_backend_flag}"
                                          "--output" "${_idl_out_dir}"
                                          ${_zerodds_idlc_include_flags}
                                          "--stamp" "${_stamp}"
                                          "--depfile" "${_depfile}"
                                          "--output-manifest" "${_manifest}"
                                          ${_GENERATOR_FLAGS}
            DEPENDS "${idl_source}" zerodds::zerodds-idlc
            DEPFILE "${_depfile}"
            BYPRODUCTS "${_manifest}"
            COMMENT "zerodds-idlc: generating ${idl_filename} (${_GENERATOR_BACKEND})"
            VERBATIM
        )
        list(APPEND _zerodds_idlc_stamps "${_stamp}")

        # Configure-time introspection: if a previous build left a manifest,
        # surface the concrete file list. Never load-bearing for the build
        # graph (the stamp is), so its absence on a clean tree is fine.
        if(EXISTS "${_manifest}")
            file(READ "${_manifest}" _manifest_json)
            # Extract the flat "outputs": [ ... ] array without a JSON parser
            # dependency: grab the bracket body and split on quoted strings.
            string(REGEX MATCH "\"outputs\"[ \t\r\n]*:[ \t\r\n]*\\[([^]]*)\\]"
                   _outputs_match "${_manifest_json}")
            if(_outputs_match)
                set(_outputs_body "${CMAKE_MATCH_1}")
                string(REGEX MATCHALL "\"([^\"]+)\"" _output_quoted "${_outputs_body}")
                foreach(_q IN LISTS _output_quoted)
                    string(REGEX REPLACE "^\"|\"$" "" _f "${_q}")
                    list(APPEND _zerodds_idlc_generated_files "${_f}")
                endforeach()
            endif()
        endif()
    endforeach()

    # A build target the library hangs off of: building it runs every stamp
    # command (and thus the compiler) exactly once per stale IDL.
    add_custom_target(${LIBRARY_NAME}_idlc_generate DEPENDS ${_zerodds_idlc_stamps})

    # The library itself is INTERFACE: in C/C++ the generator emits only headers,
    # and in other languages the outputs cannot be compiled by CMake. Consumers
    # that link it inherit the dependency on the generate target, so generation
    # runs before anything that uses the outputs.
    add_library(${LIBRARY_NAME} INTERFACE)
    add_dependencies(${LIBRARY_NAME} ${LIBRARY_NAME}_idlc_generate)

    if(_zerodds_idlc_generated_files)
        list(REMOVE_DUPLICATES _zerodds_idlc_generated_files)
    endif()
    set_target_properties(${LIBRARY_NAME} PROPERTIES
        ZERODDS_IDLC_GENERATED_FILES "${_zerodds_idlc_generated_files}"
    )

    if(${_GENERATOR_BACKEND} STREQUAL "c" OR ${_GENERATOR_BACKEND} STREQUAL "cpp")
        set_target_properties(${LIBRARY_NAME} PROPERTIES
            LINKER_LANGUAGE $<IF:$<STREQUAL: "${_GENERATOR_BACKEND}" , "c" >, "C" , "CXX" >
        )
        target_link_libraries(${LIBRARY_NAME} INTERFACE
            $<$<STREQUAL:"${_GENERATOR_BACKEND}","c">: zerodds::zerodds-c>
            $<$<STREQUAL:"${_GENERATOR_BACKEND}","cpp">: zerodds::zerodds-cpp>
        )
        target_include_directories(${LIBRARY_NAME} INTERFACE "${_OUTPUT_DIR}")
    endif()

    # Expose the library with the given namespace if specified
    # otherwise use the default namespace zerodds-idlc
    #
    # Bug fix: `cmake_parse_arguments(PARSE_ARGV 1 "" ...)` (empty prefix)
    # populates `_LIBRARY_NAMESPACE`, not bare `LIBRARY_NAMESPACE` — the
    # same undefined-variable-reference class as the IDL_INCLUDE_DIRS/
    # GENERATOR_FLAGS bugs above, caught by actually building a
    # `LIBRARY_NAMESPACE`-using consumer (the README's own Quickstart
    # example) end to end: the bare-name check is always false, so the
    # ALIAS was silently namespaced `zerodds-idlc::` instead of the
    # caller's namespace, and any caller (like the Quickstart) that
    # `target_link_libraries`s against its own namespaced alias failed
    # to configure with "target ... was not found".
    if(_LIBRARY_NAMESPACE)
        add_library(${_LIBRARY_NAMESPACE}::${LIBRARY_NAME} ALIAS ${LIBRARY_NAME})
    else()
        add_library(zerodds-idlc::${LIBRARY_NAME} ALIAS ${LIBRARY_NAME})
    endif()

endfunction()
