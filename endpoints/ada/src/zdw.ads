-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
--  Ada Stage 1 binding of the ZeroDDS endpoint wire-core (ADR 0013). Thin
--  Interfaces.C / pragma Import bindings over endpoints/c (zerodds_wire.c):
--  the exact same C89 XCDR primitives, so the Ada endpoint produces byte
--  output identical to the Rust core and the C SDK. No re-implementation --
--  Stage 2 (endpoints/ada-native) is the pure-Ada wire.

with Interfaces.C;
with System;

package Zdw is
   pragma Preelaborate;

   use Interfaces.C;

   --  Wire byte order (the XCDR encapsulation byte-order flag).
   ZDW_LE : constant int := 0;
   ZDW_BE : constant int := 1;

   --  Result codes.
   ZDW_OK        : constant int := 0;
   ZDW_E_Overrun : constant int := 1;
   ZDW_E_Invalid : constant int := 2;

   --  64-bit value as two 32-bit halves (portable C89, no stdint). Passed to
   --  the C API by value, so it must be passed by copy (GNAT would otherwise
   --  pass a C-convention record by reference and the halves would land in the
   --  wrong registers).
   type Zdw_U64 is record
      Hi : unsigned_long := 0;
      Lo : unsigned_long := 0;
   end record
     with Convention => C_Pass_By_Copy;

   --  Write cursor over a caller-owned buffer. Field layout mirrors the C
   --  struct exactly so pointers to it are shared across the FFI boundary.
   type Zdw_Writer is record
      Buf       : System.Address := System.Null_Address;
      Cap       : size_t         := 0;
      Len       : size_t         := 0;
      Endian    : int            := ZDW_LE;
      Max_Align : int            := 0;
      Error     : int            := ZDW_OK;
   end record
     with Convention => C;

   --  Read cursor over a caller-owned buffer.
   type Zdw_Reader is record
      Buf       : System.Address := System.Null_Address;
      Len       : size_t         := 0;
      Pos       : size_t         := 0;
      Endian    : int            := ZDW_LE;
      Max_Align : int            := 0;
      Error     : int            := ZDW_OK;
   end record
     with Convention => C;

   --  --- lifecycle ---

   procedure Writer_Init
     (W : access Zdw_Writer; Buf : System.Address; Cap : size_t; Endian : int)
     with Import, Convention => C, External_Name => "zdw_writer_init";

   procedure Reader_Init
     (R : access Zdw_Reader; Buf : System.Address; Len : size_t; Endian : int)
     with Import, Convention => C, External_Name => "zdw_reader_init";

   --  --- writer primitives (return ZDW_OK or set + return the error) ---

   function Put_U8 (W : access Zdw_Writer; V : unsigned_char) return int
     with Import, Convention => C, External_Name => "zdw_put_u8";

   function Put_U16 (W : access Zdw_Writer; V : unsigned) return int
     with Import, Convention => C, External_Name => "zdw_put_u16";

   function Put_U32 (W : access Zdw_Writer; V : unsigned_long) return int
     with Import, Convention => C, External_Name => "zdw_put_u32";

   function Put_U64 (W : access Zdw_Writer; V : Zdw_U64) return int
     with Import, Convention => C, External_Name => "zdw_put_u64";

   function Put_Bool (W : access Zdw_Writer; V : int) return int
     with Import, Convention => C, External_Name => "zdw_put_bool";

   function Put_F32 (W : access Zdw_Writer; V : C_float) return int
     with Import, Convention => C, External_Name => "zdw_put_f32";

   function Put_F64 (W : access Zdw_Writer; V : double) return int
     with Import, Convention => C, External_Name => "zdw_put_f64";

   --  CDR string: u32 length (incl. NUL) + bytes + one NUL. `S` is a
   --  NUL-terminated C string (pass System'Address of a char_array).
   function Put_String (W : access Zdw_Writer; S : System.Address) return int
     with Import, Convention => C, External_Name => "zdw_put_string";

   --  sequence<octet>: u32 length + raw bytes.
   function Put_Seq_U8
     (W : access Zdw_Writer; Data : System.Address; N : size_t) return int
     with Import, Convention => C, External_Name => "zdw_put_seq_u8";

   --  --- reader primitives ---

   function Get_U8 (R : access Zdw_Reader; Out_V : access unsigned_char) return int
     with Import, Convention => C, External_Name => "zdw_get_u8";

   function Get_U16 (R : access Zdw_Reader; Out_V : access unsigned) return int
     with Import, Convention => C, External_Name => "zdw_get_u16";

   function Get_U32 (R : access Zdw_Reader; Out_V : access unsigned_long) return int
     with Import, Convention => C, External_Name => "zdw_get_u32";

   function Get_U64 (R : access Zdw_Reader; Out_V : access Zdw_U64) return int
     with Import, Convention => C, External_Name => "zdw_get_u64";

   function Get_F32 (R : access Zdw_Reader; Out_V : access C_float) return int
     with Import, Convention => C, External_Name => "zdw_get_f32";

   --  Reads a CDR string into `Out` (Cap bytes incl. NUL).
   function Get_String
     (R : access Zdw_Reader; Out_S : System.Address; Cap : size_t) return int
     with Import, Convention => C, External_Name => "zdw_get_string";

   --  sequence<octet>: reads the u32 length then min(len,cap) bytes.
   function Get_Seq_U8
     (R     : access Zdw_Reader;
      Out_D : System.Address;
      Cap   : size_t;
      N     : access size_t) return int
     with Import, Convention => C, External_Name => "zdw_get_seq_u8";

   --  --- helper ---

   function U64_From_UL (V : unsigned_long) return Zdw_U64
     with Import, Convention => C, External_Name => "zdw_u64_from_ul";

end Zdw;
