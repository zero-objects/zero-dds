class ZeroddsMqttBridge < Formula
  desc "ZeroDDS DDS to MQTT 5 bridge daemon"
  homepage "https://zerodds.org"
  license "Apache-2.0"
  version "1.0.0-rc.7"
  depends_on "zerodds"
  depends_on "openssl@3"
  url "https://github.com/zero-objects/zerodds/releases/download/v#{version}/zerodds-mqtt-bridge-#{version}.tar.gz"
  sha256 "0000000000000000000000000000000000000000000000000000000000000000"
  service do
    run [opt_bin/"zerodds-mqtt-bridged", "--config", etc/"zerodds/mqtt-bridged.yaml"]
    keep_alive true
    log_path var/"log/zerodds/mqtt-bridged.log"
    error_log_path var/"log/zerodds/mqtt-bridged.err"
  end
end
