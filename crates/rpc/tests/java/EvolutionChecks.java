// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Interface evolution in the Enhanced Service Mapping (DDS-RPC 1.0 §7.7.2),
// with the generated Java clients and services of two versions of one
// interface, over the loopback binding of EnhancedRuntimeChecks. The old and
// the new version share the two topic names (§7.7: "as long as the same topic
// names are used"); they differ in the operations, in the order of the
// parameters of a @Mutable operation, and in the exceptions an operation
// raises.
//
//   - §7.7.2.1 / §7.7.2.2: an operation one side does not have is answered
//     REMOTE_EX_UNKNOWN_OPERATION; the others are served;
//   - §7.7.2.5.2: the parameters of a @Mutable operation are matched by member
//     id, so a reordering does not change which value reaches which parameter;
//   - §7.7.2.5.6: an exception the client does not know raises
//     REMOTE_EX_UNKNOWN_EXCEPTION; one both know is the user exception.

import java.util.concurrent.ExecutionException;

import org.omg.dds.rpc.Future;
import org.omg.dds.rpc.RemoteUnknownExceptionError;
import org.omg.dds.rpc.RemoteUnknownOperationError;
import org.omg.dds.rpc.ReplierParams;
import org.omg.dds.rpc.RequesterParams;
import org.omg.dds.rpc.UserExceptionRaised;

public final class EvolutionChecks {

    private static int failures = 0;

    static void check(boolean held, String what) {
        if (!held) {
            System.err.println("FAIL " + what);
            ++failures;
        }
    }

    static final String REQUEST_TOPIC = "Evolution_Service_Request";
    static final String REPLY_TOPIC = "Evolution_Service_Reply";

    static final class OldImplementation implements ZdServices.ev_old.Svc {
        int mixedX;
        int mixedY;

        @Override
        public int add(int a, int b) {
            return a + b;
        }

        @Override
        public int legacy() {
            return 7;
        }

        @Override
        public void mix(int x, int y) {
            mixedX = x;
            mixedY = y;
        }

        @Override
        public int fail() {
            ZdTypes.ev_old.E1 refusal = new ZdTypes.ev_old.E1();
            refusal.code = 11;
            throw new UserExceptionRaised(refusal);
        }
    }

    static final class NewImplementation implements ZdServices.ev_new.Svc {
        int mixedX;
        int mixedY;

        @Override
        public int add(int a, int b) {
            return a + b;
        }

        @Override
        public int extra() {
            return 9;
        }

        @Override
        public void mix(int y, int x) {
            mixedX = x;
            mixedY = y;
        }

        @Override
        public int fail() {
            ZdTypes.ev_new.E2 refusal = new ZdTypes.ev_new.E2();
            refusal.code = 22;
            throw new UserExceptionRaised(refusal);
        }
    }

    static ReplierParams oldReplier(
            EnhancedRuntimeChecks.LoopParticipant p) {
        return ZdServices.ev_old.SvcSupport.replierParams(p, "")
                .requestTopicName(REQUEST_TOPIC)
                .replyTopicName(REPLY_TOPIC);
    }

    static RequesterParams oldRequester(
            EnhancedRuntimeChecks.LoopParticipant p) {
        return ZdServices.ev_old.SvcSupport.requesterParams(p)
                .requestTopicName(REQUEST_TOPIC)
                .replyTopicName(REPLY_TOPIC);
    }

    static ReplierParams newReplier(
            EnhancedRuntimeChecks.LoopParticipant p) {
        return ZdServices.ev_new.SvcSupport.replierParams(p, "")
                .requestTopicName(REQUEST_TOPIC)
                .replyTopicName(REPLY_TOPIC);
    }

    static RequesterParams newRequester(
            EnhancedRuntimeChecks.LoopParticipant p) {
        return ZdServices.ev_new.SvcSupport.requesterParams(p)
                .requestTopicName(REQUEST_TOPIC)
                .replyTopicName(REPLY_TOPIC);
    }

