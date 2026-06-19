# pkg/macos/Formula/zerodds.rb
#
# Homebrew-Formula fuer ZeroDDS. Wird von brew tap zerodds/zerodds
# (oder homebrew-core nach Submission) konsumiert.
#
# Test:
#   brew install --build-from-source ./pkg/macos/Formula/zerodds.rb
#
# Maintainer-Workflow:
#   1. Tag setzen: git tag v0.0.0
#   2. Tarball + sha256 generieren (gitlab-Release oder github-Release).
#   3. `url` + `sha256` in dieser Datei aktualisieren.
#   4. Formula in homebrew-tap-Repository pushen.
class Zerodds < Formula
  desc "Pure-Rust DDS implementation (OMG DDS 1.4 + RTPS 2.5)"
  homepage "https://zerodds.io"
  url "https://gitlab.sandra-kessler.eu/fishermen21/zerodds/-/archive/v0.0.0/zerodds-v0.0.0.tar.gz"
  sha256 "0000000000000000000000000000000000000000000000000000000000000000"
  license "Apache-2.0"
  head "https://gitlab.sandra-kessler.eu/fishermen21/zerodds.git", branch: "main"

  depends_on "rust" => :build

  def install
    # CLI-Tools.
    system "cargo", "install", *std_cargo_args(path: "tools/admin"),     "--bin", "dds-admin"
    system "cargo", "install", *std_cargo_args(path: "tools/perf"),      "--bin", "dds-perf"
    system "cargo", "install", *std_cargo_args(path: "tools/idlc"),      "--bin", "dds-idlc"
    system "cargo", "install", *std_cargo_args(path: "tools/xmlc"),      "--bin", "dds-xmlc"
    system "cargo", "install", *std_cargo_args(path: "tools/chaos"),     "--bin", "dds-chaos"
    system "cargo", "install", *std_cargo_args(path: "tools/bench-suite"),
                              "--bin", "roundtrip-1us"

    # Shared-Library + Header (in lib/ und include/zerodds/).
    system "cargo", "build", "--release", "-p", "dds-c-api"
    lib.install "target/release/libzerodds.dylib"
    (include/"zerodds").install "crates/dds-c-api/include/zerodds.h"
  end

  test do
    # Sanity: jedes Tool darf wenigstens --version oder hw-info aufrufen.
    system "#{bin}/dds-perf", "hw-info"
    system "#{bin}/dds-admin", "--help"
  end
end
