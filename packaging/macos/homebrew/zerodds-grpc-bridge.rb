class ZeroddsGrpcBridge < Formula
  desc "ZeroDDS DDS to gRPC bridge daemon"
  homepage "https://zerodds.org"
  license "Apache-2.0"
  version "1.0.0-rc.7"
  depends_on "zerodds"
  url "https://github.com/zero-objects/zerodds/releases/download/v#{version}/zerodds-grpc-bridge-#{version}.tar.gz"
  sha256 "0000000000000000000000000000000000000000000000000000000000000000"
  service do
    run [opt_bin/"zerodds-grpc-bridged", "--config", etc/"zerodds/grpc-bridged.yaml"]
    keep_alive true
    log_path var/"log/zerodds/grpc-bridged.log"
    error_log_path var/"log/zerodds/grpc-bridged.err"
  end
end
