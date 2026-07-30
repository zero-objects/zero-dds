-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
--  Object-Ada (post-Object) async endpoint (ADR 0013). The MODERN, additive
--  Ada variant: genuine object orientation -- interfaces, tagged types and
--  dispatching -- for teams on an Ada 2012+ toolchain. (The conservative
--  pre-Object Ada 83 variant, and the procedural Stage 1/2, stay untouched.)
--  Object-Ada DDS endpoints are a defense differentiator almost nobody offers.
--
--  Event-driven: an `Async_Reader` drives a dispatching `Transport` and hands
--  each decoded sample body to a dispatching `Sample_Handler`. No threads, no
--  heap; drive it from any scheduler via `Poll` / `Run`.

with Interfaces;
with Zerodds_Native_Wire; use Zerodds_Native_Wire;

package Zerodds_Native_Endpoint is

   type Recv_Status is (Recv_Ok, Recv_Again, Recv_Error);

   --  --- OOP transport abstraction (the single integration point) ---
   type Transport is limited interface;

   --  Deliver one complete frame to the peer; Ok is False on failure.
   procedure Deliver
     (T : in out Transport; Frame : Byte_Array; Ok : out Boolean) is abstract;

   --  Receive one frame into Buf (Buf'First .. Last). Recv_Again when none.
   procedure Receive
     (T      : in out Transport;
      Buf    : out Byte_Array;
      Last   : out Natural;
      Status : out Recv_Status) is abstract;

   --  --- OOP sample callback ---
   type Sample_Handler is limited interface;
   procedure On_Sample
     (H : in out Sample_Handler; Sample_Body : Byte_Array) is abstract;

   --  --- event-driven reader ---
   type Async_Reader
     (Trans   : not null access Transport'Class;
      Handler : not null access Sample_Handler'Class)
   is tagged limited private;

   --  Dispatch one ready frame (decode XRCE envelope -> Handler.On_Sample).
   procedure Poll (R : in out Async_Reader; Dispatched : out Boolean);

   --  Drain the transport (dispatch until Recv_Again). Returns the count.
   procedure Run (R : in out Async_Reader; Count : out Natural);

   --  --- fire-and-forget writer ---
   type Async_Writer (Trans : not null access Transport'Class) is
     tagged limited record
      Session : Byte := 16#80#;
      Stream  : Byte := 16#01#;
      Seq     : Interfaces.Unsigned_16 := 1;
   end record;

   procedure Write (W : in out Async_Writer; Sample : Byte_Array; Ok : out Boolean);

private

   type Async_Reader
     (Trans   : not null access Transport'Class;
      Handler : not null access Sample_Handler'Class)
   is tagged limited record
      Rx : Byte_Array (0 .. Max_Buffer - 1) := [others => 0];
   end record;

end Zerodds_Native_Endpoint;
