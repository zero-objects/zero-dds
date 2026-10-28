# The C++ and Java columns against the Rust column

Written by `crates/wire-probe/tests/reference_columns.rs`, which is
the test that produced it. A record, not an input.

## zerodds-cpp

| measurement | slots |
|---|---|
| wrote the Rust column's octets | 101000 |
| read them back as the sample | 49900 |
| read them back as the Rust column did, which is not the sample | 60 |
| refused where the Rust column refused | 25440 |
| refused because the language has no form for the case (`NO_LANGUAGE_FORM` in the test) | 0 |
| disagreed | 0 |
| compared | 176400 |

## zerodds-java

| measurement | slots |
|---|---|
| wrote the Rust column's octets | 100920 |
| read them back as the sample | 49860 |
| read them back as the Rust column did, which is not the sample | 60 |
| refused where the Rust column refused | 25140 |
| refused because the language has no form for the case (`NO_LANGUAGE_FORM` in the test) | 420 |
| disagreed | 0 |
| compared | 176400 |

