# Sub-formula: brew install zero-objects/zerodds/zerodds-ws-bridge
# Spec: zerodds-deployment-1.0.md §3.2.1.
# Pure service formula: the daemon `zerodds-ws-bridged` ships with the
# `zerodds` formula, this one adds its brew service and example config.
class ZeroddsWsBridge < Formula
  desc "ZeroDDS DDS to WebSocket bridge daemon"
  homepage "https://zerodds.org"
  version "1.0.0-rc.7"
  license "Apache-2.0"

  depends_on "zerodds"

  def install
    config = "ws-bridged.yaml"
    source = Formula["zerodds"].opt_pkgshare/"configs/#{config}.example"
    (pkgshare/"configs").mkpath
    cp source, pkgshare/"configs"/config
    (etc/"zerodds").mkpath
    cp pkgshare/"configs"/config, etc/"zerodds"/config
  end

  service do
    run [opt_bin/"zerodds-ws-bridged",
         "--config", etc/"zerodds/ws-bridged.yaml"]
    keep_alive true
    log_path var/"log/zerodds/ws-bridged.log"
    error_log_path var/"log/zerodds/ws-bridged.err"
  end
end
