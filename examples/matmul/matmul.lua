local n = 120
local matrix_a = {}
local matrix_b = {}
for i = 0, n - 1 do
    local row_a = {}
    local row_b = {}
    for j = 0, n - 1 do
        row_a[j] = (n + i + j) / n
        row_b[j] = n / (n + i + j)
    end
    matrix_a[i] = row_a
    matrix_b[i] = row_b
end
local matrix_c = {}
for i = 0, n - 1 do
    local row_c = {}
    for j = 0, n - 1 do
        local sum = 0.0
        for k = 0, n - 1 do
            sum = sum + matrix_a[i][k] * matrix_b[k][j]
        end
        row_c[j] = sum
    end
    matrix_c[i] = row_c
end
print(matrix_c[0][0])