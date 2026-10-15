#undef NDEBUG

#include <assert.h>
#include <math.h>
#include <stdio.h>
#include <string.h>

#include "nanoexpr.h"

typedef struct sensors {
    double temperature;
    int calls;
} sensors;

static bool variable(void *user_data, const char *name, size_t name_length, double *value) {
    sensors *data = user_data;
    if (name_length == 11 && memcmp(name, "temperature", 11) == 0) {
        *value = data->temperature;
        return true;
    }
    return false;
}

static nanoexpr_status function(void *user_data, const char *name, size_t name_length, const double *arguments,
                                size_t argument_count, double *result) {
    sensors *data = user_data;
    data->calls++;
    if (name_length == 6 && memcmp(name, "double", 6) == 0) {
        if (argument_count != 1) {
            return NANOEXPR_ARGUMENT_COUNT;
        }
        *result = arguments[0] * 2;
        return NANOEXPR_OK;
    }
    return NANOEXPR_UNKNOWN_FUNCTION;
}

static double value(const char *input) {
    double result = NAN;
    nanoexpr_error error;
    nanoexpr_status status = nanoexpr_evaluate(input, strlen(input), &result, &error);
    assert(status == NANOEXPR_OK);
    assert(error.kind == NANOEXPR_OK);
    return result;
}

static void fails(const nanoexpr_options *options, const char *input, nanoexpr_status kind, size_t start,
                  size_t end) {
    double result = 42;
    nanoexpr_error error;
    nanoexpr_status status = options ? nanoexpr_evaluate_with(input, strlen(input), options, &result, &error)
                                     : nanoexpr_evaluate(input, strlen(input), &result, &error);
    assert(status == kind);
    assert(error.kind == kind);
    assert(error.start == start);
    assert(error.end == end);
    assert(result == 42);
}

static void evaluates(void) {
    assert(value("2 * (3 + 4) ^ 2 / sqrt(16)") == 24.5);
    assert(value("7 % 4") == 3);
    assert(value("max(1, 5, 3)") == 5);
}

static void reports_errors(void) {
    fails(NULL, "1 + 4 / (2 - 2)", NANOEXPR_DIVISION_BY_ZERO, 4, 15);
    fails(NULL, "2 + * 3", NANOEXPR_UNEXPECTED_TOKEN, 4, 5);
    fails(NULL, "x + 1", NANOEXPR_UNKNOWN_VARIABLE, 0, 1);
    fails(NULL, "sqrt(-1)", NANOEXPR_DOMAIN, 0, 8);
    fails(NULL, "1 + \xff", NANOEXPR_UNEXPECTED_CHARACTER, 4, 5);
}

static void calls_callbacks(void) {
    sensors data = {21.5, 0};
    nanoexpr_options options = nanoexpr_default_options();
    options.variable = variable;
    options.function = function;
    options.user_data = &data;

    const char *input = "double(temperature) + sqrt(4)";
    double result = 0;
    assert(nanoexpr_evaluate_with(input, strlen(input), &options, &result, NULL) == NANOEXPR_OK);
    assert(result == 45);
    assert(data.calls == 2);

    fails(&options, "1 + double(1, 2)", NANOEXPR_ARGUMENT_COUNT, 4, 16);

    options.math = false;
    fails(&options, "sqrt(4)", NANOEXPR_UNKNOWN_FUNCTION, 0, 4);
}

static void applies_options(void) {
    nanoexpr_options options = nanoexpr_default_options();
    assert(options.variable == NULL);
    assert(options.function == NULL);
    assert(options.user_data == NULL);
    assert(options.math);
    assert(!options.percentage);
    assert(options.max_depth == NANOEXPR_DEFAULT_MAX_DEPTH);
    assert(!options.implicit_multiplication);

    options.percentage = true;
    double result = 0;
    assert(nanoexpr_evaluate_with("200 * 15%", 9, &options, &result, NULL) == NANOEXPR_OK);
    assert(result == 30);

    options.max_depth = 2;
    fails(&options, "((1))", NANOEXPR_TOO_DEEP, 2, 3);

    options = nanoexpr_default_options();
    fails(&options, "2(3 + 4)", NANOEXPR_UNEXPECTED_TOKEN, 1, 2);
    options.implicit_multiplication = true;
    assert(nanoexpr_evaluate_with("2(3 + 4)", 8, &options, &result, NULL) == NANOEXPR_OK);
    assert(result == 14);
}

static void rejects_null_pointers(void) {
    double result = 0;
    nanoexpr_error error;
    assert(nanoexpr_evaluate(NULL, 0, &result, &error) == NANOEXPR_INVALID_ARGUMENT);
    assert(error.kind == NANOEXPR_INVALID_ARGUMENT);
    assert(nanoexpr_evaluate("1", 1, NULL, NULL) == NANOEXPR_INVALID_ARGUMENT);
    assert(nanoexpr_evaluate_with("1", 1, NULL, &result, NULL) == NANOEXPR_INVALID_ARGUMENT);
}

static void describes_statuses(void) {
    assert(strcmp(nanoexpr_status_description(NANOEXPR_OK), "success") == 0);
    assert(strcmp(nanoexpr_status_description(NANOEXPR_DIVISION_BY_ZERO), "division by zero") == 0);
    assert(strcmp(nanoexpr_status_description(NANOEXPR_UNKNOWN_ERROR), "unknown error") == 0);
}

int main(void) {
    evaluates();
    reports_errors();
    calls_callbacks();
    applies_options();
    rejects_null_pointers();
    describes_statuses();
    puts("all tests passed");
    return 0;
}
