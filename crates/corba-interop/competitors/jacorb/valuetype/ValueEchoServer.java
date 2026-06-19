// SPDX-License-Identifier: Apache-2.0
// JacORB server for the live valuetype cross-ORB interop (#13): offers
// `ValueEcho::echo(in Point p) -> Point` and registers a ValueFactory
// for `IDL:Point:1.0` so the valuetype parameter can be unmarshalled.
import org.omg.CORBA.ORB;
import org.omg.PortableServer.POA;
import org.omg.PortableServer.POAHelper;

class ValueEchoImpl extends ValueEchoPOA {
    public Point echo(Point p) {
        return p; // identity echo of the valuetype
    }
}

class PointDefaultFactory implements org.omg.CORBA.portable.ValueFactory {
    public java.io.Serializable read_value(org.omg.CORBA_2_3.portable.InputStream is) {
        return is.read_value(new PointImpl());
    }
}

public class ValueEchoServer {
    public static void main(String[] args) throws Exception {
        ORB orb = ORB.init(args, null);
        // Register the ValueFactory so incoming Point values are unmarshalled.
        ((org.omg.CORBA_2_3.ORB) orb).register_value_factory(
            "IDL:Point:1.0", new PointDefaultFactory());

        POA root = POAHelper.narrow(orb.resolve_initial_references("RootPOA"));
        root.the_POAManager().activate();
        ValueEchoImpl impl = new ValueEchoImpl();
        org.omg.CORBA.Object ref = root.servant_to_reference(impl);
        System.out.println("VALUEECHO_IOR=" + orb.object_to_string(ref));
        System.out.flush();
        orb.run();
    }
}
