// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.Test;
import org.omg.dds.core.Duration;
import org.omg.dds.core.InstanceHandle;
import org.omg.dds.core.ModifiableInstanceHandle;
import org.omg.dds.core.ModifiableTime;
import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.core.Time;

import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.ObjectInputStream;
import java.io.ObjectOutputStream;
import java.io.Serializable;
import java.util.Arrays;
import java.util.concurrent.TimeUnit;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * DDS-Java-PSM 1.0 §7.3.3 and §7.3.4: the value types Time, Duration and
 * InstanceHandle compare by value, accept a value of another implementation,
 * know the invalid time and copy their state with copyFrom.
 */
class CoreValuesTest {

    private static final ServiceEnvironment ENV = Qos.env();

    // ---- values of another implementation --------------------------------

    /** A Time as another DDS implementation would write it: nanoseconds since the epoch. */
    static final class ForeignTime extends Time {
        private static final long serialVersionUID = 1L;
        private final long nanos;

        ForeignTime(long nanos) {
            this.nanos = nanos;
        }

        @Override
        public long getTime(TimeUnit inThisUnit) {
            return nanos < 0 ? -1 : inThisUnit.convert(nanos, TimeUnit.NANOSECONDS);
        }

        @Override
        public long getRemainder(TimeUnit primaryUnit, TimeUnit remainderUnit) {
            return 0;
        }

        @Override
        public boolean isValid() {
            return nanos >= 0;
        }

        @Override
        public ModifiableTime modifiableCopy() {
            return ENV.getSPI().newTime(nanos, TimeUnit.NANOSECONDS);
        }

        @Override
        public int compareTo(Time other) {
            return Long.compare(getTime(TimeUnit.NANOSECONDS), other.getTime(TimeUnit.NANOSECONDS));
        }

        @Override
        public ServiceEnvironment getEnvironment() {
            return null;
        }

        @Override
        public boolean equals(Object o) {
            return o instanceof Time && getTime(TimeUnit.NANOSECONDS) == ((Time) o).getTime(TimeUnit.NANOSECONDS);
        }

        @Override
        public int hashCode() {
            return Long.hashCode(getTime(TimeUnit.NANOSECONDS));
        }
    }

    /** A Duration of another implementation: nanoseconds, no infinite value. */
    static final class ForeignDuration extends Duration {
        private static final long serialVersionUID = 1L;
        private final long nanos;

        ForeignDuration(long nanos) {
            this.nanos = nanos;
        }

        @Override
        public long getDuration(TimeUnit inThisUnit) {
            return inThisUnit.convert(nanos, TimeUnit.NANOSECONDS);
        }

        @Override
        public long getRemainder(TimeUnit primaryUnit, TimeUnit remainderUnit) {
            return 0;
        }

        @Override
        public boolean isZero() {
            return nanos == 0;
        }

        @Override
        public boolean isInfinite() {
            return false;
        }

        @Override
        public Duration add(Duration duration) {
            return new ForeignDuration(nanos + duration.getDuration(TimeUnit.NANOSECONDS));
        }

        @Override
        public Duration add(long duration, TimeUnit unit) {
            return new ForeignDuration(nanos + unit.toNanos(duration));
        }

        @Override
        public Duration subtract(Duration duration) {
            return new ForeignDuration(nanos - duration.getDuration(TimeUnit.NANOSECONDS));
        }

        @Override
        public Duration subtract(long duration, TimeUnit unit) {
            return new ForeignDuration(nanos - unit.toNanos(duration));
        }

        @Override
        public int compareTo(Duration other) {
            return Long.compare(getDuration(TimeUnit.NANOSECONDS), other.getDuration(TimeUnit.NANOSECONDS));
        }

        @Override
        public ServiceEnvironment getEnvironment() {
            return null;
        }

        @Override
        public boolean equals(Object o) {
            return o instanceof Duration
                    && getDuration(TimeUnit.NANOSECONDS) == ((Duration) o).getDuration(TimeUnit.NANOSECONDS);
        }

        @Override
        public int hashCode() {
            return Long.hashCode(nanos);
        }
    }

    /** An InstanceHandle of another implementation; it exposes no octets, only isNil(). */
    static final class ForeignHandle extends InstanceHandle {
        private static final long serialVersionUID = 1L;
        private final byte[] data;

