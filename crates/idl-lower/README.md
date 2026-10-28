# `zerodds-idl-lower`

From the shape of an IDL file to the model.

Scopes and name binding, resolution, and the IDL rules the common AST
cannot express structurally: the recursion OMG IDL 4.2 §7.4.1.4.4.4.4
does not admit, a declaration that reuses the name of its own scope
(§7.5.2), the member-id rules, the annotations of DDS-XTypes 1.3 §7.3.1.2.

Where the products disagree, the answer is a **named axis** rather than
a choice: `KeyOnUnionDiscriminator`, `HashedIdCounter`,
`HashedNameSpelling`, `NameOfTheEnclosingScope`. Each has the clause's
reading as its default and each vendor's measured reading as the other
value, so `--dialect rti` and a build of the same file reach the same
model by the same route.

This crate computes **no layout**. What the model means on the wire is
`zerodds-ast-layout`'s answer.
