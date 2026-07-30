# Sub-formula: brew install zero-objects/zerodds/zerodds-ws-bridge
# Spec: zerodds-deployment-1.0.md §3.2.1.
class ZeroddsWsBridge < Formula
  desc "ZeroDDS DDS to WebSocket bridge daemon"
  homepage "https://zerodds.org"
  license "Apache-2.0"
  version "1.0.0-rc.7"

  depends_on "zerodds" # core libs + zerodds-ws-bridged binary live in main formula
  depends_on "openssl@3"

  url "https://github.com/zero-objects/zerodds/releases/download/v#{version}/zerodds-ws-bridge-#{version}.tar.gz"
  sha256 "0000000000000000000000000000000000000000000000000000000000000000"

  service do
    run [opt_bin/"zerodds-ws-bridged",
         "--config", etc/"zerodds/ws-bridged.yaml"]
    keep_alive true
    log_path     var/"log/zerodds/ws-bridged.log"
    error_log_path var/"log/zerodds/ws-bridged.err"
  end
end
