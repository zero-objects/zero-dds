-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
--  Support unit for the Ada deep examples: a realistic telemetry type
--  `Reading { Id, Value, Label }` with its XCDR2 codec (over the Stage-1 Zdw
--  FFI wire-core), XRCE WRITE_DATA framing, an in-memory protected mailbox
--  transport, and a reader task -- the idiomatic Ada concurrency primitives.

with Interfaces;
with Interfaces.C;

package Deep_Reading is

   use type Interfaces.Unsigned_16;
   use type Interfaces.C.size_t;

   Label_Cap : constant := 32;
   Max_Frame : constant := 256;

   subtype Byte is Interfaces.Unsigned_8;
   type Bytes is array (Natural range <>) of aliased Byte;

   type Reading is record
      Id    : Interfaces.C.unsigned_long := 0;                          -- uint32
      Value : Interfaces.C.C_float       := 0.0;                        -- float
      Label : Interfaces.C.char_array (0 .. Label_Cap - 1)
                := [others => Interfaces.C.nul];                        -- string
   end record;

   --  A bounded byte buffer with a used length (frames and bodies alike).
   type Frame_Store is record
      Data : Bytes (0 .. Max_Frame - 1) := [others => 0];
      Len  : Natural := 0;
   end record;

   procedure Set_Label (R : in out Reading; S : String);
   function  Label_Str (R : Reading) return String;

   --  Codec: body bytes only (no XRCE header).
   function Marshal   (R : Reading) return Frame_Store;
   function Unmarshal (Data : Bytes) return Reading;

   --  XRCE WRITE_DATA framing: 8-byte header + body / strip it back off.
   function Frame   (Seq : Interfaces.Unsigned_16; Body_FS : Frame_Store)
                     return Frame_Store;
   function Deframe (F : Frame_Store) return Frame_Store;  -- Len=0 if invalid

   --  Formatting helpers (no scientific notation, lower-case hex).
   function Hex    (V : Interfaces.C.unsigned_long) return String;
   function F1     (V : Interfaces.C.C_float) return String;
   function Digit2 (N : Natural) return String;  -- "00".."99"

   Queue_Cap : constant := 32;
   type Frame_Queue is array (0 .. Queue_Cap - 1) of Frame_Store;

   --  In-memory FIFO transport, an idiomatic Ada protected object.
   protected type Mailbox is
      procedure Deliver (F : Frame_Store);
      procedure Try_Receive (F : out Frame_Store; Got : out Boolean);
      entry     Receive (F : out Frame_Store);
   private
      Q     : Frame_Queue;
      Head  : Natural := 0;
      Tail  : Natural := 0;
      Count : Natural := 0;
   end Mailbox;

   --  Background reader: pulls frames from Transport, deframes, and forwards
   --  the decoded body into Inbox. Exits after forwarding N samples. Anonymous
   --  access discriminants so the mains can pass their local mailboxes.
   task type Reader_Task
     (Transport : access Mailbox; Inbox : access Mailbox; N : Natural);

end Deep_Reading;
