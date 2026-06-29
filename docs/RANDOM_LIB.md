# Random
Import with `import "std/random.kl";`.

> `arg: T` represents a function argument of type T.

## Random
`random::random() -> Float`
Returns a random float within \[0;1\[.

## RandomRange
`random::random_range(min: Float, max: Float)`
Returns a random float within the given extrema.

## RandomInt
`random::random_int() -> Integer`\
Returns a random int.

## RandomIntRange
`random::random_int_range(min: Integer, max: Integer) -> Integer`\
Returns a random int within the given extrema.

## Seed
`random::seed(seed: Integer)`\
Seeds the RNG with `seed`.