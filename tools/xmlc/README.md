# `zerodds-xmlc`

Read an OMG DDS-XML 1.0 document and say what is in it, or what is wrong
with it. It never opens a socket: a DDS-XML file describes a system, and
this reads the description.

```
zerodds-xmlc validate FILE...   parse every library; report what failed
zerodds-xmlc types    FILE...   the type definitions, by library
zerodds-xmlc render   FILE...   the deployment: domains and participants
```

## Options

* `--skip-unknown`: leave out elements and attributes the DDS-XML schemas
  do not define and list each with its path on stderr. Without it the
  first one is a failure.
* `--vendor-forms`: read the forms other DDS products write (`<topic_filter>`
  as an element, `qos_profile_ref`, `<participant_qos>`, IDL type names such
  as `long`) and list each with its path on stderr. Without it the first
  one is a failure that names the DDS-XML 1.0 form.

## Why `validate` parses all five libraries

A document may hold types, QoS profiles, domains, participants and
applications, and any of them may be absent. Stopping at the first
library that parses would call a file valid on the strength of whichever
part came first, so all five are parsed and every failure is reported —
the caller learns about all of them in one run instead of one per fix.
An absent library is not a failure.

## Exit codes

`0` nothing to report · `1` a library failed to parse · `2` the command
line was wrong · `3` a file could not be read.
