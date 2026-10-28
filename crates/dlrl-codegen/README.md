# `zerodds-dlrl-codegen`

DLRL code generation — DDS 1.4 Annex B.4.

Turns the three DLRL pragmas — `DCPS_DATA_TYPE`, `DCPS_DATA_KEY` and
`DCPS_DLRL_RELATION` — into the object and home classes an application
programs against: one object class per data type, a getter per key
field, an accessor per relation.

## Targets

| target | mapping |
|---|---|
| C++ | DDS 1.4 §B.6 / §B.7, the specification's own PSM |
| Java | DDS 1.4 §B.8 |
| C# | no OMG PSM exists; follows the Java shape, and the generated file says so |
| TypeScript | no OMG PSM exists; interface plus implementing class |

## Why this emits text and not a `lang-cst` tree

Everything else here that generates code goes through
`zerodds-lang-cst`, because what it generates has to agree with a wire
format octet for octet. A DLRL object class is not on a wire — §B.6
describes the shape an application programs against, and no reader
parses it. There is nothing for a CST to keep consistent across
backends, and two of the four targets have no `lang-cst` dialect at all.

## The C# name rule

C# properties are `PascalCase`, so a key written `venue` in IDL becomes
`Venue`. That rename is why `[DlrlKey("venue")]` carries the IDL name
verbatim: the DCPS topic's key is `venue`, and a class that only knows
`Venue` cannot be matched back to it.
