#include <stdio.h>
#include <string.h>

#include "nanoexpr.h"

typedef struct variable {
    const char *name;
    double value;
} variable;

static const variable variables[] = {
    {"price", 80},
    {"quantity", 3},
};

static bool equals(const char *name, size_t name_length, const char *expected) {
    return strlen(expected) == name_length && memcmp(name, expected, name_length) == 0;
}

static bool resolve(void *user_data, const char *name, size_t name_length, double *value) {
    (void)user_data;
    for (size_t index = 0; index < sizeof variables / sizeof variables[0]; index++) {
        if (equals(name, name_length, variables[index].name)) {
            *value = variables[index].value;
            return true;
        }
    }
    return false;
}

static nanoexpr_status call(void *user_data, const char *name, size_t name_length, const double *arguments,
                            size_t argument_count, double *result) {
    (void)user_data;
    if (equals(name, name_length, "discount")) {
        if (argument_count != 2) {
            return NANOEXPR_ARGUMENT_COUNT;
        }
        if (arguments[1] < 0 || arguments[1] > 100) {
            return NANOEXPR_DOMAIN;
        }
        *result = arguments[0] * (1 - arguments[1] / 100);
        return NANOEXPR_OK;
    }
    return NANOEXPR_UNKNOWN_FUNCTION;
}

static int evaluate(const nanoexpr_options *options, const char *input, size_t length) {
    double result;
    nanoexpr_error error;
    if (nanoexpr_evaluate_with(input, length, options, &result, &error) == NANOEXPR_OK) {
        printf("%g\n", result);
        return 0;
    }
    fprintf(stderr, "%.*s\n%*s", (int)length, input, (int)error.start, "");
    for (size_t index = error.start; index < error.end || index == error.start; index++) {
        fputc('^', stderr);
    }
    fprintf(stderr, " %s\n", nanoexpr_status_description(error.kind));
    return 1;
}

int main(int argc, char **argv) {
    nanoexpr_options options = nanoexpr_default_options();
    options.variable = resolve;
    options.function = call;
    options.percentage = true;

    int failures = 0;
    if (argc > 1) {
        for (int index = 1; index < argc; index++) {
            failures += evaluate(&options, argv[index], strlen(argv[index]));
        }
        return failures ? 1 : 0;
    }

    char line[256];
    while (fgets(line, sizeof line, stdin)) {
        size_t length = strcspn(line, "\r\n");
        if (length > 0) {
            failures += evaluate(&options, line, length);
        }
    }
    return failures ? 1 : 0;
}
