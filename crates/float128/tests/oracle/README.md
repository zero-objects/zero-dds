# The binary128 oracle

`tests/oracle.rs` compares this crate against GCC's `__float128`, which
is a real binary128 with a printer and a parser nobody here wrote.

Three files:

| file | what it is |
|---|---|
| `patterns.txt` | 518 bit patterns and what this crate printed for each. Generated. |
| `quad_oracle.c` | the oracle program. Reads `patterns.txt`, writes the comparison. |
| `x86_64-gcc-14.2.txt` | what the oracle answered, measured on the Linux test host. |

`x86_64-gcc-14.2.txt` holds three columns per row: the pattern, what
libquadmath printed for it at 37 significant digits, and what
libquadmath's parser made of *our* text. Column two checks our parser
against their printer; column three checks our printer against their
parser.

## Re-measuring

Only on a host whose GCC has `__float128` and libquadmath — that is the
Linux test host, not an arm64 Mac, where the type does not exist.

```sh
cargo run -p zerodds-float128 --example emit_oracle_input \
    > crates/float128/tests/oracle/patterns.txt

scp crates/float128/tests/oracle/{quad_oracle.c,patterns.txt} <host>:/tmp/oracle/
ssh <host> 'cd /tmp/oracle && gcc -O2 -o quad_oracle quad_oracle.c -lquadmath \
    && ./quad_oracle < patterns.txt' > crates/float128/tests/oracle/x86_64-gcc-14.2.txt
```

Measured against `gcc (Debian 14.2.0-19) 14.2.0`, `Linux x86_64`.

## When a test here fails

`the_printer_still_writes_what_the_oracle_was_run_against` fails when the
printer changed. The recorded columns are then about text this crate no
longer produces, and the fix is to re-measure — never to edit the file
so that it agrees again. A recorded oracle that is edited to match the
code is no longer an oracle.
