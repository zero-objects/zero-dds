// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

// A grpc-go client of zerodds-grpc-bridged for
// crates/grpc-bridge/tests/cross_vendor.rs (zerodds-grpc-bridge-1.0 §10):
// the stubs protoc-gen-go and protoc-gen-go-grpc generate from
// crates/grpc-bridge/proto/zerodds/chat/v1/bridge.proto, over h2c.
//
//	grpc-go-client ADDRESS publish HEX...    one Publish stream of these payloads
//	grpc-go-client ADDRESS publish-one HEX   one PublishOne
//	grpc-go-client ADDRESS subscribe N       a Subscribe stream until N samples
//	grpc-go-client ADDRESS catalog           one Catalog
//
// Each result is one line on stdout: `ack seq=S`, `sample type=T topic=P
// flags=F payload=HEX`, `topic dds_name=D dds_type=T grpc_service=S
// idl_url=U`; a call that ends with a status other than OK prints `status
// code=C message=M` and exits 3.
package main

import (
	"context"
	"encoding/hex"
	"fmt"
	"os"
	"strconv"
	"time"

	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials/insecure"
	"google.golang.org/grpc/status"

	chatv1 "zerodds.org/grpcpeer/chatv1"
)

func fail(err error) {
	s, _ := status.FromError(err)
	fmt.Printf("status code=%d message=%s\n", int(s.Code()), s.Message())
	os.Exit(3)
}

func payload(text string) []byte {
	b, err := hex.DecodeString(text)
	if err != nil {
		fmt.Fprintf(os.Stderr, "not hex: %s\n", text)
		os.Exit(2)
	}
	return b
}

func main() {
	if len(os.Args) < 3 {
		fmt.Fprintln(os.Stderr, "usage: grpc-go-client ADDRESS publish|publish-one|subscribe|catalog [ARGS]")
		os.Exit(2)
	}
	address, command, args := os.Args[1], os.Args[2], os.Args[3:]
	conn, err := grpc.NewClient(address, grpc.WithTransportCredentials(insecure.NewCredentials()))
	if err != nil {
		fmt.Fprintf(os.Stderr, "grpc.NewClient: %v\n", err)
		os.Exit(2)
	}
	defer conn.Close()
	client := chatv1.NewChatMessageStreamClient(conn)
	ctx, cancel := context.WithTimeout(context.Background(), 20*time.Second)
	defer cancel()

	switch command {
	case "publish":
		stream, err := client.Publish(ctx)
		if err != nil {
			fail(err)
		}
		for _, text := range args {
			if err := stream.Send(&chatv1.Sample{CdrPayload: payload(text)}); err != nil {
				// The status of the call comes with CloseAndRecv.
				break
			}
		}
		ack, err := stream.CloseAndRecv()
		if err != nil {
			fail(err)
		}
		fmt.Printf("ack seq=%d\n", ack.GetSequenceNumber())
	case "publish-one":
		if len(args) != 1 {
			os.Exit(2)
		}
		ack, err := client.PublishOne(ctx, &chatv1.Sample{CdrPayload: payload(args[0])})
		if err != nil {
			fail(err)
		}
		fmt.Printf("ack seq=%d\n", ack.GetSequenceNumber())
	case "subscribe":
		if len(args) != 1 {
			os.Exit(2)
		}
		n, err := strconv.Atoi(args[0])
		if err != nil {
			os.Exit(2)
		}
		stream, err := client.Subscribe(ctx, &chatv1.SubscribeReq{})
		if err != nil {
			fail(err)
		}
		for i := 0; i < n; i++ {
			s, err := stream.Recv()
			if err != nil {
				fail(err)
			}
			fmt.Printf("sample type=%s topic=%s flags=%d payload=%s\n",
				s.GetTypeName(), s.GetTopicName(), s.GetFlags(), hex.EncodeToString(s.GetCdrPayload()))
		}
	case "catalog":
		resp, err := client.Catalog(ctx, &chatv1.CatalogReq{})
		if err != nil {
			fail(err)
		}
		for _, t := range resp.GetTopics() {
			fmt.Printf("topic dds_name=%s dds_type=%s grpc_service=%s idl_url=%s\n",
				t.GetDdsName(), t.GetDdsType(), t.GetGrpcService(), t.GetIdlUrl())
		}
	default:
		fmt.Fprintf(os.Stderr, "unknown command %s\n", command)
		os.Exit(2)
	}
}
