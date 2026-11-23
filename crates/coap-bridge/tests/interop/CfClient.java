// A Californium client for crates/coap-bridge/tests/cross_vendor.rs
// (zerodds-coap-bridge-1.0 §10): `post URI JSON`, `get URI` and
// `observe URI SECONDS`, the payloads as JSON, every response as a line
// `<code> <payload>` on stdout. Run as a single-file program:
// `java -cp <californium-core, element-connector, slf4j-api> CfClient.java ...`.
import org.eclipse.californium.core.CoapClient;
import org.eclipse.californium.core.CoapHandler;
import org.eclipse.californium.core.CoapObserveRelation;
import org.eclipse.californium.core.CoapResponse;
import org.eclipse.californium.core.coap.MediaTypeRegistry;
import org.eclipse.californium.core.config.CoapConfig;
import org.eclipse.californium.elements.config.UdpConfig;

public class CfClient {
    static void print(CoapResponse r) {
        if (r == null) {
            System.out.println("none");
        } else {
            System.out.println(r.getCode() + " " + r.getResponseText());
        }
        System.out.flush();
    }

    public static void main(String[] args) throws Exception {
        CoapConfig.register();
        UdpConfig.register();
        CoapClient client = new CoapClient(args[1]);
        client.setTimeout(5000L);
        switch (args[0]) {
            case "post":
                print(client.post(args[2], MediaTypeRegistry.APPLICATION_JSON));
                break;
            case "get":
                print(client.get(MediaTypeRegistry.APPLICATION_JSON));
                break;
            case "observe":
                CoapObserveRelation relation = client.observe(new CoapHandler() {
                    @Override
                    public void onLoad(CoapResponse r) {
                        print(r);
                    }

                    @Override
                    public void onError() {
                        System.out.println("error");
                        System.out.flush();
                    }
                }, MediaTypeRegistry.APPLICATION_JSON);
                Thread.sleep(Long.parseLong(args[2]) * 1000L);
                relation.proactiveCancel();
                Thread.sleep(500L);
                break;
            default:
                System.err.println("post URI JSON | get URI | observe URI SECONDS");
                System.exit(2);
        }
        client.shutdown();
        System.exit(0);
    }
}
