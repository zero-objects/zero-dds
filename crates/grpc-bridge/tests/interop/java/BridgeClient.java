// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

import io.grpc.ManagedChannel;
import io.grpc.ManagedChannelBuilder;
import io.grpc.Status;
import io.grpc.stub.StreamObserver;
import java.util.Iterator;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicReference;
import zerodds.chat.v1.Bridge.CatalogReq;
import zerodds.chat.v1.Bridge.CatalogResp;
import zerodds.chat.v1.Bridge.PublishAck;
import zerodds.chat.v1.Bridge.Sample;
import zerodds.chat.v1.Bridge.SubscribeReq;
import zerodds.chat.v1.Bridge.TopicEntry;
import zerodds.chat.v1.ChatMessageStreamGrpc;

/**
 * A grpc-java client of zerodds-grpc-bridged for
 * crates/grpc-bridge/tests/cross_vendor.rs (zerodds-grpc-bridge-1.0 §10): the stubs protoc's
 * grpc-java plugin generates from crates/grpc-bridge/proto/zerodds/chat/v1/bridge.proto, over h2c
 * with the Netty transport.
 *
 * <pre>
 *   BridgeClient ADDRESS publish HEX...    one Publish stream of these payloads
 *   BridgeClient ADDRESS publish-one HEX   one PublishOne
 *   BridgeClient ADDRESS subscribe N       a Subscribe stream until N samples
 *   BridgeClient ADDRESS catalog           one Catalog
 * </pre>
 *
 * <p>Each result is one line on stdout: {@code ack seq=S}, {@code sample type=T topic=P flags=F
 * payload=HEX}, {@code topic dds_name=D dds_type=T grpc_service=S idl_url=U}; a call that ends with
 * a status other than OK prints {@code status code=C message=M} and exits 3.
 */
public final class BridgeClient {
  private BridgeClient() {}

  private static com.google.protobuf.ByteString payload(String text) {
    if (text.length() % 2 != 0) {
      throw new IllegalArgumentException("not hex: " + text);
    }
    byte[] out = new byte[text.length() / 2];
    for (int i = 0; i < out.length; i++) {
      out[i] = (byte) Integer.parseInt(text.substring(2 * i, 2 * i + 2), 16);
    }
    return com.google.protobuf.ByteString.copyFrom(out);
  }

  private static String hex(com.google.protobuf.ByteString bytes) {
    StringBuilder out = new StringBuilder();
    for (byte b : bytes.toByteArray()) {
      out.append(String.format("%02x", b & 0xff));
    }
    return out.toString();
  }

  private static void fail(Throwable error) {
    Status status = Status.fromThrowable(error);
    System.out.println(
        "status code=" + status.getCode().value() + " message=" + status.getDescription());
    System.out.flush();
    System.exit(3);
  }

  public static void main(String[] args) throws Exception {
    if (args.length < 2) {
      System.err.println("usage: BridgeClient ADDRESS publish|publish-one|subscribe|catalog [ARGS]");
      System.exit(2);
    }
    ManagedChannel channel = ManagedChannelBuilder.forTarget(args[0]).usePlaintext().build();
    try {
      run(channel, args);
    } catch (io.grpc.StatusRuntimeException error) {
      fail(error);
    } finally {
      channel.shutdownNow().awaitTermination(5, TimeUnit.SECONDS);
    }
  }

  private static void run(ManagedChannel channel, String[] args) throws Exception {
    ChatMessageStreamGrpc.ChatMessageStreamBlockingStub blocking =
        ChatMessageStreamGrpc.newBlockingStub(channel).withDeadlineAfter(20, TimeUnit.SECONDS);
    switch (args[1]) {
      case "publish":
        {
          CountDownLatch done = new CountDownLatch(1);
          AtomicReference<PublishAck> ack = new AtomicReference<>();
          AtomicReference<Throwable> error = new AtomicReference<>();
          StreamObserver<Sample> requests =
              ChatMessageStreamGrpc.newStub(channel)
                  .withDeadlineAfter(20, TimeUnit.SECONDS)
                  .publish(
                      new StreamObserver<PublishAck>() {
                        @Override
                        public void onNext(PublishAck value) {
                          ack.set(value);
                        }

                        @Override
                        public void onError(Throwable t) {
                          error.set(t);
                          done.countDown();
                        }

                        @Override
                        public void onCompleted() {
                          done.countDown();
                        }
                      });
          for (int i = 2; i < args.length; i++) {
            requests.onNext(Sample.newBuilder().setCdrPayload(payload(args[i])).build());
          }
          requests.onCompleted();
          if (!done.await(25, TimeUnit.SECONDS)) {
            System.err.println("no answer to Publish");
            System.exit(2);
          }
          if (error.get() != null) {
            fail(error.get());
          }
          System.out.println("ack seq=" + ack.get().getSequenceNumber());
          break;
        }
      case "publish-one":
        {
          PublishAck ack =
              blocking.publishOne(Sample.newBuilder().setCdrPayload(payload(args[2])).build());
          System.out.println("ack seq=" + ack.getSequenceNumber());
          break;
        }
      case "subscribe":
        {
          int n = Integer.parseInt(args[2]);
          Iterator<Sample> stream = blocking.subscribe(SubscribeReq.getDefaultInstance());
          for (int i = 0; i < n; i++) {
            Sample s = stream.next();
            System.out.println(
                "sample type="
                    + s.getTypeName()
                    + " topic="
                    + s.getTopicName()
                    + " flags="
                    + Integer.toUnsignedString(s.getFlags())
                    + " payload="
                    + hex(s.getCdrPayload()));
            System.out.flush();
          }
          break;
        }
      case "catalog":
        {
          CatalogResp resp = blocking.catalog(CatalogReq.getDefaultInstance());
          for (TopicEntry t : resp.getTopicsList()) {
            System.out.println(
                "topic dds_name="
                    + t.getDdsName()
                    + " dds_type="
                    + t.getDdsType()
                    + " grpc_service="
                    + t.getGrpcService()
                    + " idl_url="
                    + t.getIdlUrl());
          }
          break;
        }
      default:
        System.err.println("unknown command " + args[1]);
        System.exit(2);
    }
  }
}
