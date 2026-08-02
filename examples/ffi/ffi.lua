local ffi = require("ffi")
ffi.cdef[[
    int increment(int x);
]]
local lib = ffi.load("./ffi.dylib")

local x = 0
for _ = 1, 10000000 do
    x = lib.increment(x)
end
print(x)