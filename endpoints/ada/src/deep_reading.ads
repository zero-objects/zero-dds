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

   --  A body the frame store cannot carry.
   --
   --  `Frame` writes the 8-byte envelope and then `Body_FS.Len` octets into a
   --  `Frame_Store`, whose `Data` is `Max_Frame` long. There was no bound: a
   --  body of `Max_Frame - 7` octets or more indexed past the end of the array,
   --  which is a Constraint_Error while checks are on and a write into the
   --  neighbouring `Len` field under `-gnatp`. The bound is stated instead of
   --  relied upon, and it is the store's and not the wire's -- `Max_Frame` is
   --  256 here, well below the 65535 the submessage_length field describes.
   Body_Too_Large : exception;

   --  A wire-core call answered with something other than ZDW_OK.
   --
   --  Every one of the eight calls in the body used to assign its result
   --  to a variable named `Ign` and read it nowhere. A `Put_String` the
   --  core refused left `W.Len` short, `Marshal` took that length as the
   --  body, and `Frame` wrapped it in an envelope whose length field
   --  agreed with itself -- a datagram that went out well-formed and
   --  carried less than the sample. Reading, a `Get_String` that stopped
   --  early left `R.Label` holding whatever the store had.
   --
   --  This binding is `Interfaces.C` imports over `endpoints/c` and has
   --  no wire code of its own, so it is the C core's error discipline
   --  that has to reach the caller; discarding it here removed the only
   --  report there was.
   Wire_Error : exception;

   --  Wire size of the BaseObjectRequest preceding the sample (7.7.8).
   Base_Size : constant := 4;

   --  XRCE WRITE_DATA framing: 8-byte header + BaseObjectRequest (7.7.8:
   --  request_id = `Seq`, object_id = `Object_Id`, the target DataWriter) +
   --  body / strip it back off. Raises Body_Too_Large when the body does not
   --  fit a Frame_Store.
   function Frame   (Seq : Interfaces.Unsigned_16;
                     Object_Id : Interfaces.Unsigned_16;
                     Body_FS : Frame_Store) return Frame_Store;
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