    /** Lets the service answer, the client read, and returns what the future holds or raised. */
    static Object outcome(
            Future<?> pending, org.omg.dds.rpc.Service service, org.omg.dds.rpc.FunctionCallClient<?, ?> client)
            throws InterruptedException {
        service.dispatch();
        client.dispatchReplies();
        try {
            return pending.get();
        } catch (ExecutionException failure) {
            return failure.getCause();
        }
    }

    /** An old client against a new service. */
    static void oldClientNewService() throws Exception {
        EnhancedRuntimeChecks.LoopParticipant participant = new EnhancedRuntimeChecks.LoopParticipant();
        NewImplementation implementation = new NewImplementation();
        ZdServices.ev_new.SvcServiceImpl service =
                new ZdServices.ev_new.SvcServiceImpl(newReplier(participant), implementation);
        ZdServices.ev_old.SvcClientImpl client =
                new ZdServices.ev_old.SvcClientImpl(oldRequester(participant));

        check(Integer.valueOf(5).equals(outcome(client.add_async(2, 3), service, client)),
                "§7.7.2: an operation both versions have is served: add(2, 3) = 5");
        // §7.7.2.2: legacy() was removed from the new version.
        check(outcome(client.legacy_async(), service, client) instanceof RemoteUnknownOperationError,
                "§7.7.2.2: an operation the new version removed is REMOTE_EX_UNKNOWN_OPERATION");
        // §7.7.2.5.2: the new version names the parameters (y, x); the values follow the names.
        outcome(client.mix_async(1, 2), service, client);
        check(implementation.mixedX == 1 && implementation.mixedY == 2,
                "§7.7.2.5.2: a @Mutable operation reaches its parameters by name, not by position");
        // §7.7.2.5.6: the new version raises E2, which the old client does not know.
        check(outcome(client.fail_async(), service, client) instanceof RemoteUnknownExceptionError,
                "§7.7.2.5.6: an exception the old client does not know is REMOTE_EX_UNKNOWN_EXCEPTION");
        client.close();
        service.close();
    }

    /** A new client against an old service. */
    static void newClientOldService() throws Exception {
        EnhancedRuntimeChecks.LoopParticipant participant = new EnhancedRuntimeChecks.LoopParticipant();
        OldImplementation implementation = new OldImplementation();
        ZdServices.ev_old.SvcServiceImpl service =
                new ZdServices.ev_old.SvcServiceImpl(oldReplier(participant), implementation);
        ZdServices.ev_new.SvcClientImpl client =
                new ZdServices.ev_new.SvcClientImpl(newRequester(participant));

        check(Integer.valueOf(9).equals(outcome(client.add_async(4, 5), service, client)),
                "add(4, 5) = 9 against the old service");
        // §7.7.2.1: extra() was added to the new version.
        check(outcome(client.extra_async(), service, client) instanceof RemoteUnknownOperationError,
                "§7.7.2.1: an operation the old service lacks is REMOTE_EX_UNKNOWN_OPERATION");
        outcome(client.mix_async(10, 20), service, client);
        check(implementation.mixedY == 10 && implementation.mixedX == 20,
                "§7.7.2.5.2: the new client's (y = 10, x = 20) reach the old service's x and y by name");
        // E1 is known on both sides.
        Object raised = outcome(client.fail_async(), service, client);
        check(raised instanceof UserExceptionRaised
                        && ((UserExceptionRaised) raised).value() instanceof ZdTypes.ev_new.E1
                        && ((ZdTypes.ev_new.E1) ((UserExceptionRaised) raised).value()).code == 11,
                "§7.7.2.5.6: an exception both versions declare is the user exception");
        client.close();
        service.close();
    }

    public static void main(String[] args) throws Exception {
        oldClientNewService();
        newClientOldService();
        if (failures != 0) {
            System.err.println(failures + " checks failed");
            System.exit(1);
        }
        System.out.println("every check held");
    }
}
