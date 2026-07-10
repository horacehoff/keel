local function fib(n)
    if n <= 1 then return n
    else return fib(n - 1) + fib(n - 2) end
end

local function main()
    print(fib(10))
    print(fib(15))
    print(fib(20))
    print(fib(25))
    print(fib(30))
    print(fib(33))
end

main()