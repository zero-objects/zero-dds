// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

import org.omg.CORBA.BAD_PARAM;
import org.omg.CORBA.LocalObject;
import org.omg.CORBA.ORB;
import org.omg.CORBA.Request;
import org.omg.CORBA.TCKind;
import org.omg.IOP.ServiceContext;
import org.omg.PortableInterceptor.ClientRequestInfo;
import org.omg.PortableInterceptor.ClientRequestInterceptor;
import org.omg.PortableInterceptor.ORBInitInfo;
import org.omg.PortableInterceptor.ORBInitializer;

/**
 * CORBA 3.3 Part 2 §7.7.2 with a JacORB client: its client request
 * interceptor adds a service context outside the OMG range to a call of
 * `ping` on the ZeroDDS object of the system property `zerodds.ior` and
 * reads the vendor context the ZeroDDS server interceptor puts in the
 * reply. Prints the answer, the reply context and ALL OK.
 */
public class ContextClient {
    static final int VENDOR_CTX = 0x4A430001;
    static final int REPLY_CTX = 0x4A430002;
    static volatile String replyContext = "none";

    public static class Init extends LocalObject implements ORBInitializer {
        public void pre_init(ORBInitInfo info) {
            try {
                info.add_client_request_interceptor(new Tagger());
            } catch (Exception e) {
                throw new RuntimeException(e);
            }
        }

        public void post_init(ORBInitInfo info) {}
    }

    public static class Tagger extends LocalObject implements ClientRequestInterceptor {
        public String name() {
            return "zerodds-vendor-context";
        }

        public void destroy() {}

        public void send_request(ClientRequestInfo ri) {
            ri.add_request_service_context(
                new ServiceContext(VENDOR_CTX, "from-jacorb".getBytes()), false);
        }

        public void send_poll(ClientRequestInfo ri) {}

        public void receive_reply(ClientRequestInfo ri) {
            try {
                replyContext =
                    new String(ri.get_reply_service_context(REPLY_CTX).context_data);
            } catch (BAD_PARAM e) {
                replyContext = "missing";
            }
        }

        public void receive_exception(ClientRequestInfo ri) {}

        public void receive_other(ClientRequestInfo ri) {}
    }

    public static void main(String[] args) {
        java.util.Properties props = new java.util.Properties();
        props.put("org.omg.PortableInterceptor.ORBInitializerClass.ContextClient$Init", "");
        ORB orb = ORB.init(args, props);
        org.omg.CORBA.Object target = orb.string_to_object(System.getProperty("zerodds.ior"));
        Request request = target._request("ping");
        request.add_in_arg().insert_string("hi");
        request.set_return_type(orb.get_primitive_tc(TCKind.tk_string));
        request.invoke();
        String answer = request.return_value().extract_string();
        System.out.println("PING=" + answer);
        System.out.println("REPLY_CTX=" + replyContext);
        if ("hi".equals(answer) && "from-zerodds".equals(replyContext)) {
            System.out.println("ALL OK");
        }
        orb.shutdown(true);
    }
}
