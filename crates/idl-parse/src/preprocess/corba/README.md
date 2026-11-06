# The CORBA module

`orb.idl` and the `CORBA_*.idl` files it includes are the machine-readable
IDL of the CORBA module as the OMG publishes it with CORBA 3.3
(`https://www.omg.org/spec/CORBA/20120301/`), with two transcription
changes listed below. OMG IDL 4.2
§7.4.6.4.7 and CORBA 3.3 part 1 §7.19: "The file orb.idl contains the
IDL definitions for the CORBA module. Except for CORBA::TypeCode, the
file orb.idl must be included in IDL files that use names defined in the
CORBA module. IDL files that use CORBA::TypeCode may obtain its
definition by including either the file orb.idl or the file
TypeCode.idl." `TypeCode.idl` is this implementation's, which the clause
leaves "implementation dependent".

Transcription changes, both needed for the files to be IDL 4.2:

- `typeprefix CORBA "omg.org"` in `orb.idl` and
  `CORBA_InterfaceRepository.idl` ends with `;`, as IDL 4.2 rule (114)
  and the definitions that use it require (`<type_prefix_dcl> ";"`);
  `CORBA_TypeCode.idl` already has it.
- `orb.idl` declares `native AbstractBase;`, which CORBA 3.3 part 1 §8.2
  and §10.2 define in the CORBA module ("Abstract interfaces implicitly
  inherit from CORBA::AbstractBase. This type is defined as native.") and
  `CORBA_Stream.idl` uses, but the published `orb.idl` does not declare.

The preprocessor resolves `#include <orb.idl>`, `#include <TypeCode.idl>`
and the files `orb.idl` includes to these texts when the caller's loader
does not find a file of that name (`super::builtin_include`), so a file
of the caller's own takes precedence.

The files are covered by the OMG specification license, which grants the
right to use the specification to create implementations of it.
