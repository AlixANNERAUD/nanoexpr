cmake_minimum_required(VERSION 3.21)

project(nanoexpr LANGUAGES C)

option(NANOEXPR_BUILD_TESTS "Build the tests of nanoexpr" ${PROJECT_IS_TOP_LEVEL})
set(NANOEXPR_CARGO_TARGET "" CACHE STRING "Rust target triple to build nanoexpr for, empty for the host")
set(NANOEXPR_CARGO_FLAGS "" CACHE STRING "Extra arguments of cargo build, such as --no-default-features")

find_program(CARGO cargo REQUIRED)

set(NANOEXPR_CARGO_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}/cargo")
set(NANOEXPR_CARGO_ARGUMENTS
    build
    --release
    --package nanoexpr-ffi
    --lib
    --manifest-path "${CMAKE_CURRENT_SOURCE_DIR}/Cargo.toml"
    --target-dir "${NANOEXPR_CARGO_DIRECTORY}"
)
if(NANOEXPR_CARGO_TARGET)
    list(APPEND NANOEXPR_CARGO_ARGUMENTS --target "${NANOEXPR_CARGO_TARGET}")
    set(NANOEXPR_CARGO_OUTPUT "${NANOEXPR_CARGO_DIRECTORY}/${NANOEXPR_CARGO_TARGET}/release")
else()
    set(NANOEXPR_CARGO_OUTPUT "${NANOEXPR_CARGO_DIRECTORY}/release")
endif()
separate_arguments(NANOEXPR_CARGO_EXTRA_FLAGS NATIVE_COMMAND "${NANOEXPR_CARGO_FLAGS}")
list(APPEND NANOEXPR_CARGO_ARGUMENTS ${NANOEXPR_CARGO_EXTRA_FLAGS})

set(NANOEXPR_LIBRARY
    "${NANOEXPR_CARGO_OUTPUT}/${CMAKE_STATIC_LIBRARY_PREFIX}nanoexpr_c${CMAKE_STATIC_LIBRARY_SUFFIX}")

add_custom_target(nanoexpr_cargo
    COMMAND "${CARGO}" ${NANOEXPR_CARGO_ARGUMENTS}
    BYPRODUCTS "${NANOEXPR_LIBRARY}"
    WORKING_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}"
    COMMENT "Building nanoexpr with cargo"
    USES_TERMINAL
    VERBATIM
)

add_library(nanoexpr STATIC IMPORTED GLOBAL)
set_target_properties(nanoexpr PROPERTIES
    IMPORTED_LOCATION "${NANOEXPR_LIBRARY}"
    INTERFACE_INCLUDE_DIRECTORIES "${CMAKE_CURRENT_SOURCE_DIR}/include"
)
add_dependencies(nanoexpr nanoexpr_cargo)

if(NOT NANOEXPR_CARGO_FLAGS MATCHES "--no-default-features")
    if(UNIX AND NOT APPLE)
        find_package(Threads REQUIRED)
        target_link_libraries(nanoexpr INTERFACE Threads::Threads ${CMAKE_DL_LIBS} m)
    endif()
endif()

if(NANOEXPR_BUILD_TESTS)
    enable_testing()
    add_executable(nanoexpr_test tests/test.c)
    target_link_libraries(nanoexpr_test PRIVATE nanoexpr)
    set_target_properties(nanoexpr_test PROPERTIES C_STANDARD 99 C_STANDARD_REQUIRED ON C_EXTENSIONS OFF)
    target_compile_options(nanoexpr_test PRIVATE
        $<$<C_COMPILER_ID:GNU,Clang,AppleClang>:-Wall -Wextra -Wpedantic -Werror>)
    add_test(NAME nanoexpr_test COMMAND nanoexpr_test)
endif()