        ForeignHandle(int... values) {
            data = new byte[values.length];
            for (int i = 0; i < values.length; i++) {
                data[i] = (byte) values[i];
            }
        }

        @Override
        public boolean isNil() {
            for (byte b : data) {
                if (b != 0) {
                    return false;
                }
            }
            return true;
        }

        @Override
        public ModifiableInstanceHandle modifiableCopy() {
            throw new UnsupportedOperationException();
        }

        @Override
        public int compareTo(InstanceHandle other) {
            return 0;
        }

        @Override
        public ServiceEnvironment getEnvironment() {
            return null;
        }

        @Override
        public boolean equals(Object o) {
            return o instanceof ForeignHandle && Arrays.equals(data, ((ForeignHandle) o).data);
        }

        @Override
        public int hashCode() {
            return Arrays.hashCode(data);
        }
    }

    private static byte[] octets(int first, int last) {
        byte[] out = new byte[16];
        out[0] = (byte) first;
        out[15] = (byte) last;
        return out;
    }

    @SuppressWarnings("unchecked")
    private static <T extends Serializable> T roundTrip(T value) throws Exception {
        ByteArrayOutputStream bytes = new ByteArrayOutputStream();
        try (ObjectOutputStream out = new ObjectOutputStream(bytes)) {
            out.writeObject(value);
        }
        try (ObjectInputStream in = new ObjectInputStream(new ByteArrayInputStream(bytes.toByteArray()))) {
            return (T) in.readObject();
        }
    }

    // ---- Time -----------------------------------------------------------

    @Test
    void equalTimesHaveEqualHashCodeAndCompareAsEqual() {
        Time a = ENV.getSPI().newTime(1500, TimeUnit.MILLISECONDS);
        ModifiableTime b = ENV.getSPI().newTime(1, TimeUnit.SECONDS);
        b.add(500, TimeUnit.MILLISECONDS);
        // 1 s + 500 ms = 1 500 000 000 ns on both sides
        assertEquals(a, b);
        assertEquals(b, a);
        assertEquals(a.hashCode(), b.hashCode());
        assertEquals(0, a.compareTo(b));
        // the immutable and the modifiable copy are equal to the original
        assertEquals(a, a.modifiableCopy());
        assertEquals(a.hashCode(), a.modifiableCopy().hashCode());
        assertEquals(a, b.immutableCopy());
        assertEquals(1_500_000_000L, a.getTime(TimeUnit.NANOSECONDS));
    }

    @Test
    void timesOrderByTheirInstant() {
        Time early = ENV.getSPI().newTime(1, TimeUnit.SECONDS);
        Time late = ENV.getSPI().newTime(2, TimeUnit.SECONDS);
        assertTrue(early.compareTo(late) < 0);
        assertTrue(late.compareTo(early) > 0);
        assertNotEquals(early, late);
    }

    @Test
    void aNegativeInstantInAnyUnitIsInvalid() {
        // §7.3.4: an instant before the epoch is the invalid time. A value in a
        // unit below seconds has zero whole seconds and must still be invalid.
        for (TimeUnit unit : TimeUnit.values()) {
            Time t = ENV.getSPI().newTime(-5, unit);
            assertFalse(t.isValid(), "-5 " + unit);
            assertEquals(-1, t.getTime(TimeUnit.NANOSECONDS), "-5 " + unit);
        }
        assertFalse(ENV.getSPI().invalidTime().isValid());
        assertEquals(-1, ENV.getSPI().invalidTime().getTime(TimeUnit.MILLISECONDS));
        assertFalse(Time.invalidTime(ENV).isValid());
        assertFalse(Time.newTime(-1, TimeUnit.NANOSECONDS, ENV).isValid());
        assertTrue(ENV.getSPI().newTime(0, TimeUnit.NANOSECONDS).isValid());
        assertTrue(ENV.getSPI().newTime(1, TimeUnit.NANOSECONDS).isValid());
        assertFalse(org.zerodds.core.Values.ofMillis(ENV, -1).isValid());
    }

    @Test
    void theInvalidTimeEqualsItselfAndOrdersBeforeEveryValidTime() {
        Time invalid = ENV.getSPI().invalidTime();
        Time zero = ENV.getSPI().newTime(0, TimeUnit.NANOSECONDS);
        assertEquals(invalid, ENV.getSPI().newTime(-7, TimeUnit.MILLISECONDS));
        assertEquals(invalid.hashCode(), ENV.getSPI().newTime(-7, TimeUnit.MILLISECONDS).hashCode());
        assertNotEquals(invalid, zero);
        assertTrue(invalid.compareTo(zero) < 0);
        assertTrue(zero.compareTo(invalid) > 0);
    }

