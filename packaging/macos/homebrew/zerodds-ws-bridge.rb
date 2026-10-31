# Sub-formula: brew install zero-objects/zerodds/zerodds-ws-bridge
# Spec: zerodds-deployment-1.0.md §3.2.1.
# Pure service formula: the daemon `zerodds-ws-bridged` ships with the
# `zerodds` formula, this one adds its brew service and example config.
# Homebrew requires a URL in every formula: this one names the bundle of the
# `zerodds` formula (same URL and sha256, downloaded once), and takes only the
# example configuration from it.
class ZeroddsWsBridge < Formula
  desc "ZeroDDS DDS to WebSocket bridge daemon"
  homepage "https://zerodds.org"
  version "1.0.0-rc.7"
  license "Apache-2.0"

  on_arm do
    url "https://github.com/zero-objects/zero-dds/releases/download/v#{version}/zerodds-#{version}-aarch64-apple-darwin.tar.gz"
    sha256 "0000000000000000000000000000000000000000000000000000000000000000"
  end
  on_intel do
    url "https://github.com/zero-objects/zero-dds/releases/download/v#{version}/zerodds-#{version}-x86_64-apple-darwin.tar.gz"
    sha256 "0000000000000000000000000000000000000000000000000000000000000000"
  end

  depends_on "zerodds"

  def install
    config = "ws-bridged.yaml"
    (pkgshare/"configs").install "share/zerodds/configs/#{config}.example" => config
    # An existing configuration is the user's; an upgrade keeps it.
    (etc/"zerodds").mkpath
    cp pkgshare/"configs"/config, etc/"zerodds"/config unless (etc/"zerodds"/config).exist?
  end

  service do
    run [opt_bin/"zerodds-ws-bridged",
         "--config", etc/"zerodds/ws-bridged.yaml"]
    keep_alive true
    log_path var/"log/zerodds/ws-bridged.log"
    error_log_path var/"log/zerodds/ws-bridged.err"
  end
end
