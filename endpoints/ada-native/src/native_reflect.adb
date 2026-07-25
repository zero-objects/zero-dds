-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors

package body Native_Reflect is

   procedure Encode_Field (W : in out Writer; F : Dyn_Field);

   procedure Encode_Body (W : in out Writer; S : Dyn_Struct) is
   begin
      for I in S.Fields'Range loop
         if S.Ext = X_Mutable then
            declare
               Member_Id : constant Unsigned_32 :=
                 (if S.Ids /= null and then I in S.Ids'Range
                  then S.Ids (I) else Unsigned_32 (I));
               Body_Start : Natural;
            begin
               EMHeader_Begin (W, Member_Id, False, Body_Start);
               Encode_Field (W, S.Fields (I));
               EMHeader_End (W, Body_Start);
            end;
         else
            Encode_Field (W, S.Fields (I));
         end if;
      end loop;
   end Encode_Body;

   procedure Encode_Struct (W : in out Writer; S : Dyn_Struct) is
      Frame : Natural;
   begin
      case S.Ext is
         when X_Final =>
            Encode_Body (W, S);
         when X_Appendable | X_Mutable =>
            --  Both carry a DHEADER frame; Mutable additionally EMHEADER-tags
            --  each member inside Encode_Body.
            DHeader_Begin (W, Frame);
            Encode_Body (W, S);
            DHeader_End (W, Frame);
      end case;
   end Encode_Struct;

   procedure Encode_Field (W : in out Writer; F : Dyn_Field) is
   begin
      case F.Kind is
         when K_U8     => Put_U8 (W, F.U8v);
         when K_U16    => Put_U16 (W, F.U16v);
         when K_U32    => Put_U32 (W, F.U32v);
         when K_U64    => Put_U64 (W, F.U64v);
         when K_F32    => Put_F32 (W, F.F32v);
         when K_F64    => Put_F64 (W, F.F64v);
         when K_Bool   => Put_Bool (W, F.Boolv);
         when K_String => Put_String (W, F.Strv.all);
         when K_Seq_U8 => Put_Seq_U8 (W, F.Bytesv.all);
         when K_Nested => Encode_Struct (W, F.Nestedv.all);
         when K_Seq_Struct =>
            --  sequence<struct>: a collection DHEADER (u32 body length) wrapping
            --  a u32 count + each element, built in a 4-aligned sub-writer.
            declare
               Sub : Writer;
            begin
               Init (Sub, W.Endian);
               Put_U32 (Sub, Unsigned_32 (F.Elemsv.all'Length));
               for E of F.Elemsv.all loop
                  Encode_Struct (Sub, E.all);
               end loop;
               Put_U32 (W, Unsigned_32 (Sub.Len));
               Put_Bytes (W, Bytes (Sub));
            end;
      end case;
   end Encode_Field;

end Native_Reflect;
