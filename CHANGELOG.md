# Keel Changelog

## 0.4.0 (07/30/2026)
- Function arguments can now be typed with the syntax `arg: T`
- Structs and bools can now be passed and returned through FFI functions
- Anonymous functions & higher-order functions are finally here! Declare them with `fn (arg1, arg2, ...) {// code here}` anywhere as an expression
- `map` and `filter` methods have been added to the standard library
- The entire FFI VM logic has been simplified and optimized
- Keel now has a VS Code extension for syntax highlighting!
- Much better error messages (they will keep on improving)
  - Notably, invalid FFI types are now a compile-time error
- The `.len()` function now works with maps
- The VM has been optimized & recursion is faster
- The map type is now written `{K: V}` instead of `[K: V]`
- The module system is much more robust, and supports circular imports
- Multiple bugs have been fixed

## 0.3.0 (07/18/2026)
- Keel now ships a standard library, currently it's just Keel wrappers over native C: `std/math`, `std/time`, and `std/random`
- Libraries placed in the `libs/`folder next to the Keel executable can be imported from anywhere
- That standard library is now included in every binary release artifact and native libraries are compiled per-platform
- New Map type! The syntax is `{key: value, key1: value1}`, the type annotation is `[K: V]`. Available methods are `.get()` and `.insert()`.
- Union types are now valid (meaning `int | string` is now a valid type) but they're currently very experimental and aren't really useful
- Many errors are now *much* more useful, more helpful, and clearer. This includes some common compiler/parser errors. Please note that all errors haven't been upgraded yet, it's a process, so some errors will be cooler than others, but this'll improve over time. Some of those upgraded errors are also fixed and now report the error on the correct source file
- The compiler and the VM have been optimized, the compiler now uses a single unified compilation path, and literal compilation and register allocation are finally uniform. Concerning the VM, redundant bounds checks have been removed.
- Multiple bug fixes
- Also: licenses are now shipped in the artifacts