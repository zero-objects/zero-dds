// SPDX-License-Identifier: Apache-2.0
package org.zerodds.cdr;

import java.net.URI;
import java.net.URISyntaxException;
import java.time.DateTimeException;
import java.time.Instant;
import java.time.LocalDate;
import java.time.LocalDateTime;
import java.time.LocalTime;
import java.time.MonthDay;
import java.time.OffsetDateTime;
import java.time.Period;
import java.time.Year;
import java.time.YearMonth;
import java.time.ZoneId;
import java.time.ZoneOffset;
import java.time.ZonedDateTime;
import java.util.Currency;
import java.util.Date;
import java.util.Locale;
import java.util.UUID;

import org.zerodds.wire.Reader;
import org.zerodds.wire.Writer;

/**
 * The DDS form of the {@code Serializable} classes of the JDK that Table 8.1 of DDS-Java-PSM 1.0
 * has no entry for (§8.1: "any Java type that implements Serializable shall be available for
 * publishing and/or subscribing"). The fields of a JDK class cannot be read reflectively, so each
 * class has a form of its own, a structure of Table 8.1 types (FINAL, in the order given, XCDR2,
 * natural alignment):
 *
 * <pre>
 * java.util.Date           int64 milliseconds since the epoch
 * java.util.UUID           int64 most significant bits, int64 least significant bits
 * java.time.Instant        int64 epoch second, int32 nano of second
 * java.time.Duration       int64 seconds, int32 nano of second
 * java.time.Period         int32 years, int32 months, int32 days
 * java.time.LocalDate      int64 epoch day
 * java.time.LocalTime      int64 nano of day
 * java.time.LocalDateTime  int64 epoch day, int64 nano of day
 * java.time.OffsetDateTime a LocalDateTime, int32 offset seconds
 * java.time.ZonedDateTime  a LocalDateTime, int32 offset seconds, string zone id
 * java.time.ZoneOffset     int32 total seconds
 * java.time.Year           int32 year
 * java.time.YearMonth      int32 year, int32 month
 * java.time.MonthDay       int32 month, int32 day
 * java.net.URI             string (UTF-8)
 * java.util.Locale         string, the IETF language tag (UTF-8)
 * java.util.Currency       string, the ISO 4217 code
 * </pre>
 *
 * A value a class refuses (a month 13, a malformed URI) is an {@link XcdrException} on reading.
 */
final class JdkValueTypes {

    private JdkValueTypes() {}

    static boolean supports(Class<?> c) {
        return c == Date.class || c == UUID.class || c == Instant.class
                || c == java.time.Duration.class || c == Period.class || c == LocalDate.class
                || c == LocalTime.class || c == LocalDateTime.class || c == OffsetDateTime.class
                || c == ZonedDateTime.class || c == ZoneOffset.class || c == Year.class
                || c == YearMonth.class || c == MonthDay.class || c == URI.class
                || c == Locale.class || c == Currency.class;
    }

    static Object defaultOf(Class<?> c) {
        if (c == Date.class) {
            return new Date(0L);
        } else if (c == UUID.class) {
            return new UUID(0L, 0L);
        } else if (c == Instant.class) {
            return Instant.EPOCH;
        } else if (c == java.time.Duration.class) {
            return java.time.Duration.ZERO;
        } else if (c == Period.class) {
            return Period.ZERO;
        } else if (c == LocalDate.class) {
            return LocalDate.ofEpochDay(0);
        } else if (c == LocalTime.class) {
            return LocalTime.MIDNIGHT;
        } else if (c == LocalDateTime.class) {
            return LocalDateTime.of(LocalDate.ofEpochDay(0), LocalTime.MIDNIGHT);
        } else if (c == OffsetDateTime.class) {
            return OffsetDateTime.of(LocalDateTime.of(LocalDate.ofEpochDay(0), LocalTime.MIDNIGHT),
                    ZoneOffset.UTC);
        } else if (c == ZonedDateTime.class) {
            return ZonedDateTime.of(LocalDateTime.of(LocalDate.ofEpochDay(0), LocalTime.MIDNIGHT),
                    ZoneOffset.UTC);
        } else if (c == ZoneOffset.class) {
            return ZoneOffset.UTC;
        } else if (c == Year.class) {
            return Year.of(1970);
        } else if (c == YearMonth.class) {
            return YearMonth.of(1970, 1);
        } else if (c == MonthDay.class) {
            return MonthDay.of(1, 1);
        } else if (c == URI.class) {
            return URI.create("");
        } else if (c == Locale.class) {
            return Locale.ROOT;
        }
        return null; // Currency has no neutral value
    }

    private static void localDateTime(Writer w, LocalDateTime v) {
        w.writeI64(v.toLocalDate().toEpochDay());
        w.writeI64(v.toLocalTime().toNanoOfDay());
    }

    private static LocalDateTime localDateTime(Reader r) {
        long day = r.readI64();
        long nano = r.readI64();
        return LocalDateTime.of(LocalDate.ofEpochDay(day), LocalTime.ofNanoOfDay(nano));
    }

