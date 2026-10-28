// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
package org.zerodds.wire;

/**
 * What a receiver does with a value its own type cannot hold.
 *
 * <p>DDS-XTypes 1.3 §7.2.2.7. The behaviour belongs to the <i>position</i> and
 * not to the type: the clause attaches it to the member, the element, the map
 * key or the discriminator, and one {@code typedef string<5>} may be used at
 * two positions that want different answers. That is why a generated decoder
 * takes it as a parameter rather than reading it off the type it decodes.
 *
 * <p>The whole value is always read off the wire first. Stopping at the bound
 * would leave the rest of it in the stream and put every following member at
 * the wrong offset; what these three decide is only what is kept.
 */
public enum TryConstruct {
    /**
     * The sample is dropped and the receiver carries on.
     *
     * <p>The default where nothing else is declared, and what
     * {@link Reader.Status#DISCARDED} carries back out.
     */
    DISCARD,
    /**
     * The value is cut to what fits.
     *
     * <p>Table 11 defines it for a string, a wide string and a collection and
     * for nothing else; a declaration that writes it elsewhere is refused by
     * the frontend rather than silently becoming a discard.
     */
    TRIM,
    /**
     * The member takes Table 9's value.
     *
     * <p>Its own {@code @default} where the declaration gave it one —
     * §7.2.2.4.4.4.9 puts the explicit definition first — and the type's
     * otherwise.
     */
    USE_DEFAULT
}
