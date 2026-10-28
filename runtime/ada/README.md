# runtime/ada — the Ada wire runtime

DDS-XTypes 1.3 §7.4 implemented from the clause, in Ada 2012 — a
further reading of the specification beside `crates/wire`, `runtime/c`,
`runtime/d` and the rest, never a binding to any of them. The generated
codecs of `crates/lang-rules/src/emit/ada.rs` call this package and
nothing else; no FFI is involved anywhere in the column.

## Layout

- `src/zd_wire.ads`, `src/zd_wire.adb` — the package. GNAT derives unit
  names from file names, so these two names are fixed.
- `tests/zd_wire_selftest.adb` — fixed-octet checks of the writer and
  the reader.
- `zd_wire.gpr` — the GNAT project for building the runtime and its
  self-test on their own: `gprbuild -P zd_wire.gpr`.

The probe driver (`tools/wireprobe/wireprobe-zerodds-ada`) does not use
the project file: it hands `src/zd_wire.adb` to `gnatmake` beside the
generated probe, the same way the D driver hands `zdwire.d` to `ldc2`.

## Decisions

- **Ada 2012, `Interfaces` only.** Every width is
  `Interfaces.Integer_N` / `Unsigned_N` / `IEEE_Float_N`; `Integer` and
  `Float` have implementation-defined widths and appear nowhere.
- **Strings are `Ada.Strings.Unbounded.Unbounded_String`, holding
  UTF-8.** The UTF-16 conversion for `wstring` happens here, which makes
  §7.4.1.1.2's surrogate rule this runtime's own reading.
- **A malformed stream raises `Wire_Error`** and unwinds to the probe's
  harness (exit 1); a refusal about the *type* sets the reader's status
  and the generated decoder walks on — the same split every sibling
  runtime keeps.
- **`Finish_Pid_Read` measures the value against its start** (the
  header's `Value_At`) and validates the declared length against both
  the exact and the padded reading; it aligns to four only under a
  framing whose writer wrote that padding (`Pad_After_Value`). Under
  Fast DDS's framing the next header aligns itself and a final
  parameter ends the stream where its value does.
- **binary128 rests.** `Write_F128` and `Read_F128` raise `Wire_Error`
  by name: an IDL `long double` needs a correctly rounded decimal↔
  binary128 conversion over a big integer, and that work rests with the
  f128 arc rather than being approximated here.
- **No `-gnatwa`.** The shared codec walk declares locals a dialect may
  leave unread (`zd_sent` under a framing with no sentinel, `zd_tc` in
  a decoder with no bound to miss); GNAT's default warning set does not
  flag them and the probes are compiled with defaults on purpose.
