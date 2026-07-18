---
icon: lucide/dice-5
---
# Random library

!!! warning

    This is highly subject to change.

Import this library with `import "std/random.kl";` at the top-level.

*[at the top-level]: Outside of any function.

## Random
`random::random() -> float`<br/>
Returns a random float within \[0;1\[.

## RandomRange
`random::random_range(min: float, max: float) -> float`<br/>
Returns a random float within the given extrema.

## RandomInt
`random::random_int() -> int`<br/>
Returns a random int.

## RandomIntRange
`random::random_int_range(min: int, max: int) -> int`<br/>
Returns a random int within the given extrema.

## Seed
`random::seed(seed: int)`<br/>
Seeds the RNG with `seed`.