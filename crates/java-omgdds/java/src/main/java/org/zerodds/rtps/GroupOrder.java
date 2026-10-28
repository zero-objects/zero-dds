package org.zerodds.rtps;

import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.TreeMap;
import java.util.function.Consumer;
import java.util.function.Function;

/**
 * The ordering across the writers of one remote Publisher with access scope GROUP (DDSI-RTPS 2.5
 * 8.7.5, 8.7.6): the samples carry the group sequence number of the Publisher (PID_GROUP_SEQ_NUM)
 * and are handed to the readers in that order, and the samples of a group coherent set
 * (PID_GROUP_COHERENT_SET, ended by the End Coherent Set DATA of the writers) are handed over
 * together, as one step of {@link org.zerodds.internal.InProcessBus#atomically}.
 *
 * <p>A sample with the group sequence number n is committed when n-1 has been committed. If n-1
 * has not arrived, the standard decides by the HEARTBEATs with the GroupInfoFlag: n-1 is given up
 * when a HEARTBEAT has currentGSN >= n-1 and every counted writer (matched, reliable, of this
 * group) has said that the number is none of its own: it delivered a sample with a larger group
 * sequence number, or its HEARTBEAT, sent after n-1 was assigned (currentGSN >= n-1), gives a
 * range [firstGSN, lastGSN] that does not hold n-1. A coherent set with a number given up is not
 * delivered (8.7.6). The timeout rule that the standard allows in addition is an option, off by
 * default: {@code zerodds.rtps.group-timeout-millis} (a number that does not arrive within that
 * time is skipped).
 */
final class GroupOrder {

    private static final class Item {
        final long setFirst;
        final Runnable delivery; // null: the End Coherent Set marker
        final long since = System.currentTimeMillis();

        Item(long setFirst, Runnable delivery) {
            this.setFirst = setFirst;
            this.delivery = delivery;
        }
    }

    /** What the reader knows of one writer of the group. */
    private static final class WriterState {
        long maxSeen; // the largest group sequence number of a DATA of this writer
        boolean heartbeat;
        long current;
        long first;
        long last;
    }

    private static final class Group {
        long next; // 0: nothing seen yet
        final TreeMap<Long, Item> pending = new TreeMap<Long, Item>();
        long setFirst; // the coherent set being collected, 0: none
        final List<Runnable> set = new ArrayList<Runnable>();
        boolean setComplete = true;
        long current; // the largest currentGSN of a HEARTBEAT
        final Map<String, WriterState> writers = new HashMap<String, WriterState>();
    }

    private final Map<String, Group> groups = new HashMap<String, Group>();
    private final Consumer<String> log;
    private final Consumer<Runnable> atomically;
    private final Function<String, Set<String>> counted;
    private final long timeoutMillis;

    /**
     * @param counted the writers (GUIDs in hex) that count for the group with the key: matched,
     *     reliable, of the group, not lost
     * @param timeoutMillis the optional timeout rule, 0: off
     */
    GroupOrder(Consumer<String> log, Consumer<Runnable> atomically, Function<String, Set<String>> counted,
               long timeoutMillis) {
        this.log = log;
        this.atomically = atomically;
        this.counted = counted;
        this.timeoutMillis = timeoutMillis;
    }

    private Group group(String key) {
        Group g = groups.get(key);
        if (g == null) {
            g = new Group();
            groups.put(key, g);
        }
        return g;
    }

    private static WriterState writer(Group g, String writer) {
        WriterState w = g.writers.get(writer);
        if (w == null) {
            w = new WriterState();
            g.writers.put(writer, w);
        }
        return w;
    }

    /** A sample of the group {@code key} from {@code writer} with group sequence number {@code gsn}, in the set {@code setFirst} (0: none). */
    void accept(String key, String writer, long gsn, long setFirst, Runnable delivery) {
        add(key, writer, gsn, new Item(setFirst, delivery));
    }

    /** The End Coherent Set DATA with group sequence number {@code gsn} for the set {@code setFirst}. */
    void ecs(String key, String writer, long gsn, long setFirst) {
        add(key, writer, gsn, new Item(setFirst, null));
    }

    /**
     * A HEARTBEAT with the GroupInfoFlag from {@code writer} (DDSI-RTPS 2.5 8.3.7.5): the largest
     * group sequence number of the Publisher and the range of this writer.
     */
    void heartbeat(String key, String writer, long currentGsn, long firstGsn, long lastGsn) {
        List<Runnable> now = new ArrayList<Runnable>();
        synchronized (this) {
            Group g = group(key);
            WriterState w = writer(g, writer);
            if (!w.heartbeat || currentGsn >= w.current) {
                w.heartbeat = true;
                w.current = currentGsn;
                w.first = firstGsn;
                w.last = lastGsn;
            }
            g.current = Math.max(g.current, currentGsn);
            resolve(g, key, now);
        }
        run(now);
    }

