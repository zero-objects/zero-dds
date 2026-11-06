// SPDX-License-Identifier: Apache-2.0
package org.zerodds.testing;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.FileAlreadyExistsException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Set;

/**
 * Leases a DDS domain to one test process at a time, by the protocol of {@code crates/test-domains}
 * (the Rust tests): one file {@code domain-<d>.lease} per domain in the directory
 * {@code $ZERODDS_TEST_DOMAIN_DIR} names (the volume the CI jobs of a runner host share; it must
 * exist), else {@code $TMPDIR/zerodds-dcps-domains} (else {@code /tmp}), created exclusively,
 * holding {@code pid <N> since <T> ns <pid namespace>}; a lease whose owner is gone is taken over
 * under {@code takeover.lock}. An owner in this PID namespace is alive while its PID is; an owner
 * in another one (another container of the host) while it refreshes the file's modification time,
 * which every holder does every {@link #HEARTBEAT_MILLIS} ms: a lease older than
 * {@link #HEARTBEAT_STALE_MILLIS} ms counts as given up. Leases come from the same band: the domains up to {@link #MAX_DOMAIN}
 * whose RTPS port window lies outside the ephemeral port range of the host and that are not in
 * {@link #RESERVED_DOMAINS} or {@link #FOREIGN_DOMAINS}. On Linux (ephemeral 32768 to 60999) that
 * is 215 to 231 without 226 and 230, 90 and 92. Of those a host leases its share ({@link #share}): the CI
 * runner host's jobs set {@code $ZERODDS_TEST_DOMAIN_BAND} to {@code ci} and lease from
 * {@link #CI_SHARE_LOW} to {@link #CI_SHARE_HIGH}, every other host from the rest, because SPDP
 * multicast reaches every host of the LAN.
 *
 * <p>A domain is a host-wide resource: its number is the UDP port of its participants (DDSI-RTPS
 * 2.5 §9.6.1.1). The wire tests of this package used the fixed domains 227 to 232 and 7, which lie
 * in that band and among the domains the Rust tests keep, and their participants appeared in the
 * domain a Rust test had leased (measured: {@code java_peer.rs} saw the topics {@code WireCoherent}
 * and {@code DCPSParticipant} of a parallel Maven run). {@code crates/test-domains/tests/
 * java_lease_agrees.rs} checks that the constants here are those of the Rust crate.
 */
public final class TestDomains {

    /** RTPS §9.6.1.1: the port base. */
    static final int PORT_BASE = 7400;
    /** The distance of two neighbouring domains. */
    static final int DOMAIN_GAIN = 250;
    /** The SPDP unicast offset inside a domain. */
    static final int OFFSET_SPDP_UNICAST = 10;
    /** The distance of two participants of one domain. */
    static final int PARTICIPANT_GAIN = 2;
    /** The highest participant index a participant of a domain takes. */
    static final int MAX_PARTICIPANT_INDEX = 119;
    /** The highest domain whose port window fits in a {@code u16}. */
    public static final int MAX_DOMAIN = 231;
    /** The domains that stay hard-coded in the Rust tests and are never leased out. */
    static final int[] RESERVED_DOMAINS = {
        0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
        26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49,
        50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73,
        74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 91, 93, 94, 95, 96, 97, 98, 99,
        100,
    };
    /** Domains processes outside this workspace use on the LAN: 226 and 230. */
    static final int[] FOREIGN_DOMAINS = {226, 230};
    /** The lowest domain of the CI runner host's share. */
    static final int CI_SHARE_LOW = 219;
    /** The highest domain of the CI runner host's share. */
    static final int CI_SHARE_HIGH = 231;
    /** The environment variable that names this host's share of the band. */
    static final String BAND_VARIABLE = "ZERODDS_TEST_DOMAIN_BAND";
    /** How long a test waits for a domain of the band. */
    private static final long PATIENCE_MILLIS = 180_000;
    /** How often a holder refreshes the modification time of its lease files. */
    static final long HEARTBEAT_MILLIS = 2_000;
    /** How old the lease of another PID namespace may be before its owner counts as gone. */
    static final long HEARTBEAT_STALE_MILLIS = 20_000;
    /** The environment variable that names the lease directory the jobs of a host share. */
    static final String LEASE_DIR_VARIABLE = "ZERODDS_TEST_DOMAIN_DIR";
    /** The heartbeat thread, started with the first lease. */
    private static Thread heartbeat;