    @Test
    void addAndSubtractMoveTheInstantAndKeepItNormalised() {
        ModifiableTime t = ENV.getSPI().newTime(1500, TimeUnit.MILLISECONDS);
        t.subtract(700, TimeUnit.MILLISECONDS);
        // 1.5 s - 0.7 s = 0.8 s
        assertTrue(t.isValid());
        assertEquals(800, t.getTime(TimeUnit.MILLISECONDS));
        t.add(ENV.getSPI().newDuration(2, TimeUnit.SECONDS));
        assertEquals(2800, t.getTime(TimeUnit.MILLISECONDS));
        t.subtract(ENV.getSPI().newDuration(3, TimeUnit.SECONDS));
        // before the epoch: invalid, and an invalid time stays invalid
        assertFalse(t.isValid());
        t.add(10, TimeUnit.SECONDS);
        assertFalse(t.isValid());
    }

    @Test
    void copyFromOverwritesTheStateAndCopiesAnInvalidSourceAsInvalid() {
        ModifiableTime target = ENV.getSPI().newTime(9, TimeUnit.SECONDS);
        target.copyFrom(ENV.getSPI().newTime(1234, TimeUnit.MILLISECONDS));
        assertEquals(1234, target.getTime(TimeUnit.MILLISECONDS));
        assertEquals(ENV.getSPI().newTime(1234, TimeUnit.MILLISECONDS), target);

        target.copyFrom(ENV.getSPI().invalidTime());
        assertFalse(target.isValid());
        assertEquals(ENV.getSPI().invalidTime(), target);

        target.copyFrom(new ForeignTime(2_500_000_000L));
        assertTrue(target.isValid());
        assertEquals(2500, target.getTime(TimeUnit.MILLISECONDS));

        target.copyFrom(new ForeignTime(-1));
        assertFalse(target.isValid());

        assertThrows(IllegalArgumentException.class, () -> target.copyFrom(null));
    }

    @Test
    void setTimeWithANegativeValueGivesTheInvalidTime() {
        ModifiableTime t = ENV.getSPI().newTime(5, TimeUnit.SECONDS);
        t.setTime(-3, TimeUnit.MILLISECONDS);
        assertFalse(t.isValid());
        t.setTime(3, TimeUnit.MILLISECONDS);
        assertTrue(t.isValid());
        assertEquals(3, t.getTime(TimeUnit.MILLISECONDS));
    }

    @Test
    void aTimeOfAnotherImplementationIsComparedByValue() {
        Time ours = ENV.getSPI().newTime(1, TimeUnit.SECONDS);
        ForeignTime foreign = new ForeignTime(1_000_000_000L);
        assertEquals(ours, foreign);
        assertEquals(0, ours.compareTo(foreign));
        assertEquals(ours.hashCode(), foreign.hashCode());
        assertTrue(ours.compareTo(new ForeignTime(2_000_000_000L)) < 0);
        ModifiableTime sum = ENV.getSPI().newTime(0, TimeUnit.SECONDS);
        sum.copyFrom(foreign);
        sum.add(new ForeignDuration(500_000_000L));
        assertEquals(1500, sum.getTime(TimeUnit.MILLISECONDS));
    }

    @Test
    void timesAreSerializableAndKeepTheirValue() throws Exception {
        ModifiableTime modifiable = ENV.getSPI().newTime(1234, TimeUnit.MILLISECONDS);
        ModifiableTime copy = roundTrip(modifiable);
        assertEquals(modifiable, copy);
        assertSame(ENV, copy.getEnvironment());
        Time invalid = roundTrip(ENV.getSPI().invalidTime());
        assertFalse(invalid.isValid());
    }

    // ---- Duration --------------------------------------------------------

    @Test
    void equalDurationsHaveEqualHashCodeAndOrderByLength() {
        Duration a = ENV.getSPI().newDuration(1500, TimeUnit.MILLISECONDS);
        Duration b = ENV.getSPI().newDuration(1, TimeUnit.SECONDS).add(500, TimeUnit.MILLISECONDS);
        assertEquals(a, b);
        assertEquals(a.hashCode(), b.hashCode());
        assertEquals(0, a.compareTo(b));
        assertTrue(a.compareTo(ENV.getSPI().newDuration(2, TimeUnit.SECONDS)) < 0);
        assertEquals(1500, a.getDuration(TimeUnit.MILLISECONDS));
    }

