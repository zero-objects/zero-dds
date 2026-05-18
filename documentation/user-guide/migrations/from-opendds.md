# Migration from OpenDDS (legacy notes)

These migration notes are early drafts. See
[`07-migration/from-opendds.md`](../../07-migration/from-opendds.md)
for the current migration playbook.

## Quick orientation

OpenDDS uses **OMG-IDL 4.2** with a couple of vendor pragmas
(`#pragma DCPS_DATA_TYPE`, `#pragma DCPS_DATA_KEY`). Like RTI
Connext, these are modelled as a vendor grammar delta.

What works without changes (the standard OMG part):

- Modules, structs, unions, enums
- `@key` annotation as the modern equivalent to
  `#pragma DCPS_DATA_KEY`
- Standard type specs

Migration pattern: rewrite

```idl
#pragma DCPS_DATA_KEY "Topic field"
```

to a `@key` annotation on the field.

## References

- [OpenDDS](https://opendds.org/)
- [OpenDDS DCPS-type pragmas](https://opendds.readthedocs.io/en/latest/devguide/getting_started.html#defining-data-types-with-idl)
