# Cyclone-DDS fixture licenses

The IDL files in this directory are **not** copied from the Eclipse Cyclone
DDS repo. They are hand-maintained representatives of typical
Cyclone-DDS IDL constructs (OMG standard IDL 4.2 + standard annotations
@final/@appendable/@key) for grammar-coverage tests.

Cyclone DDS uses **no** vendor-specific grammar extensions —
these fixtures parse with the base grammar (`IDL_42`) without a delta.

License: the same license as the surrounding `zerodds-idl` crate.
