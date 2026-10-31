# Homebrew formula for ZeroDDS — top-level meta-formula.
# Spec: zerodds-deployment-1.0.md §3.2.1.
# Tap: zero-objects/homebrew-zerodds (configured in [workspace.metadata.dist].homebrew-tap).
# Sub-formulae for individual bridges live under Formula/zerodds-*-bridge.rb.
# The bundle this formula installs is packed by
# packaging/macos/bundle/build-bundle.sh in the macOS jobs of release.yml and
# published with the GitHub release; publish-distros.yml renders the sha256.
class Zerodds < Formula
  desc "Rust-native DDS implementation with bridges (DDS 1.4 / RTPS 2.5 / XTypes 1.3)"
  homepage "https://zerodds.org"
  version "1.0.0-rc.7"
  license "Apache-2.0"

  depends_on "openssl@3"

  on_arm do
    url "https://github.com/zero-objects/zero-dds/releases/download/v#{version}/zerodds-#{version}-aarch64-apple-darwin.tar.gz"
    sha256 "0000000000000000000000000000000000000000000000000000000000000000"
  end
  on_intel do
    url "https://github.com/zero-objects/zero-dds/releases/download/v#{version}/zerodds-#{version}-x86_64-apple-darwin.tar.gz"
    sha256 "0000000000000000000000000000000000000000000000000000000000000000"
  end

  def install
    # Binaries (7 daemons + 17 CLIs, see §1.1/§1.3).
    bin.install Dir["bin/zerodds-*"]

    # libzerodds (ABI v1) + headers + pkg-config.
    lib.install Dir["lib/libzerodds.*"]
    include.install "include/zerodds.h"
    (include/"zerodds").install Dir["include/zerodds/*"]
    (include).install "include/dds" if Dir.exist?("include/dds")
    (lib/"pkgconfig").install "lib/pkgconfig/zerodds.pc"

    # Default configs go to #{etc}/zerodds; existing files are kept.
    # `etc.install example => basename` put them into #{etc} itself, the
    # services read #{etc}/zerodds/<daemon>.yaml and found nothing.
    # `cp` instead of `install`: the pkgshare template stays in place, the
    # sub-formulae read it from there.
    pkgshare.install Dir["share/zerodds/*"]
    (etc/"zerodds").mkpath
    %w[ws-bridged mqtt-bridged coap-bridged amqp-bridged
       grpc-bridged corba-bridged].each do |daemon|
      example = pkgshare/"configs/#{daemon}.yaml.example"
      target  = etc/"zerodds/#{daemon}.yaml"
      cp example, target unless target.exist?
    end

    # man-pages.
    man1.install Dir["share/man/man1/zerodds-*.1"]
    man5.install Dir["share/man/man5/zerodds*.yaml.5"]

    # launchd plists (system daemons under /Library/LaunchDaemons,
    # user agents under ~/Library/LaunchAgents — we put templates
    # into pkgshare and the admin decides deliberately).
    (pkgshare/"launchd").install Dir["share/launchd/*.plist"]

    # State/log directories of the services. `brew style` would reject
    # `post_install` as deprecated; the directories are global
    # (not keg content), so they are created here.
    (var/"log/zerodds").mkpath
    (var/"lib/zerodds").mkpath
    (etc/"zerodds/certs").mkpath
  end

  service do
    # The default service is the WS bridge. Other bridges via sub-formula.
    run [opt_bin/"zerodds-ws-bridged",
         "--config", etc/"zerodds/ws-bridged.yaml"]
    keep_alive true
    log_path var/"log/zerodds/ws-bridged.log"
    error_log_path var/"log/zerodds/ws-bridged.err"
    working_dir var/"lib/zerodds"
  end

  def caveats
    <<~EOS
      The services read #{etc}/zerodds/<daemon>.yaml (passed as --config).
      A program started by hand without --config reads the system
      directory /usr/local/etc/zerodds (zerodds-deployment-1.0 §3.1).
    EOS
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/zerodds-admin --version")
  end
end