    /** Domains this process holds. */
    private static final Set<Integer> LEASED_HERE = new HashSet<Integer>();
    /** The leases held until the process exits ({@link #unique()}). */
    private static final List<Lease> HELD_FOR_PROCESS = new ArrayList<Lease>();

    private TestDomains() {
    }

    /** A domain held until {@link #close()}. */
    public static final class Lease implements AutoCloseable {
        private final int domain;

        private Lease(int domain) {
            this.domain = domain;
        }

        /** The domain number. */
        public int id() {
            return domain;
        }

        /** Gives the domain back to the band. */
        @Override
        public void close() {
            synchronized (LEASED_HERE) {
                LEASED_HERE.remove(domain);
            }
            try {
                Files.deleteIfExists(file(domain));
            } catch (IOException ignored) {
                // The next process that finds the owner gone takes it over.
            }
        }
    }

    /** A domain no other process of the host uses, held until the lease is closed. */
    public static Lease lease() {
        long end = System.currentTimeMillis() + PATIENCE_MILLIS;
        while (true) {
            Lease lease = tryLease();
            if (lease != null) {
                return lease;
            }
            if (System.currentTimeMillis() >= end) {
                throw new IllegalStateException("no DDS domain left after " + PATIENCE_MILLIS / 1000
                        + " s: every domain of the band is leased by a live process (" + dir() + ")");
            }
            try {
                Thread.sleep(500);
            } catch (InterruptedException interrupted) {
                Thread.currentThread().interrupt();
                throw new IllegalStateException("interrupted while waiting for a DDS domain");
            }
        }
    }

    /** A domain held for the life of this process, for a test class that keeps one. */
    public static int unique() {
        Lease lease = lease();
        synchronized (HELD_FOR_PROCESS) {
            if (HELD_FOR_PROCESS.isEmpty()) {
                Runtime.getRuntime().addShutdownHook(new Thread(TestDomains::releaseHeld));
            }
            HELD_FOR_PROCESS.add(lease);
        }
        return lease.id();
    }

    private static void releaseHeld() {
        synchronized (HELD_FOR_PROCESS) {
            for (Lease lease : HELD_FOR_PROCESS) {
                lease.close();
            }
        }
    }

    private static Lease tryLease() {
        synchronized (LEASED_HERE) {
            for (int domain : safeDomains()) {
                if (LEASED_HERE.contains(domain)) {
                    continue;
                }
                if (claim(domain)) {
                    LEASED_HERE.add(domain);
                    startHeartbeat();
                    return new Lease(domain);
                }
            }
            return null;
        }
    }

    /** The domains of the band, the high ones first. */
    static List<Integer> safeDomains() {
        long[] busy = ephemeralPortRange();
        List<Integer> band = new ArrayList<Integer>();
        for (int domain = MAX_DOMAIN; domain >= 0; domain--) {
            if (contains(RESERVED_DOMAINS, domain) || contains(FOREIGN_DOMAINS, domain)) {
                continue;
            }
            long low = PORT_BASE + (long) DOMAIN_GAIN * domain;
            long high = low + OFFSET_SPDP_UNICAST + (long) PARTICIPANT_GAIN * MAX_PARTICIPANT_INDEX;
            if (high <= 65535 && (high < busy[0] || low > busy[1])) {
                band.add(domain);
            }
        }
        if (band.isEmpty()) {
            throw new IllegalStateException("no DDS domain keeps its RTPS ports out of the ephemeral "
                    + "port range " + busy[0] + ".." + busy[1]);
        }
        String named = System.getenv(BAND_VARIABLE);
        List<Integer> mine = share(band, named == null ? "" : named.trim());
        if (mine.isEmpty()) {
            throw new IllegalStateException("no DDS domain in this host's share (" + BAND_VARIABLE + "="
                    + named + ") of the band " + band);
        }
        return mine;
    }