    private void add(String key, String writer, long gsn, Item item) {
        List<Runnable> now = new ArrayList<Runnable>();
        synchronized (this) {
            Group g = group(key);
            WriterState w = writer(g, writer);
            w.maxSeen = Math.max(w.maxSeen, gsn);
            if (g.next == 0) {
                g.next = gsn;
            }
            if (gsn < g.next) {
                // late: its place has passed; a marker already counted, a sample goes on at once
                if (item.delivery != null) {
                    now.add(item.delivery);
                }
            } else if (!g.pending.containsKey(gsn)) {
                g.pending.put(gsn, item);
                drain(g, now);
                resolve(g, key, now);
            }
        }
        run(now);
    }

    /** Gives up the numbers that the HEARTBEATs show to be none of the counted writers' (8.7.5). */
    private void resolve(Group g, String key, List<Runnable> out) {
        if (g.next == 0 || g.pending.isEmpty()) {
            return;
        }
        Set<String> writers = counted.apply(key);
        while (!g.pending.isEmpty() && g.pending.firstKey() > g.next && absent(g, writers, g.next)) {
            log.accept("group sequence number " + g.next + " is no sample of a writer of the group;"
                    + " given up (DDSI-RTPS 2.5 8.7.5)");
            if (g.setFirst != 0) {
                g.setComplete = false;
            }
            g.next++;
            drain(g, out);
        }
    }

    private static boolean absent(Group g, Set<String> writers, long n) {
        if (writers.isEmpty() || g.current < n) {
            return false;
        }
        for (String id : writers) {
            WriterState w = g.writers.get(id);
            if (w == null) {
                return false; // nothing heard of this writer yet
            }
            boolean ruledOut = w.maxSeen > n
                    || (w.heartbeat && w.current >= n && !(w.first <= n && n <= w.last));
            if (!ruledOut) {
                return false;
            }
        }
        return true;
    }

    private void drain(Group g, List<Runnable> out) {
        while (true) {
            Item item = g.pending.remove(g.next);
            if (item == null) {
                return;
            }
            g.next++;
            commit(g, item, out);
        }
    }

    private void commit(Group g, Item item, List<Runnable> out) {
        if (item.delivery == null) { // End Coherent Set
            if (g.setFirst == item.setFirst && g.setFirst != 0) {
                flush(g, out);
            }
            return;
        }
        if (item.setFirst != g.setFirst) {
            flush(g, out); // a sample of another set ends the open one (8.7.6)
        }
        if (item.setFirst == 0) {
            out.add(item.delivery);
            return;
        }
        if (g.setFirst == 0) {
            g.setFirst = item.setFirst;
            g.setComplete = true;
        }
        g.set.add(item.delivery);
    }

    private void flush(Group g, List<Runnable> out) {
        if (!g.set.isEmpty()) {
            if (g.setComplete) {
                final List<Runnable> all = new ArrayList<Runnable>(g.set);
                out.add(new Runnable() {
                    @Override
                    public void run() {
                        atomically.accept(new Runnable() {
                            @Override
                            public void run() {
                                for (Runnable d : all) {
                                    d.run();
                                }
                            }
                        });
                    }
                });
            } else {
                log.accept("group coherent set " + g.setFirst + " is incomplete; not delivered"
                        + " (DDSI-RTPS 2.5 8.7.6)");
            }
        }
        g.set.clear();
        g.setFirst = 0;
        g.setComplete = true;
    }

    /** The optional timeout rule: a number that did not arrive in time is skipped. */
    void expire(long nowMillis) {
        if (timeoutMillis <= 0) {
            return;
        }
        List<Runnable> now = new ArrayList<Runnable>();
        synchronized (this) {
            for (Group g : groups.values()) {
                while (!g.pending.isEmpty()) {
                    Map.Entry<Long, Item> first = g.pending.firstEntry();
                    if (first.getKey() <= g.next || nowMillis - first.getValue().since < timeoutMillis) {
                        break;
                    }
                    log.accept("group sequence number " + g.next + (first.getKey() - 1 > g.next
                            ? " to " + (first.getKey() - 1) : "") + " did not arrive; skipped");
                    if (g.setFirst != 0) {
                        g.setComplete = false;
                    }
                    g.next = first.getKey();
                    drain(g, now);
                }
            }
        }
        run(now);
    }

    private static void run(List<Runnable> deliveries) {
        for (Runnable d : deliveries) {
            d.run();
        }
    }
}