    @Test
    void theInfiniteDurationIsTheLargestAndLongMaxInAnyUnitIsInfinite() {
        Duration infinite = ENV.getSPI().infiniteDuration();
        assertTrue(infinite.isInfinite());
        assertEquals(infinite, ENV.getSPI().newDuration(Long.MAX_VALUE, TimeUnit.SECONDS));
        assertEquals(infinite, ENV.getSPI().newDuration(Long.MAX_VALUE, TimeUnit.NANOSECONDS));
        assertEquals(infinite.hashCode(), ENV.getSPI().newDuration(Long.MAX_VALUE, TimeUnit.DAYS).hashCode());
        assertTrue(infinite.compareTo(ENV.getSPI().newDuration(10_000, TimeUnit.DAYS)) > 0);
        assertTrue(ENV.getSPI().zeroDuration().isZero());
        assertFalse(ENV.getSPI().zeroDuration().isInfinite());
    }

    @Test
    void aDurationOfAnotherImplementationIsAccepted() {
        Duration ours = ENV.getSPI().newDuration(1500, TimeUnit.MILLISECONDS);
        Duration sum = ours.add(new ForeignDuration(250_000_000L));
        // 1.5 s + 0.25 s = 1.75 s
        assertEquals(1750, sum.getDuration(TimeUnit.MILLISECONDS));
        Duration difference = ours.subtract(new ForeignDuration(500_000_000L));
        assertEquals(1000, difference.getDuration(TimeUnit.MILLISECONDS));
        assertEquals(ours, new ForeignDuration(1_500_000_000L));
        assertEquals(0, ours.compareTo(new ForeignDuration(1_500_000_000L)));
    }

    @Test
    void durationsAreSerializableAndKeepTheirValue() throws Exception {
        Duration infinite = roundTrip(ENV.getSPI().infiniteDuration());
        assertTrue(infinite.isInfinite());
        assertSame(ENV, infinite.getEnvironment());
        Duration d = roundTrip(ENV.getSPI().newDuration(250, TimeUnit.MILLISECONDS));
        assertEquals(250, d.getDuration(TimeUnit.MILLISECONDS));
    }

    // ---- InstanceHandle ---------------------------------------------------

    @Test
    void handlesWithTheSameOctetsAreEqualAndHaveTheSameHashCode() {
        InstanceHandle a = org.zerodds.core.Values.of(ENV, octets(1, 2));
        InstanceHandle b = org.zerodds.core.Values.of(ENV, octets(1, 2));
        assertEquals(a, b);
        assertEquals(a.hashCode(), b.hashCode());
        assertEquals(0, a.compareTo(b));
        assertNotEquals(a, org.zerodds.core.Values.of(ENV, octets(1, 3)));
        assertFalse(a.isNil());
        assertTrue(ENV.getSPI().nilHandle().isNil());
    }

    @Test
    void handlesCompareOctetWiseAsUnsignedValues() {
        InstanceHandle high = org.zerodds.core.Values.of(ENV, octets(0x80, 0));
        InstanceHandle low = org.zerodds.core.Values.of(ENV, octets(0x01, 0));
        // 0x80 is 128 unsigned and larger than 0x01, although the byte is negative
        assertTrue(high.compareTo(low) > 0);
        assertTrue(low.compareTo(high) < 0);
    }

    @Test
    void anImmutableAndAModifiableHandleOfTheSameOctetsAreEqualInBothDirections() {
        ModifiableInstanceHandle m = ENV.getSPI().newInstanceHandle();
        assertTrue(m.isNil());
        m.copyFrom(org.zerodds.core.Values.of(ENV, octets(7, 9)));
        InstanceHandle i = org.zerodds.core.Values.of(ENV, octets(7, 9));
        assertFalse(m.isNil());
        assertEquals(i, m);
        assertEquals(m, i);
        assertEquals(i.hashCode(), m.hashCode());
        assertEquals(i, m.immutableCopy());
        assertEquals(m, i.modifiableCopy());
    }

