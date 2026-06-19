import org.omg.CORBA.*;
import java.io.*; import java.util.*;
public class Client {
  public static void main(String[] a) throws Exception {
    ORB orb = ORB.init(a, null);
    BufferedReader r = new BufferedReader(new FileReader("/tmp/echo_jacorb.ior"));
    String ior = r.readLine(); r.close();
    Echo echo = EchoHelper.narrow(orb.string_to_object(ior));
    int payload = a.length>0?Integer.parseInt(a[0]):56;
    int n = a.length>1?Integer.parseInt(a[1]):50000;
    char[] c = new char[payload]; Arrays.fill(c, (char)120); String msg = new String(c);
    for(int i=0;i<10000;i++) echo.ping(msg);
    long[] s = new long[n];
    for(int i=0;i<n;i++){ long t0=System.nanoTime(); echo.ping(msg); s[i]=System.nanoTime()-t0; }
    Arrays.sort(s);
    System.out.printf("JacORB Echo roundtrip (IIOP loopback, payload=%dB, N=%d)%n", payload, n);
    System.out.printf("  min=%.1fus  p50=%.1fus  p90=%.1fus  p99=%.1fus  p99.9=%.1fus%n",
      s[0]/1000.0, s[(int)(n*0.5)]/1000.0, s[(int)(n*0.9)]/1000.0, s[(int)(n*0.99)]/1000.0, s[(int)(n*0.999)]/1000.0);
    System.exit(0);
  }
}
