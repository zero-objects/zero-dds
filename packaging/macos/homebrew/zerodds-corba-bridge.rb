class ZeroddsCorbaBridge < Formula
  desc "ZeroDDS DDS to CORBA GIOP/IIOP bridge daemon"
  homepage "https://zerodds.org"
  license "Apache-2.0"
  version "1.0.0-rc.7"
  depends_on "zerodds"
  url "https://github.com/zero-objects/zerodds/releases/download/v#{version}/zerodds-corba-bridge-#{version}.tar.gz"
  sha256 "0000000000000000000000000000000000000000000000000000000000000000"
  service do
    run [opt_bin/"zerodds-corba-bridged", "--config", etc/"zerodds/corba-bridged.yaml"]
    keep_alive true
    log_path var/"log/zerodds/corba-bridged.log"
    error_log_path var/"log/zerodds/corba-bridged.err"
  end
end
