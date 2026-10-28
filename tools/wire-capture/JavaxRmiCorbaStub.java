package javax.rmi.CORBA;

import org.omg.CORBA.ORB;
import org.omg.CORBA.portable.ObjectImpl;

/* JDK 21 dropped javax.rmi.CORBA. JacORB 3.9`s org.jacorb.orb.Reference
   extends this class, so it has to exist with the same shape as the JDK 8
   original: an abstract ObjectImpl. Nothing here is called on the IIOP
   path this measurement uses. */
public abstract class Stub extends ObjectImpl implements java.io.Serializable {
    public void connect(ORB orb) throws java.rmi.RemoteException {
    }

    private void readObject(java.io.ObjectInputStream s)
            throws java.io.IOException, ClassNotFoundException {
    }

    private void writeObject(java.io.ObjectOutputStream s) throws java.io.IOException {
    }
}
