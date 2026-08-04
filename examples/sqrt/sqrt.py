from math import sqrt

x = 0.0
for i in range(10000000):
    x += sqrt(i)
print(x)