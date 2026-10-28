// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

package zerodds

import (
	"fmt"
	"time"
)

// Client is the synchronous endpoint API — a blocking counterpart to the
// goroutine/channel AsyncReader. Frame + deliver on Write; poll (or block with a
// timeout) on Receive. Same wire, same framing; pick whichever fits your code.
type Client struct {
	Transport Transport
	Session   byte
	Stream    byte
	// ObjectID names the target DataWriter in every frame's BaseObjectRequest
	// (DDS-XRCE 1.0 §7.7.8).
	ObjectID uint16
	seq      uint16
	rxbuf    []byte
}

// NewClient returns a sync client over t (best-effort no-key session) writing
// to the DataWriter objectID.
func NewClient(t Transport, objectID uint16) *Client {
	return &Client{
		Transport: t,
		Session:   XrceSessionNoKey,
		Stream:    XrceStreamBestEffort,
		ObjectID:  objectID,
		seq:       1,
		rxbuf:     make([]byte, 2048),
	}
}

// Write frames and delivers one sample (synchronous), advancing the sequence.
//
// XrceWriteFrame answers nil for a sample the 16-bit submessage_length cannot
// describe, and this used to hand that nil to Deliver and return its error: an
// oversize sample went out as an empty datagram and Write returned nil. The
// refusal was already representable and this is the caller that has to read it.
// The sequence is not advanced, because nothing was written.
func (c *Client) Write(sample []byte) error {
	frame := XrceWriteFrame(c.Session, c.Stream, c.seq, c.ObjectID, sample)
	if frame == nil {
		return fmt.Errorf(
			"zerodds: %d octets: the 16-bit XRCE submessage_length describes at most %d",
			len(sample), 0xFFFF)
	}
	c.seq++
	return c.Transport.Deliver(frame)
}

// Poll performs one non-blocking receive. ok is false when nothing is ready.
func (c *Client) Poll() (body []byte, ok bool, err error) {
	n, again, err := c.Transport.Receive(c.rxbuf)
	if err != nil || again {
		return nil, false, err
	}
	b, valid := XrceReadFrame(c.rxbuf[:n])
	if !valid {
		return nil, false, nil
	}
	cp := make([]byte, len(b))
	copy(cp, b)
	return cp, true, nil
}

// Receive blocks up to timeout for one sample (polling the transport). ok is
// false on timeout.
func (c *Client) Receive(timeout time.Duration) (body []byte, ok bool, err error) {
	deadline := time.Now().Add(timeout)
	for {
		b, got, e := c.Poll()
		if e != nil {
			return nil, false, e
		}
		if got {
			return b, true, nil
		}
		if time.Now().After(deadline) {
			return nil, false, nil
		}
		time.Sleep(time.Millisecond)
	}
}
