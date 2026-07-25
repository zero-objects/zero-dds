-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
--  Fixed sample encoders for the pure-Ada wire-core, exercising all three
--  extensibility kinds. The field sequences MUST match
--  endpoints/golden-gen/src/main.rs so the output is byte-identical to the Rust
--  goldens (and to endpoints/c / endpoints/ada Stage 1).

with Zerodds_Native_Wire; use Zerodds_Native_Wire;

package Native_Samples is

   --  @final SensorReading (primitives + string + sequence<octet>).
   function Encode_Final (Endian : Endianness) return Byte_Array;

   --  @appendable Outer { uint32 id; Inner one; sequence<Inner> many; string }
   --  with Inner { uint16 a; uint32 b } -- exercises DHEADER + nesting +
   --  sequence<non-primitive>.
   function Encode_Nested (Endian : Endianness) return Byte_Array;

   --  @mutable M { @id(10) uint32; @id(20) string; @id(30) uint16 } -- EMHEADER
   --  (LC4) members inside an appendable DHEADER frame.
   function Encode_Mutable (Endian : Endianness) return Byte_Array;

end Native_Samples;