    /**
     * This host's share of the band: {@code ci} the domains from {@link #CI_SHARE_LOW} to
     * {@link #CI_SHARE_HIGH} (the whole band where none of them is in it: Windows and macOS),
     * {@code <low>-<high>} those in that range, empty the domains outside the CI share.
     */
    static List<Integer> share(List<Integer> band, String named) {
        List<Integer> ci = new ArrayList<Integer>();
        for (int domain : band) {
            if (domain >= CI_SHARE_LOW && domain <= CI_SHARE_HIGH) {
                ci.add(domain);
            }
        }
        List<Integer> out = new ArrayList<Integer>();
        if (named.isEmpty()) {
            for (int domain : band) {
                if (!ci.contains(domain)) {
                    out.add(domain);
                }
            }
            return out;
        }
        if (named.equals("ci")) {
            return ci.isEmpty() ? new ArrayList<Integer>(band) : ci;
        }
        String[] bounds = named.split("-");
        if (bounds.length != 2) {
            throw new IllegalStateException(BAND_VARIABLE + "=" + named + ": `ci` or `<low>-<high>`");
        }
        int low = Integer.parseInt(bounds[0].trim());
        int high = Integer.parseInt(bounds[1].trim());
        for (int domain : band) {
            if (domain >= low && domain <= high) {
                out.add(domain);
            }
        }
        return out;
    }

    private static boolean contains(int[] list, int domain) {
        for (int r : list) {
            if (r == domain) {
                return true;
            }
        }
        return false;
    }

    /** The port range the kernel hands out for "any port", read from the kernel. */
    static long[] ephemeralPortRange() {
        try {
            // Not Files.readAllBytes: it sizes its read by the file's size, which
            // procfs gives as 0, and JDK 21 returned "3" of "32768\t60999" (measured on
            // the Linux test host, 2026-11-01); the band then fell back to 90 and 92.
            String text;
            try (java.io.InputStream in = Files.newInputStream(Paths.get("/proc/sys/net/ipv4/ip_local_port_range"))) {
                text = new String(readAll(in), StandardCharsets.UTF_8).trim();
            }
            String[] fields = text.split("\\s+");
            if (fields.length >= 2) {
                long low = Long.parseLong(fields[0]);
                long high = Long.parseLong(fields[1]);
                if (low <= high) {
                    return new long[] {low, high};
                }
            }
        } catch (IOException | NumberFormatException notLinux) {
            // fall through to sysctl
        }
        try {
            Process sysctl = new ProcessBuilder("sysctl", "-n", "net.inet.ip.portrange.first",
                    "net.inet.ip.portrange.last").redirectErrorStream(true).start();
            byte[] out = readAll(sysctl.getInputStream());
            sysctl.waitFor();
            String[] fields = new String(out, StandardCharsets.UTF_8).trim().split("\\s+");
            if (fields.length >= 2) {
                long low = Long.parseLong(fields[0]);
                long high = Long.parseLong(fields[1]);
                if (low <= high) {
                    return new long[] {low, high};
                }
            }
        } catch (IOException | NumberFormatException | InterruptedException unreadable) {
            // the union below
        }
        // Neither kernel answered: the union of what both platforms use.
        return new long[] {32768, 65535};
    }

    private static byte[] readAll(java.io.InputStream in) throws IOException {
        java.io.ByteArrayOutputStream out = new java.io.ByteArrayOutputStream();
        byte[] buffer = new byte[256];
        for (int n; (n = in.read(buffer)) > 0; ) {
            out.write(buffer, 0, n);
        }
        return out.toByteArray();
    }

    /** Refreshes the modification time of every held lease, for readers in another PID namespace. */
    private static void startHeartbeat() {
        synchronized (LEASED_HERE) {
            if (heartbeat != null) {
                return;
            }
            heartbeat = new Thread(() -> {
                while (true) {
                    try {
                        Thread.sleep(HEARTBEAT_MILLIS);
                    } catch (InterruptedException stop) {
                        return;
                    }
                    List<Integer> held;
                    synchronized (LEASED_HERE) {
                        held = new ArrayList<Integer>(LEASED_HERE);
                    }
                    for (int domain : held) {
                        Path path = file(domain);
                        try {
                            if (Files.exists(path)) {
                                Files.setLastModifiedTime(path,
                                        java.nio.file.attribute.FileTime.fromMillis(System.currentTimeMillis()));
                            }
                        } catch (IOException givenBack) {
                            // given back meanwhile
                        }
                    }
                }
            }, "zerodds-domain-heartbeat");
            heartbeat.setDaemon(true);
            heartbeat.start();
        }
    }

    /** The PID namespace of this process, {@code pid:[<inode>]}, where Linux names it; else null. */
    static String pidNamespace() {
        try {
            String ns = Files.readSymbolicLink(Paths.get("/proc/self/ns/pid")).toString();
            return ns.isEmpty() || ns.matches(".*\\s.*") ? null : ns;
        } catch (IOException | UnsupportedOperationException | SecurityException notLinux) {
            return null;
        }
    }

