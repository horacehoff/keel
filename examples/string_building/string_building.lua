local s = {}
for i = 1, 100000 do
    s[i] = "Hello, world!"
end
local s = table.concat(s)
print(#s)