import org.jacorb.orb.CDROutputStream;
public class JLd { public static void main(String[] a) throws Exception {
  S s = new S(); s.head=(byte)0x11; s.d=1.5; s.tail=(byte)0xAB;
  CDROutputStream o = new CDROutputStream();
  SHelper.write(o, s);
  byte[] b = o.getBufferCopy();
  StringBuilder sb=new StringBuilder(); for(byte x:b) sb.append(String.format("%02x",x));
  System.out.println("jacorb bytes="+sb+" ("+b.length+")");
}}
