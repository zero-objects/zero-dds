# RTI fixture licenses

The IDL files in this directory are **not** copied from the RTI Connext SDK.
They are hand-maintained representatives of typical RTI Connext IDL
constructs for grammar/delta tests, based on the published
RTI doc syntax (https://community.rti.com/static/documentation/connext-dds/).

RTI-specific constructs used:
- `keylist <Type> (<field>+);` — alternative to the `@key` annotation
- (prospectively) `#pragma keylist <Type> <field>...` — preprocessor form

License: the same license as the surrounding `zerodds-idl` crate (workspace default).
No RTI-proprietary texts included.
