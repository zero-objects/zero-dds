import org.jacorb.orb.CDROutputStream;
public class JW { public static void main(String[] a) throws Exception {
  W w = new W(); w.head=(byte)0x11; w.c=(char)0x00e4; w.mid=(byte)0x22; w.s="aä"; w.tail=(byte)0xAB;
  CDROutputStream o = new CDROutputStream();
  try { WHelper.write(o,w); } catch (Throwable t) { System.out.println("jacorb refused: "+t); return; }
  byte[] b=o.getBufferCopy(); StringBuilder sb=new StringBuilder(); for(byte x:b) sb.append(String.format("%02x",x));
  System.out.println("jacorb bytes="+sb+" ("+b.length+")");
}}
