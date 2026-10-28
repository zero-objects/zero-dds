import java.math.BigDecimal;
import org.jacorb.orb.CDROutputStream;

public class Ref {
    public static void main(String[] args) throws Exception {
        FixedAll s = new FixedAll();
        s.head = (byte) 0x77;
        s.pos = new BigDecimal("123.45");
        s.neg = new BigDecimal("-123.45");
        s.zero = new BigDecimal("0.00");
        s.even = new BigDecimal("12.34");
        s.allfrac = new BigDecimal("0.123");
        s.one = new BigDecimal("7");
        s.two = new BigDecimal("42");
        s.big = new BigDecimal("1234567890123456789012345678901");
        s.tail = (byte) 0xAB;
        CDROutputStream out = new CDROutputStream();
        out.beginEncapsulatedArray();
        FixedAllHelper.write(out, s);
        byte[] b = out.getBufferCopy();
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < b.length; i++) sb.append(String.format("%02x", b[i]));
        System.out.println("jacorb-cdr-be " + sb + " (" + b.length + ")");
    }
}