    static void write(Writer w, Class<?> c, Object v) {
        if (c == Date.class) {
            w.writeI64(((Date) v).getTime());
        } else if (c == UUID.class) {
            w.writeI64(((UUID) v).getMostSignificantBits());
            w.writeI64(((UUID) v).getLeastSignificantBits());
        } else if (c == Instant.class) {
            w.writeI64(((Instant) v).getEpochSecond());
            w.writeI32(((Instant) v).getNano());
        } else if (c == java.time.Duration.class) {
            w.writeI64(((java.time.Duration) v).getSeconds());
            w.writeI32(((java.time.Duration) v).getNano());
        } else if (c == Period.class) {
            w.writeI32(((Period) v).getYears());
            w.writeI32(((Period) v).getMonths());
            w.writeI32(((Period) v).getDays());
        } else if (c == LocalDate.class) {
            w.writeI64(((LocalDate) v).toEpochDay());
        } else if (c == LocalTime.class) {
            w.writeI64(((LocalTime) v).toNanoOfDay());
        } else if (c == LocalDateTime.class) {
            localDateTime(w, (LocalDateTime) v);
        } else if (c == OffsetDateTime.class) {
            localDateTime(w, ((OffsetDateTime) v).toLocalDateTime());
            w.writeI32(((OffsetDateTime) v).getOffset().getTotalSeconds());
        } else if (c == ZonedDateTime.class) {
            localDateTime(w, ((ZonedDateTime) v).toLocalDateTime());
            w.writeI32(((ZonedDateTime) v).getOffset().getTotalSeconds());
            w.writeString(((ZonedDateTime) v).getZone().getId());
        } else if (c == ZoneOffset.class) {
            w.writeI32(((ZoneOffset) v).getTotalSeconds());
        } else if (c == Year.class) {
            w.writeI32(((Year) v).getValue());
        } else if (c == YearMonth.class) {
            w.writeI32(((YearMonth) v).getYear());
            w.writeI32(((YearMonth) v).getMonthValue());
        } else if (c == MonthDay.class) {
            w.writeI32(((MonthDay) v).getMonthValue());
            w.writeI32(((MonthDay) v).getDayOfMonth());
        } else if (c == URI.class) {
            w.writeString(v.toString());
        } else if (c == Locale.class) {
            w.writeString(((Locale) v).toLanguageTag());
        } else if (c == Currency.class) {
            w.writeString(((Currency) v).getCurrencyCode());
        } else {
            throw new XcdrException(c.getName() + " has no DDS form");
        }
    }

    static Object read(Reader r, Class<?> c) {
        try {
            if (c == Date.class) {
                return new Date(r.readI64());
            } else if (c == UUID.class) {
                long msb = r.readI64();
                return new UUID(msb, r.readI64());
            } else if (c == Instant.class) {
                long s = r.readI64();
                return Instant.ofEpochSecond(s, r.readI32());
            } else if (c == java.time.Duration.class) {
                long s = r.readI64();
                return java.time.Duration.ofSeconds(s, r.readI32());
            } else if (c == Period.class) {
                int y = r.readI32();
                int m = r.readI32();
                return Period.of(y, m, r.readI32());
            } else if (c == LocalDate.class) {
                return LocalDate.ofEpochDay(r.readI64());
            } else if (c == LocalTime.class) {
                return LocalTime.ofNanoOfDay(r.readI64());
            } else if (c == LocalDateTime.class) {
                return localDateTime(r);
            } else if (c == OffsetDateTime.class) {
                LocalDateTime t = localDateTime(r);
                return OffsetDateTime.of(t, ZoneOffset.ofTotalSeconds(r.readI32()));
            } else if (c == ZonedDateTime.class) {
                LocalDateTime t = localDateTime(r);
                ZoneOffset offset = ZoneOffset.ofTotalSeconds(r.readI32());
                return ZonedDateTime.ofStrict(t, offset, ZoneId.of(r.readString()));
            } else if (c == ZoneOffset.class) {
                return ZoneOffset.ofTotalSeconds(r.readI32());
            } else if (c == Year.class) {
                return Year.of(r.readI32());
            } else if (c == YearMonth.class) {
                int y = r.readI32();
                return YearMonth.of(y, r.readI32());
            } else if (c == MonthDay.class) {
                int m = r.readI32();
                return MonthDay.of(m, r.readI32());
            } else if (c == URI.class) {
                return new URI(r.readString());
            } else if (c == Locale.class) {
                return Locale.forLanguageTag(r.readString());
            } else if (c == Currency.class) {
                return Currency.getInstance(r.readString());
            }
        } catch (DateTimeException | URISyntaxException | IllegalArgumentException refused) {
            throw new XcdrException("the value is no " + c.getName() + ": " + refused.getMessage(), refused);
        }
        throw new XcdrException(c.getName() + " has no DDS form");
    }
}
