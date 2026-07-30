-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors

with Interfaces; use Interfaces;
with Native_Framing;

package body Zerodds_Native_Endpoint is

   procedure Poll (R : in out Async_Reader; Dispatched : out Boolean) is
      Last       : Natural;
      Status     : Recv_Status;
      Body_First : Natural;
      Body_Last  : Integer;
      Valid      : Boolean;
   begin
      Dispatched := False;
      R.Trans.Receive (R.Rx, Last, Status);           -- dispatching
      if Status /= Recv_Ok then
         return;
      end if;
      Native_Framing.Xrce_Read_Frame
        (R.Rx (0 .. Last), Body_First, Body_Last, Valid);
      if not Valid then
         return;
      end if;
      R.Handler.On_Sample (R.Rx (Body_First .. Body_Last)); -- dispatching
      Dispatched := True;
   end Poll;

   procedure Run (R : in out Async_Reader; Count : out Natural) is
      Did : Boolean;
   begin
      Count := 0;
      loop
         Poll (R, Did);
         exit when not Did;
         Count := Count + 1;
      end loop;
   end Run;

   procedure Write
     (W : in out Async_Writer; Sample : Byte_Array; Ok : out Boolean)
   is
      Frame : constant Byte_Array :=
        Native_Framing.Xrce_Write_Frame (W.Session, W.Stream, W.Seq, Sample);
   begin
      W.Seq := W.Seq + 1;
      W.Trans.Deliver (Frame, Ok);                    -- dispatching
   end Write;

end Zerodds_Native_Endpoint;
