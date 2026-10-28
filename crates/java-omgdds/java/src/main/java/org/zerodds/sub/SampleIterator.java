// SPDX-License-Identifier: Apache-2.0
package org.zerodds.sub;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.NoSuchElementException;

import org.omg.dds.sub.Sample;

/**
 * ZeroDDS {@link Sample.Iterator} (DDS-Java-PSM 1.0 §7.7.3).
 *
 * <p>A read or take hands the application an iterator over the snapshot the
 * operation took, so the iterator keeps working while samples keep arriving.
 * {@link #close()} ends the loan (return_loan, DDS-Java-PSM 1.0 §7.7.3); the samples are
 * copies, so nothing is held, and closing again has no effect (Closeable). The iterator is
 * read-only: {@code remove}, {@code set} and {@code add} throw
 * {@code UnsupportedOperationException} (§7.7.2).
 */
public final class SampleIterator<TYPE> implements Sample.Iterator<TYPE> {

    private final List<Sample<TYPE>> samples;
    private int cursor = 0;
    private boolean closed = false;

    public SampleIterator(List<Sample<TYPE>> samples) {
        this.samples = samples;
    }

    /** The samples this iterator walks, as an unmodifiable list. */
    public List<Sample<TYPE>> samples() {
        return Collections.unmodifiableList(samples);
    }

    @Override
    public boolean hasNext() {
        return cursor < samples.size();
    }

    @Override
    public Sample<TYPE> next() {
        if (!hasNext()) {
            throw new NoSuchElementException();
        }
        return samples.get(cursor++);
    }

    @Override
    public boolean hasPrevious() {
        return cursor > 0;
    }

    @Override
    public Sample<TYPE> previous() {
        if (!hasPrevious()) {
            throw new NoSuchElementException();
        }
        return samples.get(--cursor);
    }

    @Override
    public int nextIndex() {
        return cursor;
    }

    @Override
    public int previousIndex() {
        return cursor - 1;
    }

    @Override
    public void remove() {
        throw new UnsupportedOperationException("a Sample.Iterator gives read-only access");
    }

    @Override
    public void set(Sample<TYPE> o) {
        throw new UnsupportedOperationException("a Sample.Iterator gives read-only access");
    }

    @Override
    public void add(Sample<TYPE> o) {
        throw new UnsupportedOperationException("a Sample.Iterator gives read-only access");
    }

    @Override
    public void close() {
        // Closeable.close and Sample.Iterator.close: closing again has no effect.
        closed = true;
    }

    /** Whether {@link #close()} was called. */
    public boolean isClosed() {
        return closed;
    }

    /** The samples as a plain list, for a caller that does not walk an iterator. */
    public List<Sample<TYPE>> asList() {
        return new ArrayList<>(samples);
    }

    @Override
    public String toString() {
        return "Sample.Iterator" + samples.size();
    }
}
