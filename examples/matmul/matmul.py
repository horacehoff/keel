n = 120
matrix_a = []
matrix_b = []
for i in range(n):
    row_a = []
    row_b = []
    for j in range(n):
        row_a.append(float(n + i + j) / float(n))
        row_b.append(float(n) / float(n + i + j))
    matrix_a.append(row_a)
    matrix_b.append(row_b)
matrix_c = []
for i in range(n):
    row_c = []
    for j in range(n):
        sum = 0.0
        for k in range(n):
            sum += matrix_a[i][k] * matrix_b[k][j]
        row_c.append(sum)
    matrix_c.append(row_c)
print(matrix_c[0][0])
