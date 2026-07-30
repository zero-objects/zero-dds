// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

package zerodds

import "time"

// Client is the synchronous endpoint API — a blocking counterpart to the
// goroutine/channel AsyncReader. Frame + deliver on Write; poll (or block with a
// timeout) on Receive. Same wire, same framing; pick whichever fits your code.
type Client struct {
	Transport Transport
	Session   byte
	Stream    byte
	seq       uint16
	rxbuf     []byte
}

// NewClient returns a sync client over t (best-effort no-key session).
func NewClient(t Transport) *Client {
	return &Client{
		Transport: t,
		Session:   XrceSessionNoKey,
		Stream:    XrceStreamBestEffort,
		seq:       1,
		rxbuf:     make([]byte, 2048),
	}
}

// Write frames and delivers one sample (synchronous), advancing the sequence.
func (c *Client) Write(sample []byte) error {
	frame := XrceWriteFrame(c.Session, c.Stream, c.seq, sample)
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
