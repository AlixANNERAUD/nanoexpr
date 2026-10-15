/**
 * @file nanoexpr.h
 * @brief Tiny math expression evaluator: zero heap allocation, single pass, variables and functions.
 *
 * Expressions are evaluated in `double`. Nothing is allocated and no state is kept between calls, so every
 * function is reentrant and thread safe as long as the callbacks are.
 */

#ifndef NANOEXPR_H
#define NANOEXPR_H

#include <stdbool.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/** @brief The most arguments a function call can take. More give ::NANOEXPR_TOO_MANY_ARGUMENTS. */
#define NANOEXPR_MAX_ARGUMENTS 8

/** @brief Default limit of nested parentheses, calls and operators. */
#define NANOEXPR_DEFAULT_MAX_DEPTH 32

/** @brief Outcome of an evaluation. */
typedef enum nanoexpr_status {
    /** @brief Success. */
    NANOEXPR_OK = 0,
    /** @brief A character that is not part of the syntax, or input that is not valid UTF-8. */
    NANOEXPR_UNEXPECTED_CHARACTER,
    /** @brief A token where it cannot appear, such as `2 + * 3`. */
    NANOEXPR_UNEXPECTED_TOKEN,
    /** @brief The input ends where an operand or a closing parenthesis is expected. */
    NANOEXPR_UNEXPECTED_END,
    /** @brief A `(` without its matching `)`. */
    NANOEXPR_UNCLOSED_PARENTHESIS,
    /** @brief A number literal that cannot be represented. */
    NANOEXPR_INVALID_NUMBER,
    /** @brief A name that is not a known variable. */
    NANOEXPR_UNKNOWN_VARIABLE,
    /** @brief A name that is not a known function. */
    NANOEXPR_UNKNOWN_FUNCTION,
    /** @brief A call with more than ::NANOEXPR_MAX_ARGUMENTS arguments. */
    NANOEXPR_TOO_MANY_ARGUMENTS,
    /** @brief A call with the wrong number of arguments for its function. */
    NANOEXPR_ARGUMENT_COUNT,
    /** @brief A division or remainder by zero. */
    NANOEXPR_DIVISION_BY_ZERO,
    /** @brief An operand outside the domain of an operation, such as `sqrt(-1)`. */
    NANOEXPR_DOMAIN,
    /** @brief A result too large to represent. */
    NANOEXPR_OVERFLOW,
    /** @brief Parentheses, calls or operators nested deeper than allowed. */
    NANOEXPR_TOO_DEEP,
    /** @brief A pointer that must not be null is null. */
    NANOEXPR_INVALID_ARGUMENT,
    /** @brief An error this version of the library does not know. */
    NANOEXPR_UNKNOWN_ERROR
} nanoexpr_status;

/** @brief An evaluation error and the byte range of the input that caused it. */
typedef struct nanoexpr_error {
    /** @brief What went wrong, ::NANOEXPR_OK on success. */
    nanoexpr_status kind;
    /** @brief Offset of the first byte. */
    size_t start;
    /** @brief Offset one past the last byte. */
    size_t end;
} nanoexpr_error;

/**
 * @brief Resolves a variable.
 *
 * @param user_data The `user_data` of the options.
 * @param name The name of the variable, not NUL-terminated.
 * @param name_length The length of @p name in bytes.
 * @param value Where to write the value.
 * @return `true` after writing @p value if the variable exists, `false` otherwise.
 */
typedef bool (*nanoexpr_variable_callback)(void *user_data, const char *name, size_t name_length, double *value);

/**
 * @brief Calls a function.
 *
 * @param user_data The `user_data` of the options.
 * @param name The name of the function, not NUL-terminated.
 * @param name_length The length of @p name in bytes.
 * @param arguments The arguments, not to be read when @p argument_count is 0.
 * @param argument_count The number of @p arguments.
 * @param result Where to write the result.
 * @return ::NANOEXPR_OK after writing @p result, ::NANOEXPR_UNKNOWN_FUNCTION if the function does not exist, so
 *         that the built-in functions are tried, or another status, such as ::NANOEXPR_ARGUMENT_COUNT or
 *         ::NANOEXPR_DOMAIN, to fail the evaluation with it.
 */
typedef nanoexpr_status (*nanoexpr_function_callback)(void *user_data, const char *name, size_t name_length,
                                                      const double *arguments, size_t argument_count,
                                                      double *result);

/**
 * @brief How to evaluate an expression.
 *
 * Start from nanoexpr_default_options() and change what you need.
 */
typedef struct nanoexpr_options {
    /** @brief Resolves variables, before the built-in constants. May be `NULL`. */
    nanoexpr_variable_callback variable;
    /** @brief Calls functions, before the built-in functions. May be `NULL`. */
    nanoexpr_function_callback function;
    /** @brief Passed to both callbacks. */
    void *user_data;
    /** @brief Whether the built-in constants (`pi`, `e`...) and functions (`sqrt`, `sin`, `max`...) exist. */
    bool math;
    /** @brief Whether `%` is a percentage, as in `200 * 15%`, rather than the remainder. `mod` is always the
     *         remainder. */
    bool percentage;
    /** @brief Most levels of nested parentheses, calls and operators, which bounds the stack used. */
    size_t max_depth;
} nanoexpr_options;

/**
 * @brief Returns the default options.
 *
 * No callbacks, built-in constants and functions, `%` as the remainder and a depth limit of
 * ::NANOEXPR_DEFAULT_MAX_DEPTH.
 */
nanoexpr_options nanoexpr_default_options(void);

/**
 * @brief Evaluates an expression with the default options.
 *
 * @param input The expression, in UTF-8. It does not need to be NUL-terminated.
 * @param length The length of @p input in bytes.
 * @param result Where to write the value on success. Left untouched on failure.
 * @param error Where to write the status and the span of the error, or `NULL`. Its span is `0..0` on success.
 * @return ::NANOEXPR_OK on success, the error otherwise, or ::NANOEXPR_INVALID_ARGUMENT if @p input or
 *         @p result is `NULL`.
 */
nanoexpr_status nanoexpr_evaluate(const char *input, size_t length, double *result, nanoexpr_error *error);

/**
 * @brief Evaluates an expression with options.
 *
 * Names are resolved by the callbacks of @p options first, then by the built-in constants and functions if
 * `options->math` is set. Callbacks must not throw C++ exceptions or `longjmp` out.
 *
 * @param input The expression, in UTF-8. It does not need to be NUL-terminated.
 * @param length The length of @p input in bytes.
 * @param options The options.
 * @param result Where to write the value on success. Left untouched on failure.
 * @param error Where to write the status and the span of the error, or `NULL`. Its span is `0..0` on success.
 * @return ::NANOEXPR_OK on success, the error otherwise, or ::NANOEXPR_INVALID_ARGUMENT if @p input,
 *         @p options or @p result is `NULL`.
 */
nanoexpr_status nanoexpr_evaluate_with(const char *input, size_t length, const nanoexpr_options *options,
                                       double *result, nanoexpr_error *error);

/**
 * @brief Returns a short English description of a status.
 *
 * @param status A ::nanoexpr_status value.
 * @return A static NUL-terminated string, such as `"division by zero"`.
 */
const char *nanoexpr_status_description(nanoexpr_status status);

#ifdef __cplusplus
}
#endif

#endif
