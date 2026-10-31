# Sub-formula: brew install zero-objects/zerodds/zerodds-ros2
# Spec: zerodds-deployment-1.0.md §3.2.1.
# Pure formula over the `zerodds-ros2-shim` binary the `zerodds` formula
# installs. The shim is the RMW's diagnostics CLI: rcl loads the RMW into
# each node, so there is no service — only the example configuration.
# Homebrew requires a URL in every formula: this one names the bundle of the
# `zerodds` formula (same URL and sha256, downloaded once), and takes only the
# example configuration from it.
class ZeroddsRos2 < Formula
  desc "ROS-2 RMW shim and diagnostics"
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
    config = "ros2.yaml"
    (pkgshare/"configs").install "share/zerodds/configs/#{config}.example" => config
    # An existing configuration is the user's; an upgrade keeps it.
    (etc/"zerodds").mkpath
    cp pkgshare/"configs"/config, etc/"zerodds"/config unless (etc/"zerodds"/config).exist?
  end
end
