package org.zerodds.rtps;

import java.util.ArrayList;
import java.util.Collections;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.function.Consumer;

/**
 * What a reader of a coherent-access Subscriber does with the samples of a remote writer's
 * coherent set (DDSI-RTPS 2.5 8.7.6, access scope TOPIC): they are held until the set is
 * complete and then released together. The set ends with a DATA of the writer that belongs to
 * another set, to none, or carries no serialized data (Table 9.22). A set that is not complete
 * (a sample of it did not arrive, or its first did not) is not released: "Incomplete coherent
 * sets shall not be added to the history of the RTPS DataReaders" (8.7.6).
 */
final class CoherentHold {

    /** The set-first sequence number {@code PID_COHERENT_SET} says; 0: the sample is in no set. */
    static final long NO_SET = 0L;

    private static final class Pending {
        final long set;
        long last;
        boolean complete = true;
        final List<SerializedSample> samples = new ArrayList<SerializedSample>();

        Pending(long set) {
            this.set = set;
        }
    }

    private final Consumer<String> log;
    private final Map<String, Pending> pending = new HashMap<String, Pending>();

    CoherentHold(Consumer<String> log) {
        this.log = log;
    }

    /**
     * A sample of {@code writer} with sequence number {@code sn}, in the coherent set {@code set}
     * ({@link #NO_SET}: none).
     *
     * @return the samples to hand on now, in order
     */
    synchronized List<SerializedSample> accept(String writer, long set, long sn,
            SerializedSample sample) {
        Pending p = pending.get(writer);
        if (set == NO_SET) {
            List<SerializedSample> out = new ArrayList<SerializedSample>(release(writer, p));
            out.add(sample);
            return out;
        }
        List<SerializedSample> out = Collections.emptyList();
        if (p != null && p.set != set) {
            out = release(writer, p);
            p = null;
        }
        if (p == null) {
            p = new Pending(set);
            p.complete = sn == set;
            pending.put(writer, p);
        } else if (sn != p.last + 1) {
            p.complete = false;
        }
        p.last = sn;
        p.samples.add(sample);
        return out;
    }

    /** A DATA of {@code writer} without serialized data: the writer's open set ends. */
    synchronized List<SerializedSample> end(String writer) {
        return release(writer, pending.get(writer));
    }

    /** The writer is gone: what it left of a set is not released. */
    synchronized void drop(String writer) {
        Pending p = pending.remove(writer);
        if (p != null) {
            log.accept("coherent set " + p.set + " of " + writer + " dropped with its writer");
        }
    }

    private List<SerializedSample> release(String writer, Pending p) {
        if (p == null) {
            return Collections.emptyList();
        }
        pending.remove(writer);
        if (!p.complete) {
            log.accept("coherent set " + p.set + " of " + writer + " is incomplete (" + p.samples.size()
                    + " samples held); not delivered (DDSI-RTPS 2.5 8.7.6)");
            return Collections.emptyList();
        }
        return p.samples;
    }
}