    @Test
    void copyFromOverwritesTheOctetsAndAnEarlierCopyKeepsItsValue() {
        ModifiableInstanceHandle m = ENV.getSPI().newInstanceHandle();
        m.copyFrom(org.zerodds.core.Values.of(ENV, octets(1, 1)));
        InstanceHandle before = m.immutableCopy();
        m.copyFrom(org.zerodds.core.Values.of(ENV, octets(2, 2)));
        assertEquals(org.zerodds.core.Values.of(ENV, octets(2, 2)), m);
        assertEquals(org.zerodds.core.Values.of(ENV, octets(1, 1)), before);
        m.copyFrom(ENV.getSPI().nilHandle());
        assertTrue(m.isNil());
    }

    @Test
    void aHandleOfAnotherImplementationIsAcceptedEverywhere() {
        ForeignHandle foreign = new ForeignHandle(5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20);
        ForeignHandle same = new ForeignHandle(5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20);
        ForeignHandle other = new ForeignHandle(5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 21);
        ModifiableInstanceHandle m = ENV.getSPI().newInstanceHandle();
        m.copyFrom(foreign);
        assertFalse(m.isNil());
        // equal foreign handles give equal handles, different ones different handles
        ModifiableInstanceHandle n = ENV.getSPI().newInstanceHandle();
        n.copyFrom(same);
        assertEquals(m, n);
        assertEquals(m.hashCode(), n.hashCode());
        ModifiableInstanceHandle o = ENV.getSPI().newInstanceHandle();
        o.copyFrom(other);
        assertNotEquals(m, o);
        // equals and compareTo are total
        assertEquals(m, foreign);
        assertEquals(0, m.compareTo(foreign));
        assertEquals(0, ((InstanceHandle) m.immutableCopy()).compareTo(same));
        assertNotEquals(0, m.compareTo(other));
        assertEquals(-Integer.signum(m.compareTo(other)), Integer.signum(o.compareTo(m)));
    }

    @Test
    void aNilHandleOfAnotherImplementationIsTheNilHandle() {
        ForeignHandle nil = new ForeignHandle(0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0);
        InstanceHandle ours = ENV.getSPI().nilHandle();
        assertEquals(ours, nil);
        assertEquals(0, ours.compareTo(nil));
        ModifiableInstanceHandle m = ENV.getSPI().newInstanceHandle();
        m.copyFrom(org.zerodds.core.Values.of(ENV, octets(1, 1)));
        m.copyFrom(nil);
        assertTrue(m.isNil());
    }

    @Test
    void aNullHandleIsABadParameter() {
        ModifiableInstanceHandle m = ENV.getSPI().newInstanceHandle();
        assertThrows(IllegalArgumentException.class, () -> m.copyFrom(null));
    }

    @Test
    void handlesAreSerializableAndKeepTheirOctets() throws Exception {
        InstanceHandle h = org.zerodds.core.Values.of(ENV, octets(3, 4));
        InstanceHandle copy = roundTrip(h);
        assertEquals(h, copy);
        assertSame(ENV, copy.getEnvironment());
        ModifiableInstanceHandle m = h.modifiableCopy();
        ModifiableInstanceHandle mCopy = roundTrip(m);
        assertEquals(m, mCopy);
        assertSame(ENV, mCopy.getEnvironment());
    }

    @Test
    void aBuiltinTopicKeyCopiesItsWordsAndIsBuiltFromAHandleBigEndian() {
        org.zerodds.topic.BuiltinTopicKeyImpl source =
                new org.zerodds.topic.BuiltinTopicKeyImpl(ENV, new int[] {1, 2, 3, 4});
        org.zerodds.topic.BuiltinTopicKeyImpl target =
                new org.zerodds.topic.BuiltinTopicKeyImpl(ENV, new int[] {9, 9, 9, 9});
        target.copyFrom(source);
        assertTrue(Arrays.equals(new int[] {1, 2, 3, 4}, target.getValue()));
        assertEquals(source, target);
        assertEquals(source.hashCode(), target.hashCode());
        assertEquals(source, source.clone());
        // octets 01 00 00 00 | 00.. | 00.. | 00 00 00 02 read as four big-endian words
        org.zerodds.topic.BuiltinTopicKeyImpl fromHandle = org.zerodds.topic.BuiltinTopicKeyImpl.of(
                ENV, org.zerodds.core.Values.of(ENV, octets(1, 2)));
        assertTrue(Arrays.equals(new int[] {16777216, 0, 0, 2}, fromHandle.getValue()));
    }
}