    /**
     * The directory {@code $ZERODDS_TEST_DOMAIN_DIR} names, which must exist (a variable naming a
     * directory that is not there means the shared volume is not mounted); else
     * {@code $TMPDIR/zerodds-dcps-domains}, as Rust's {@code std::env::temp_dir()} reads it.
     */
    static Path dir() {
        String shared = System.getenv(LEASE_DIR_VARIABLE);
        if (shared != null && !shared.isEmpty()) {
            Path named = Paths.get(shared);
            if (!Files.isDirectory(named)) {
                throw new IllegalStateException(LEASE_DIR_VARIABLE + "=" + shared
                        + " is not a directory: the lease directory the jobs of this host share is not mounted");
            }
            return named;
        }
        String tmp = System.getenv("TMPDIR");
        Path dir = Paths.get(tmp != null && !tmp.isEmpty() ? tmp : "/tmp", "zerodds-dcps-domains");
        try {
            Files.createDirectories(dir);
        } catch (IOException why) {
            throw new IllegalStateException(dir + ": " + why);
        }
        return dir;
    }

    private static Path file(int domain) {
        return dir().resolve("domain-" + domain + ".lease");
    }

    private static long pid() {
        String name = java.lang.management.ManagementFactory.getRuntimeMXBean().getName();
        int at = name.indexOf('@');
        return Long.parseLong(at > 0 ? name.substring(0, at) : name);
    }

    private static boolean writeClaim(Path path) throws IOException {
        try {
            Files.createFile(path);
        } catch (FileAlreadyExistsException held) {
            return false;
        }
        String ns = pidNamespace();
        String line = "pid " + pid() + " since " + System.currentTimeMillis() / 1000
                + (ns != null ? " ns " + ns : "") + "\n";
        Files.write(path, line.getBytes(StandardCharsets.UTF_8));
        return true;
    }

    private static boolean claim(int domain) {
        Path path = file(domain);
        try {
            if (writeClaim(path)) {
                return true;
            }
            Path lock = dir().resolve("takeover.lock");
            for (int attempt = 0; attempt < 400; attempt++) {
                try {
                    Files.createFile(lock);
                } catch (FileAlreadyExistsException busy) {
                    long age = System.currentTimeMillis() - Files.getLastModifiedTime(lock).toMillis();
                    if (age > 30_000) {
                        Files.deleteIfExists(lock);
                        continue;
                    }
                    Thread.sleep(5);
                    continue;
                }
                try {
                    String held = new String(Files.readAllBytes(path), StandardCharsets.UTF_8).trim();
                    if (ownerIsAlive(held, path)) {
                        return false;
                    }
                    Files.deleteIfExists(path);
                    return writeClaim(path);
                } finally {
                    Files.deleteIfExists(lock);
                }
            }
            return false;
        } catch (IOException | InterruptedException unreadable) {
            return false;
        }
    }

    /**
     * Whether {@code pid N since T [ns NS]} names a live owner; an unreadable line counts as alive.
     * An owner in another PID namespace is alive while its heartbeat is fresh.
     */
    private static boolean ownerIsAlive(String owner, Path path) {
        String[] words = owner.split("\\s+");
        if (words.length < 2) {
            return true;
        }
        long pid;
        try {
            pid = Long.parseLong(words[1]);
        } catch (NumberFormatException unreadable) {
            return true;
        }
        String theirs = null;
        for (int i = 0; i + 1 < words.length; i++) {
            if (words[i].equals("ns")) {
                theirs = words[i + 1];
                break;
            }
        }
        String ours = pidNamespace();
        if (theirs != null && ours != null && !theirs.equals(ours)) {
            try {
                long age = System.currentTimeMillis() - Files.getLastModifiedTime(path).toMillis();
                return age < HEARTBEAT_STALE_MILLIS;
            } catch (java.nio.file.NoSuchFileException gone) {
                return false;
            } catch (IOException unreadable) {
                return true;
            }
        }
        if (Files.exists(Paths.get("/proc", Long.toString(pid)))) {
            return true;
        }
        if (Files.isDirectory(Paths.get("/proc/self"))) {
            return false;
        }
        try {
            Process kill = new ProcessBuilder("kill", "-0", Long.toString(pid)).start();
            return kill.waitFor() == 0;
        } catch (IOException | InterruptedException unknown) {
            return true;
        }
    }
}
