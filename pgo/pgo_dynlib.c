int pgo_add_int(int a, int b) {
    return a + b;
}

double pgo_add_float(double a, double b) {
    return a + b;
}

int pgo_all_types(
    int i,
    double f,
    const char *s,
    const int *ints,
    const double *floats,
    const char **strings,
    int **matrix,
    int len
) {
    return i
        + (int)f
        + ints[0]
        + (int)floats[0]
        + matrix[0][0]
        + len;
}