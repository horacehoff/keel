for _ = 1, 200000 do
    local a = 0
    local b = 1
    local c = 0
    for i = 1, 45 do
        c = a + b
        a = b
        b = c
    end
end