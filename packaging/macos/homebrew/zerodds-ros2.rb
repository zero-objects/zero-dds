class ZeroddsRos2 < Formula
  desc "ZeroDDS ROS-2 RMW shim and diagnostics"
  homepage "https://zerodds.org"
  license "Apache-2.0"
  version "1.0.0-rc.1"
  depends_on "zerodds"
  url "https://github.com/zero-objects/zerodds/releases/download/v#{version}/zerodds-ros2-#{version}.tar.gz"
  sha256 "0000000000000000000000000000000000000000000000000000000000000000"
  # Diagnose-Tool: kein Auto-Service, On-Demand.
end
