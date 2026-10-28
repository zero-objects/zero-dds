# `zerodds-idl-cst`

The syntax tree that keeps everything it read.

A lossless concrete syntax tree for OMG IDL 4.2: tokens carry their
trivia, and a green/red tree reproduces its input byte for byte —
including for a file that fails to parse, because error recovery has to
hand back what it could not read.

`SyntaxSpans` narrows a span to the text it covers. A node's own span
includes its leading trivia, which is right for reproducing the file and
wrong for pointing a diagnostic at a declaration; the two questions have
two answers and both live here.

This crate resolves **no names and knows no types**. It is the shape of
the file, not its meaning; `zerodds-idl-lower` supplies that.
