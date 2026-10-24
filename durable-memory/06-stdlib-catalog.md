# Standard library catalog

The numeric catalog is exposed by `dslvm::stdlib::register_math` and the
`stdlib/math.schema-fragment.json` fragment. Each of `math_abs`, `math_floor`,
`math_ceil`, `math_round`, and `math_sqrt` has one int and one float overload;
`math_min`, `math_max`, and `math_pow` have matching int,int and float,float
overloads.

Integer arithmetic wraps in signed 64-bit two's-complement space. Integer
absolute value wraps for `i64::MIN`; integer floor, ceil, and round are
identity; integer power uses exponentiation by squaring, with `0^0 == 1`, and
rejects negative exponents with `math_pow: integer exponent must be
nonnegative`. Integer square root is the exact floor root and rejects negative
arguments with `math_sqrt: integer argument must be nonnegative`.

Float floor, ceil, and round use IEEE behavior with ties away from zero.
Minimum and maximum propagate NaN and return the first argument on ties,
including signed zero. Float power handles exponent zero and base one before
NaN propagation, and otherwise uses native binary64 power. Float square root
preserves negative zero and returns NaN for negative finite inputs.
