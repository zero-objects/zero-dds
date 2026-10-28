# pkg/macos/Formula/zerodds.rb
#
# Homebrew formula for ZeroDDS. Consumed by brew tap zerodds/zerodds
# (or by homebrew-core after submission).
#
# Test:
#   brew install --build-from-source ./pkg/macos/Formula/zerodds.rb
#
# Maintainer-Workflow:
#   1. Set the tag: git tag v0.0.0
#   2. Generate the tarball + sha256 (GitLab release or GitHub release).
#   3. Update `url` + `sha256` in this file.
#   4. Push the formula to the homebrew-tap repository.
class Zerodds < Formula
  desc "Pure-Rust DDS implementation (OMG DDS 1.4 + RTPS 2.5)"
  homepage "https://zerodds.io"
  url "https://github.com/zero-objects/zero-dds/archive/refs/tags/v0.0.0.tar.gz"
  sha256 "0000000000000000000000000000000000000000000000000000000000000000"
  license "Apache-2.0"
  head "https://github.com/zero-objects/zero-dds.git", branch: "main"

  depends_on "rust" => :build

  def install
    # CLI tools.
    system "cargo", "install", *std_cargo_args(path: "tools/admin"),     "--bin", "dds-admin"
    system "cargo", "install", *std_cargo_args(path: "tools/perf"),      "--bin", "dds-perf"
    system "cargo", "install", *std_cargo_args(path: "tools/idlc"),      "--bin", "dds-idlc"
    system "cargo", "install", *std_cargo_args(path: "tools/xmlc"),      "--bin", "dds-xmlc"
    system "cargo", "install", *std_cargo_args(path: "tools/chaos"),     "--bin", "dds-chaos"
    system "cargo", "install", *std_cargo_args(path: "tools/bench-suite"),
                              "--bin", "roundtrip-1us"

    # Shared library + header (in lib/ and include/zerodds/).
    system "cargo", "build", "--release", "-p", "dds-c-api"
    lib.install "target/release/libzerodds.dylib"
    (include/"zerodds").install "crates/dds-c-api/include/zerodds.h"
  end

  test do
    # Sanity: every tool may at least call --version or hw-info.
    system "#{bin}/dds-perf", "hw-info"
    system "#{bin}/dds-admin", "--help"
  end
end
