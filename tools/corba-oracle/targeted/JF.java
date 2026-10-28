import org.jacorb.orb.CDROutputStream;
import java.math.BigDecimal;
public class JF { public static void main(String[] a) throws Exception {
  F s=new F(); s.head=(byte)0x11; s.m=new BigDecimal("123.456"); s.p=new BigDecimal("-1.25");
  s.b=new BigDecimal("1234567890123456789012345678901"); s.c=Colour.BLUE; s.t=true; s.off=false; s.tail=(byte)0xAB;
  CDROutputStream o=new CDROutputStream();
  try { FHelper.write(o,s); } catch (Throwable t){ System.out.println("jacorb refused: "+t); return; }
  byte[] b=o.getBufferCopy(); StringBuilder sb=new StringBuilder(); for(byte x:b) sb.append(String.format("%02x",x));
  System.out.println("jacorb="+sb+" ("+b.length+")");
}}
