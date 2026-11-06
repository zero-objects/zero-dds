#!/usr/bin/env python3
"""The foreign side of the gRPC bridge scenario: grpcio, the gRPC
implementation of the gRPC project (HTTP/2, the length-prefixed message of
the gRPC protocol), with the messages written by hand after the bridge's
.proto (zerodds-grpc-bridge-1.0 §4.2): `Sample.cdr_payload` is field 1,
wire type 2 (protobuf encoding, length-delimited).

    grpc_peer.py <addr> <out-service> <count> <in-service> <hex>...

Calls `<out-service>/Subscribe` and prints `sample <hex>` with the
`cdr_payload` of each of the first <count> messages; then calls
`<in-service>/PublishOne` once per <hex> and prints `ack`; then waits until
its standard input ends.
"""
import sys

import grpc


def varint(value):
    out = bytearray()
    while True:
        byte = value & 0x7F
        value >>= 7
        if value:
            out.append(byte | 0x80)
        else:
            out.append(byte)
            return bytes(out)


def read_varint(data, at):
    shift, value = 0, 0
    while True:
        byte = data[at]
        at += 1
        value |= (byte & 0x7F) << shift
        if not byte & 0x80:
            return value, at
        shift += 7


def field_one(message):
    """The bytes of field 1 (wire type 2) of a protobuf message."""
    at = 0
    while at < len(message):
        key, at = read_varint(message, at)
        number, wire = key >> 3, key & 7
        if wire == 2:
            length, at = read_varint(message, at)
            if number == 1:
                return message[at:at + length]
            at += length
        elif wire == 0:
            _, at = read_varint(message, at)
        elif wire == 1:
            at += 8
        elif wire == 5:
            at += 4
        else:
            raise ValueError(f"wire type {wire}")
    return b""


def main(argv):
    addr, out_service, count, in_service, payloads = argv[1], argv[2], int(argv[3]), argv[4], argv[5:]
    identity = lambda b: b  # noqa: E731
    with grpc.insecure_channel(addr) as channel:
        grpc.channel_ready_future(channel).result(timeout=10)
        subscribe = channel.unary_stream(f"/{out_service}/Subscribe",
                                         request_serializer=identity, response_deserializer=identity)
        stream = subscribe(b"", timeout=20)
        print("subscribing", flush=True)
        got = 0
        for message in stream:
            print("sample " + field_one(message).hex(), flush=True)
            got += 1
            if got == count:
                stream.cancel()
                break
        publish = channel.unary_unary(f"/{in_service}/PublishOne",
                                      request_serializer=identity, response_deserializer=identity)
        for payload in payloads:
            data = bytes.fromhex(payload)
            publish(b"\x0a" + varint(len(data)) + data, timeout=10)
            print("ack", flush=True)
        sys.stdin.read()
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
