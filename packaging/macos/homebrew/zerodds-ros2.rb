# Sub-formula: brew install zero-objects/zerodds/zerodds-ros2
# Spec: zerodds-deployment-1.0.md §3.2.1.
# Pure formula over the `zerodds-ros2-shim` binary the `zerodds` formula
# installs. The shim is the RMW's diagnostics CLI: rcl loads the RMW into
# each node, so there is no service — only the example configuration.
class ZeroddsRos2 < Formula
  desc "ROS-2 RMW shim and diagnostics"
  homepage "https://zerodds.org"
  version "1.0.0-rc.7"
  license "Apache-2.0"

  depends_on "zerodds"

  def install
    config = "ros2.yaml"
    source = Formula["zerodds"].opt_pkgshare/"configs/ros2.yaml.example"
    (pkgshare/"configs").mkpath
    cp source, pkgshare/"configs"/config
    (etc/"zerodds").mkpath
    cp pkgshare/"configs"/config, etc/"zerodds"/config
  end
end
