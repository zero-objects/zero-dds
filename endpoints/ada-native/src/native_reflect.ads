-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
--  Pure-Ada reflective (descriptor-driven) codec (ADR 0013 Stage 2), mirroring
--  endpoints/c/src/zerodds_reflect.c. A `Dyn_Struct` is a runtime value tree
--  (extensibility + a field list of tagged values, recursing into nested
--  structs and sequence<struct>). Encoding walks the tree and drives the same
--  wire-core, so the reflective path is byte-identical to the fixed codegen
--  path and to the Rust core.

with Interfaces;          use Interfaces;
with Zerodds_Native_Wire; use Zerodds_Native_Wire;

package Native_Reflect is

   type Field_Kind is
     (K_U8, K_U16, K_U32, K_U64, K_F32, K_F64, K_Bool,
      K_String, K_Seq_U8, K_Nested, K_Seq_Struct);

   type Extensibility is (X_Final, X_Appendable, X_Mutable);

   type Dyn_Struct;
   type Struct_Access is access constant Dyn_Struct;

   type Struct_Ptr_Array is array (Natural range <>) of Struct_Access;
   type Struct_Ptr_Array_Access is access constant Struct_Ptr_Array;
   type String_Access is access constant String;
   type Bytes_Access is access constant Byte_Array;

   type Dyn_Field (Kind : Field_Kind := K_U32) is record
      case Kind is
         when K_U8         => U8v     : Byte;
         when K_U16        => U16v    : Unsigned_16;
         when K_U32        => U32v    : Unsigned_32;
         when K_U64        => U64v    : Unsigned_64;
         when K_F32        => F32v    : IEEE_Float_32;
         when K_F64        => F64v    : IEEE_Float_64;
         when K_Bool       => Boolv   : Boolean;
         when K_String     => Strv    : String_Access;
         when K_Seq_U8     => Bytesv  : Bytes_Access;
         when K_Nested     => Nestedv : Struct_Access;
         when K_Seq_Struct => Elemsv  : Struct_Ptr_Array_Access;
      end case;
   end record;

   type Field_Array is array (Natural range <>) of Dyn_Field;
   type Field_Array_Access is access constant Field_Array;

   type Id_Array is array (Natural range <>) of Unsigned_32;
   type Id_Array_Access is access constant Id_Array;

   type Dyn_Struct is record
      Ext    : Extensibility;
      Fields : Field_Array_Access;
      Ids    : Id_Array_Access := null;  -- per-field member id (Mutable only)
   end record;

   --  Reflectively encode `S` into `W` (recursing through nested structs and
   --  sequence<struct>).
   procedure Encode_Struct (W : in out Writer; S : Dyn_Struct);

end Native_Reflect;
