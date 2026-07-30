class ZeroddsCoapBridge < Formula
  desc "ZeroDDS DDS to CoAP bridge daemon"
  homepage "https://zerodds.org"
  license "Apache-2.0"
  version "1.0.0-rc.7"
  depends_on "zerodds"
  url "https://github.com/zero-objects/zerodds/releases/download/v#{version}/zerodds-coap-bridge-#{version}.tar.gz"
  sha256 "0000000000000000000000000000000000000000000000000000000000000000"
  service do
    run [opt_bin/"zerodds-coap-bridged", "--config", etc/"zerodds/coap-bridged.yaml"]
    keep_alive true
    log_path var/"log/zerodds/coap-bridged.log"
    error_log_path var/"log/zerodds/coap-bridged.err"
  end
end
