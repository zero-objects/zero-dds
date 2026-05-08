# Homebrew formula for ZeroDDS — top-level meta-formula.
# Spec: zerodds-deployment-1.0.md §3.2.1.
# Tap: zero-objects/homebrew-zerodds (configured in [workspace.metadata.dist].homebrew-tap).
# Sub-formulae for individual bridges live under Formula/zerodds-*-bridge.rb.
class Zerodds < Formula
  desc "ZeroDDS — pure-Rust DDS implementation with bridges (DDS 1.4 / RTPS 2.5 / XTypes 1.3)"
  homepage "https://zerodds.org"
  license "Apache-2.0"
  version "1.0.0-rc.1"

  depends_on "openssl@3"

  if Hardware::CPU.arm?
    url "https://github.com/zero-objects/zerodds/releases/download/v#{version}/zerodds-#{version}-aarch64-apple-darwin.tar.gz"
    sha256 "0000000000000000000000000000000000000000000000000000000000000000"
  else
    url "https://github.com/zero-objects/zerodds/releases/download/v#{version}/zerodds-#{version}-x86_64-apple-darwin.tar.gz"
    sha256 "0000000000000000000000000000000000000000000000000000000000000000"
  end

  def install
    # Binaries (7 daemons + 17 CLIs).
    bin.install Dir["bin/zerodds-*"]

    # libzerodds (ABI v1) + headers + pkg-config.
    lib.install Dir["lib/libzerodds.*"]
    include.install "include/zerodds.h"
    (include/"zerodds").install Dir["include/zerodds/*"]
    (lib/"pkgconfig").install "lib/pkgconfig/zerodds.pc"

    # Default-Configs; brew installiert sie nach #{etc} und respektiert User-Edits.
    pkgshare.install Dir["share/zerodds/*"]
    %w[ws-bridged mqtt-bridged coap-bridged amqp-bridged
       grpc-bridged corba-bridged ros2-shim].each do |daemon|
      example = pkgshare/"configs/#{daemon}.yaml.example"
      target  = etc/"zerodds/#{daemon}.yaml"
      etc.install example => target.basename unless target.exist?
    end

    # man-pages.
    man1.install Dir["share/man/man1/zerodds-*.1"]
    man5.install Dir["share/man/man5/zerodds-*.yaml.5"]

    # launchd-plists (System-Daemons unter /Library/LaunchDaemons,
    # User-Agents unter ~/Library/LaunchAgents — wir packen Vorlagen
    # in pkgshare und der Admin entscheidet bewusst).
    (pkgshare/"launchd").install Dir["share/launchd/*.plist"]
  end

  service do
    # Default-Service ist die WS-Bridge. Andere Bridges via Sub-Formula.
    run [opt_bin/"zerodds-ws-bridged",
         "--config", etc/"zerodds/ws-bridged.yaml"]
    keep_alive true
    log_path     var/"log/zerodds/ws-bridged.log"
    error_log_path var/"log/zerodds/ws-bridged.err"
    working_dir  var/"lib/zerodds"
  end

  def post_install
    (var/"log/zerodds").mkpath
    (var/"lib/zerodds").mkpath
    (etc/"zerodds/certs").mkpath
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/zerodds-admin --version")
  end
end
