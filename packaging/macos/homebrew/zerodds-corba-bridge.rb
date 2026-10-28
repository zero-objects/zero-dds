# Sub-formula: brew install zero-objects/zerodds/zerodds-corba-bridge
# Spec: zerodds-deployment-1.0.md §3.2.1.
# Pure service formula: the daemon `zerodds-corba-bridged` ships with the
# `zerodds` formula, this one adds its brew service and example config.
class ZeroddsCorbaBridge < Formula
  desc "ZeroDDS DDS to CORBA GIOP/IIOP bridge daemon"
  homepage "https://zerodds.org"
  version "1.0.0-rc.7"
  license "Apache-2.0"

  depends_on "zerodds"

  def install
    config = "corba-bridged.yaml"
    source = Formula["zerodds"].opt_pkgshare/"configs/#{config}.example"
    (pkgshare/"configs").mkpath
    cp source, pkgshare/"configs"/config
    (etc/"zerodds").mkpath
    cp pkgshare/"configs"/config, etc/"zerodds"/config
  end

  service do
    run [opt_bin/"zerodds-corba-bridged",
         "--config", etc/"zerodds/corba-bridged.yaml"]
    keep_alive true
    log_path var/"log/zerodds/corba-bridged.log"
    error_log_path var/"log/zerodds/corba-bridged.err"
  end
end
