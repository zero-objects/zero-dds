#!/usr/bin/env bash
# Installs the foreign programs the integration pillar's scenarios talk to,
# when the job's image does not carry them already
# (internal/release/rc8-test-suite.md, "Pillars -> Integration").
#
#     bash tests/integration/provision.sh <tool>...
#
# Each tool is looked for first (PATH, then /opt/<sdk> where the common image
# keeps the SDKs that Debian does not ship) and installed only when missing:
# Debian packages with one apt-get call, the other SDKs from their release
# archives, at the versions below. The script prints what was
# present and what it installed, and one `PATH+=<dir>` line per directory a
# caller has to put in front of PATH (ci/run_integration.py reads them).
set -euo pipefail

NIM_VERSION="${NIM_VERSION:-2.2.10}"
KOTLIN_VERSION="${KOTLIN_VERSION:-2.0.21}"
JULIA_SERIES="${JULIA_SERIES:-1.12}"
JULIA_VERSION="${JULIA_VERSION:-1.12.6}"
ZIG_VERSION="${ZIG_VERSION:-0.16.0}"
SWIFT_VERSION="${SWIFT_VERSION:-6.0.3}"
# grpcurl (grpc-go): the reference client of zerodds-grpc-bridge's tests.
GRPCURL_VERSION="${GRPCURL_VERSION:-1.9.3}"
# JacORB and the Java 8 JDK it runs on (Temurin): the foreign ORB of the
# zerodds-corba-dds-bridge and zerodds-corba-interop tests, which find them
# under /opt/jacorb and /opt/jdk8 (JACORB_HOME, JACORB_JAVA_HOME).
JACORB_VERSION="${JACORB_VERSION:-3.9}"
TEMURIN8_RELEASE="${TEMURIN8_RELEASE:-jdk8u492-b09}"
DOTNET_CHANNEL="${DOTNET_CHANNEL:-8.0}"
GRADLE_VERSION="${GRADLE_VERSION:-8.10.2}"
# The wasm-bindgen CLI of the JavaScript tests of zerodds-ts-wasm
# (tests/integration/ts_wasm.sh): the version of the wasm-bindgen crate in
# Cargo.lock, which that script checks; the release archive's sha256.
WASM_BINDGEN_VERSION="${WASM_BINDGEN_VERSION:-0.2.127}"
WASM_BINDGEN_SHA256="${WASM_BINDGEN_SHA256:-61d4a7dc85acfa0d2354ccc0b8361928c7e52a746d17f28ebaa795ed3dc1614a}"
# The Flutter SDK of the dds-chat tutorial's Flutter app
# (tests/integration/flutter_chat.sh), its stable release archive for Linux
# and the archive's sha256 from releases_linux.json of flutter_infra_release.
FLUTTER_VERSION="${FLUTTER_VERSION:-3.47.7}"
FLUTTER_SHA256="${FLUTTER_SHA256:-af7455f540df951184c7953fbe3cf884766e7541dacf8571f9f582db03dd6230}"

present=()
packages=()
archives=()
# Where Debian's softhsm2 package puts the PKCS#11 module.
SOFTHSM_MODULE="${SOFTHSM_MODULE:-/usr/lib/softhsm/libsofthsm2.so}"

have() { command -v "$1" >/dev/null 2>&1; }
py_has() { python3 -c "import $1" >/dev/null 2>&1; }
# A tool from an archive: on PATH, or under /opt/<dir> (the common image).
on_path_or_opt() {
  local program="$1" dir="$2"
  if have "$program"; then return 0; fi
  if [ -x "$dir/$program" ]; then echo "PATH+=$dir"; export PATH="$dir:$PATH"; return 0; fi
  return 1
}

want_packages() { packages+=("$@"); }

for tool in "$@"; do
  case "$tool" in
    openssl) have openssl && present+=(openssl) || want_packages openssl ;;
    strace) have strace && present+=(strace) || want_packages strace ;;
    sqlite3) have sqlite3 && present+=(sqlite3) || want_packages sqlite3 ;;
    tcpdump) { have tcpdump || [ -x /usr/sbin/tcpdump ]; } && present+=(tcpdump) || want_packages tcpdump ;;
    iproute2) { have ip || [ -x /usr/sbin/ip ] || [ -x /sbin/ip ]; } && present+=(iproute2) || want_packages iproute2 ;;
    cmake) have cmake && have ctest && present+=(cmake) || want_packages cmake ;;
    # Cyclone DDS's ddsperf (Debian's cyclonedds-tools, 0.10.2 in bookworm): the
    # foreign participant of crates/py's test_shapesdemo_interop.py.
    cyclonedds) have ddsperf && present+=(cyclonedds) || want_packages cyclonedds-tools ;;
    mosquitto)
      if have mosquitto || [ -x /usr/sbin/mosquitto ]; then
        have mosquitto_sub && present+=(mosquitto) || want_packages mosquitto-clients
      else
        want_packages mosquitto mosquitto-clients
      fi ;;
    libcoap) have coap-client-notls || have coap-client-openssl && present+=(libcoap) || want_packages libcoap3-bin ;;
    websockets) py_has websockets.sync.client && present+=(websockets) || want_packages python3-websockets ;;
    grpcio) py_has grpc && present+=(grpcio) || want_packages python3-grpcio ;;
    rabbitmq)
      { have rabbitmq-server || [ -x /usr/sbin/rabbitmq-server ]; } && py_has pika && present+=(rabbitmq) \
        || want_packages rabbitmq-server python3-pika ;;
    # qpid-proton's Python binding: the AMQP 1.0 reference client of the
    # zerodds-amqp-0-9-1 and zerodds-amqp-endpoint tests.
    proton) py_has proton && present+=(proton) || want_packages python3-qpid-proton ;;
    # PostgreSQL's server programs (initdb, postgres; Debian keeps them in
    # /usr/lib/postgresql/<version>/bin) and the `postgres` user they run as
    # under root: the zerodds-durability-store-postgres tests.
    postgresql) compgen -G '/usr/lib/postgresql/*/bin/initdb' >/dev/null && present+=(postgresql) \
        || want_packages postgresql ;;
    open62541) pkg-config --exists open62541 2>/dev/null && present+=(open62541) \
        || want_packages libopen62541-1.4-dev pkg-config ;;
    omniorb) have omniidl && have omniNames && present+=(omniorb) \
        || want_packages omniorb omniidl omniorb-nameserver libomniorb4-dev ;;
    # OpenSSL's headers and libraries: omniORB's SSL transport
    # (omniORB4/sslContext.h includes openssl/ssl.h) for the SSLIOP programs
    # of crates/corba-interop/competitors/build_peers.sh.
    libssl) [ -e /usr/include/openssl/ssl.h ] && present+=(libssl) || want_packages libssl-dev ;;
    go) have go && present+=(go) || want_packages golang-go ;;
    gdc) have gdc && present+=(gdc) || want_packages gdc ;;
    gnat) have gprbuild && present+=(gnat) || want_packages gnat gprbuild ;;
    elixir) have elixir && present+=(elixir) || want_packages erlang-base erlang-crypto elixir ;;
    lua)
      { have lua5.4 && lua5.4 -e 'require("socket")' >/dev/null 2>&1; } && present+=(lua) \
        || want_packages lua5.4 lua-socket ;;
    ocaml) have ocamlfind && present+=(ocaml) || want_packages ocaml-nox ocaml-findlib ;;
    jdk) have javac && present+=(jdk) || want_packages default-jdk-headless ;;
    # Maven for the Java PSM (crates/java-omgdds/java/pom.xml), Debian's package.
    maven) have mvn && present+=(maven) || want_packages maven ;;
    unzip) have unzip && present+=(unzip) || want_packages unzip ;;
    # SoftHSM with OpenSC's pkcs11-tool, for the `pkcs11:` identity test of
    # zerodds-security-runtime (tools/pkcs11/softhsm-fixture.sh).
    softhsm) have softhsm2-util && have pkcs11-tool && [ -e "$SOFTHSM_MODULE" ] \
        && present+=(softhsm) || want_packages softhsm2 opensc ;;
    zig|nim|julia|dotnet|dotnet9|kotlin|swift|grpcurl|jacorb|gradle|wasm|flutter) archives+=("$tool") ;;
    *) echo "provision: unknown tool '$tool'" >&2; exit 2 ;;
  esac
done

# The archives the image does not carry, and the Debian packages they need.
missing=()
need_icu=0
for sdk in "${archives[@]}"; do
  case "$sdk" in
    zig) on_path_or_opt zig /opt/zig ;;
    nim) on_path_or_opt nim /opt/nim/bin ;;
    julia) on_path_or_opt julia /opt/julia/bin ;;
    dotnet) on_path_or_opt dotnet /opt/dotnet ;;
    # The SDK 9 that crates/cs/csharp/global.json asks for (its projects target net8.0).
    dotnet9) on_path_or_opt dotnet /opt/dotnet && dotnet --list-sdks | grep -q '^9\.' ;;
    kotlin) on_path_or_opt kotlinc /opt/kotlinc/kotlinc/bin || on_path_or_opt kotlinc /opt/kotlinc/bin ;;
    swift) on_path_or_opt swiftc /opt/swift/usr/bin ;;
    grpcurl) on_path_or_opt grpcurl /opt/grpcurl ;;
    jacorb) [ -f /opt/jacorb/lib/idl.jar ] && [ -x /opt/jdk8/bin/javac ] ;;
    gradle) on_path_or_opt gradle /opt/gradle/bin ;;
    # Rust's wasm32-unknown-unknown and the wasm-bindgen CLI of the pin.
    wasm) rustup target list --installed 2>/dev/null | grep -qx wasm32-unknown-unknown \
        && on_path_or_opt wasm-bindgen /opt/wasm-bindgen \
        && [ "$(wasm-bindgen --version | awk '{print $2}')" = "$WASM_BINDGEN_VERSION" ] ;;
    flutter) on_path_or_opt flutter /opt/flutter/bin ;;
  esac && { present+=("$sdk"); continue; }
  missing+=("$sdk")
  case "$sdk" in
    kotlin) have javac || want_packages default-jdk-headless; have unzip || want_packages unzip ;;
    jacorb) have unzip || want_packages unzip ;;
    # The flutter tool asks git for its version and unpacks with xz.
    flutter) have git || want_packages git; have xz || want_packages xz-utils
      have unzip || want_packages unzip ;;
    # Debian ships Gradle 4.4, too old for the Kotlin DSL of build-integration/java-gradle.
    gradle) have javac || want_packages default-jdk-headless; have unzip || want_packages unzip ;;
    dotnet|dotnet9) need_icu=1 ;;
    # What the Swift toolchain links at run time and SwiftPM links against
    # (swift.org's list for Debian 12, by the -dev packages, whose runtime
    # library names change between Debian releases): swiftc needs
    # libncurses.so.6 to start (measured in job 227800, ci-rust image);
    # Foundation reads the time zone database.
    swift) need_icu=1
      want_packages libncurses-dev libedit-dev libxml2-dev libcurl4-openssl-dev libsqlite3-dev \
        zlib1g-dev libpython3-dev binutils tzdata ;;
  esac
done

# The downloaded Debian packages in ZERODDS_APT_CACHE, a directory of the
# job's slot on the runner host (/zerodds-cache/apt-<slot> on the CI runner host,
# ci/integration.yml), which only one job at a time uses: a job downloads
# what the slot's earlier jobs did not, and unpacks from there.
apt_options=()
if [ -n "${ZERODDS_APT_CACHE:-}" ] && mkdir -p "$ZERODDS_APT_CACHE/partial" 2>/dev/null; then
  apt_options=(-o "Dir::Cache::Archives=$ZERODDS_APT_CACHE")
fi

if [ ${#packages[@]} -gt 0 ] || [ "$need_icu" = 1 ]; then
  apt_started=$SECONDS
  apt-get "${apt_options[@]}" update -qq
  apt_updated=$SECONDS
  if [ "$need_icu" = 1 ]; then
    icu="$(apt-cache search --names-only '^libicu[0-9]+$' | cut -d' ' -f1 | sort -V | tail -1)"
    [ -n "$icu" ] && want_packages "$icu"
  fi
  # The gold linker Swift on Linux links with, where the distribution
  # still ships it as a package of its own.
  case " ${missing[*]} " in
    *" swift "*) apt-cache show binutils-gold >/dev/null 2>&1 && want_packages binutils-gold ;;
  esac
  echo "provision: apt-get install ${packages[*]}"
  DEBIAN_FRONTEND=noninteractive apt-get "${apt_options[@]}" install -y -qq --no-install-recommends \
    "${packages[@]}" >/dev/null
  echo "provision: apt-get update $((apt_updated - apt_started)) s, install $((SECONDS - apt_updated)) s${apt_options:+, packages from $ZERODDS_APT_CACHE}"
fi

# A transfer that stalls is given up after 30 s below 1 KiB/s and retried:
# without a limit, the download of JacORB 3.9 waited out the 5 minutes of its
# job (pipeline 9186, job 231150, 280 s in the archives step). Each download
# prints its duration.
fetch() {
  local started=$SECONDS
  curl -fsSL --connect-timeout 20 --speed-limit 1024 --speed-time 30 \
    --retry 5 --retry-delay 3 --retry-connrefused --retry-all-errors "$1" -o "$2"
  echo "provision: downloaded ${1##*/} in $((SECONDS - started)) s"
}

# The release archives once per runner host instead of once per job:
# ZERODDS_TOOL_CACHE names a directory every job of the host mounts
# (/zerodds-cache/tools on the CI runner host, ci/integration.yml). An SDK installed there
# under its name and version is linked to its place under /opt; one that is
# not yet there is installed as before and then copied there, under a
# temporary name renamed into place, so that two jobs installing it at the
# same time leave one copy. A job reads the cached SDK and writes nothing to
# it. `dotnet` and `dotnet9` are keyed by their channel: the cached SDK is
# the patch release the first job got.
cache_key() {
  case "$1" in
    zig) echo "zig-$ZIG_VERSION" ;;
    nim) echo "nim-$NIM_VERSION" ;;
    julia) echo "julia-$JULIA_VERSION" ;;
    dotnet) echo "dotnet-$DOTNET_CHANNEL" ;;
    dotnet9) echo "dotnet-9.0" ;;
    kotlin) echo "kotlin-$KOTLIN_VERSION" ;;
    gradle) echo "gradle-$GRADLE_VERSION" ;;
    swift) echo "swift-$SWIFT_VERSION" ;;
    grpcurl) echo "grpcurl-$GRPCURL_VERSION" ;;
    flutter) echo "flutter-$FLUTTER_VERSION" ;;
    jacorb) echo "jacorb-$JACORB_VERSION-$TEMURIN8_RELEASE" ;;
  esac
}
cache_dir_of() {
  case "$1" in
    zig) echo /opt/zig ;; nim) echo /opt/nim ;; julia) echo /opt/julia ;;
    dotnet|dotnet9) echo /opt/dotnet ;; kotlin) echo /opt/kotlinc ;; gradle) echo /opt/gradle ;;
    swift) echo /opt/swift ;; grpcurl) echo /opt/grpcurl ;; flutter) echo /opt/flutter ;;
    jacorb) echo /opt/jacorb-sdk ;;
  esac
}
cache_path() {
  case "$1" in
    zig) echo "PATH+=/opt/zig" ;; nim) echo "PATH+=/opt/nim/bin" ;; julia) echo "PATH+=/opt/julia/bin" ;;
    dotnet|dotnet9) echo "PATH+=/opt/dotnet" ;; kotlin) echo "PATH+=/opt/kotlinc/kotlinc/bin" ;;
    gradle) echo "PATH+=/opt/gradle/bin" ;; swift) echo "PATH+=/opt/swift/usr/bin" ;;
    grpcurl) echo "PATH+=/opt/grpcurl" ;; flutter) echo "PATH+=/opt/flutter/bin" ;;
    # JacORB and its JDK go into the cache as one directory; the tests find
    # them under /opt/jacorb and /opt/jdk8 (JACORB_HOME, JACORB_JAVA_HOME).
    jacorb) rm -rf /opt/jacorb /opt/jdk8
      ln -s /opt/jacorb-sdk/jacorb /opt/jacorb && ln -s /opt/jacorb-sdk/jdk8 /opt/jdk8 ;;
  esac
}
from_cache() {
  local key dir
  key="$(cache_key "$1")"; dir="$(cache_dir_of "$1")"
  [ -n "${ZERODDS_TOOL_CACHE:-}" ] && [ -n "$key" ] && [ -f "$ZERODDS_TOOL_CACHE/$key/.complete" ] || return 1
  rm -rf "$dir" && mkdir -p "$(dirname "$dir")" && ln -s "$ZERODDS_TOOL_CACHE/$key" "$dir"
  cache_path "$1"
}
to_cache() {
  local key dir partial
  key="$(cache_key "$1")"; dir="$(cache_dir_of "$1")"
  [ -n "${ZERODDS_TOOL_CACHE:-}" ] && [ -n "$key" ] && [ -d "$dir" ] || return 0
  [ -e "$ZERODDS_TOOL_CACHE/$key" ] && return 0
  mkdir -p "$ZERODDS_TOOL_CACHE" || return 0
  partial="$ZERODDS_TOOL_CACHE/.partial-$key-$$"
  if cp -a "$dir" "$partial" && touch "$partial/.complete"; then
    mv -T "$partial" "$ZERODDS_TOOL_CACHE/$key" 2>/dev/null || rm -rf "$partial"
  else
    rm -rf "$partial"
  fi
}

installed=()
cached=()
for sdk in "${missing[@]}"; do
  if from_cache "$sdk"; then
    cached+=("$sdk")
    continue
  fi
  case "$sdk" in
    zig)
      fetch "https://ziglang.org/download/${ZIG_VERSION}/zig-x86_64-linux-${ZIG_VERSION}.tar.xz" /tmp/zig.tar.xz
      mkdir -p /opt/zig && tar xf /tmp/zig.tar.xz -C /opt/zig --strip-components=1
      echo "PATH+=/opt/zig" ;;
    nim)
      fetch "https://nim-lang.org/download/nim-${NIM_VERSION}-linux_x64.tar.xz" /tmp/nim.tar.xz
      mkdir -p /opt/nim && tar xJf /tmp/nim.tar.xz -C /opt/nim --strip-components=1
      echo "PATH+=/opt/nim/bin" ;;
    julia)
      fetch "https://julialang-s3.julialang.org/bin/linux/x64/${JULIA_SERIES}/julia-${JULIA_VERSION}-linux-x86_64.tar.gz" /tmp/julia.tgz
      mkdir -p /opt/julia && tar xzf /tmp/julia.tgz -C /opt/julia --strip-components=1
      python3 endpoints/julia/clear_execstack.py $(find /opt/julia -name '*.so*' -type f) || true
      echo "PATH+=/opt/julia/bin" ;;
    dotnet)
      fetch https://dot.net/v1/dotnet-install.sh /tmp/dotnet-install.sh
      bash /tmp/dotnet-install.sh --channel "$DOTNET_CHANNEL" --install-dir /opt/dotnet >/dev/null
      echo "PATH+=/opt/dotnet" ;;
    dotnet9)
      fetch https://dot.net/v1/dotnet-install.sh /tmp/dotnet-install.sh
      bash /tmp/dotnet-install.sh --channel 9.0 --install-dir /opt/dotnet >/dev/null
      echo "PATH+=/opt/dotnet" ;;
    kotlin)
      fetch "https://github.com/JetBrains/kotlin/releases/download/v${KOTLIN_VERSION}/kotlin-compiler-${KOTLIN_VERSION}.zip" /tmp/kotlin.zip
      mkdir -p /opt/kotlinc && unzip -q -o /tmp/kotlin.zip -d /opt/kotlinc
      echo "PATH+=/opt/kotlinc/kotlinc/bin" ;;
    gradle)
      fetch "https://services.gradle.org/distributions/gradle-${GRADLE_VERSION}-bin.zip" /tmp/gradle.zip
      rm -rf /opt/gradle && mkdir -p /opt && unzip -q -o /tmp/gradle.zip -d /opt
      mv "/opt/gradle-${GRADLE_VERSION}" /opt/gradle
      echo "PATH+=/opt/gradle/bin" ;;
    swift)
      fetch "https://download.swift.org/swift-${SWIFT_VERSION}-release/debian12/swift-${SWIFT_VERSION}-RELEASE/swift-${SWIFT_VERSION}-RELEASE-debian12.tar.gz" /tmp/swift.tgz
      mkdir -p /opt/swift && tar xzf /tmp/swift.tgz -C /opt/swift --strip-components=1
      echo "PATH+=/opt/swift/usr/bin" ;;
    grpcurl)
      fetch "https://github.com/fullstorydev/grpcurl/releases/download/v${GRPCURL_VERSION}/grpcurl_${GRPCURL_VERSION}_linux_x86_64.tar.gz" /tmp/grpcurl.tgz
      mkdir -p /opt/grpcurl && tar xzf /tmp/grpcurl.tgz -C /opt/grpcurl grpcurl
      echo "PATH+=/opt/grpcurl" ;;
    jacorb)
      fetch "https://www.jacorb.org/releases/${JACORB_VERSION}/jacorb-${JACORB_VERSION}-binary.zip" /tmp/jacorb.zip
      rm -rf /tmp/jacorb-unpack && mkdir -p /tmp/jacorb-unpack && unzip -q /tmp/jacorb.zip -d /tmp/jacorb-unpack
      rm -rf /opt/jacorb-sdk && mkdir -p /opt/jacorb-sdk/jdk8
      mv /tmp/jacorb-unpack/jacorb-"${JACORB_VERSION}" /opt/jacorb-sdk/jacorb
      build="${TEMURIN8_RELEASE#jdk}"
      fetch "https://github.com/adoptium/temurin8-binaries/releases/download/${TEMURIN8_RELEASE}/OpenJDK8U-jdk_x64_linux_hotspot_${build/-/}.tar.gz" /tmp/jdk8.tgz
      tar xzf /tmp/jdk8.tgz -C /opt/jacorb-sdk/jdk8 --strip-components=1
      cache_path jacorb ;;
    wasm)
      rustup target add wasm32-unknown-unknown >/dev/null
      python3 ci/download_tool.py \
        "https://github.com/wasm-bindgen/wasm-bindgen/releases/download/${WASM_BINDGEN_VERSION}/wasm-bindgen-${WASM_BINDGEN_VERSION}-x86_64-unknown-linux-musl.tar.gz" \
        /tmp/wasm-bindgen.tgz --sha256 "$WASM_BINDGEN_SHA256"
      rm -rf /opt/wasm-bindgen && mkdir -p /opt/wasm-bindgen
      tar xzf /tmp/wasm-bindgen.tgz -C /opt/wasm-bindgen --strip-components=1 \
        "wasm-bindgen-${WASM_BINDGEN_VERSION}-x86_64-unknown-linux-musl/wasm-bindgen"
      echo "PATH+=/opt/wasm-bindgen" ;;
    flutter)
      python3 ci/download_tool.py \
        "https://storage.googleapis.com/flutter_infra_release/releases/stable/linux/flutter_linux_${FLUTTER_VERSION}-stable.tar.xz" \
        /tmp/flutter.tar.xz --sha256 "$FLUTTER_SHA256"
      rm -rf /opt/flutter && mkdir -p /opt/flutter
      tar xJf /tmp/flutter.tar.xz -C /opt/flutter --strip-components=1 --no-same-owner
      rm -f /tmp/flutter.tar.xz
      # The tool's first run builds its snapshot and fetches the engine
      # artifacts `flutter test` needs into bin/cache; done here, before
      # the SDK goes to the host's cache, so that a job reading it writes
      # nothing there. git refuses the SDK's repository under another
      # owner unless told otherwise, for this process only.
      GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0=safe.directory GIT_CONFIG_VALUE_0='*' CI=true \
        /opt/flutter/bin/flutter --suppress-analytics precache --no-android --no-ios --no-web \
        --no-linux --no-windows --no-macos --no-fuchsia --universal >/dev/null
      echo "PATH+=/opt/flutter/bin" ;;
  esac
  to_cache "$sdk"
  installed+=("$sdk")
done

echo "provision: present: ${present[*]:-none}; installed packages: ${packages[*]:-none}; installed archives: ${installed[*]:-none}; archives from the host's cache: ${cached[*]:-none}"
case " ${archives[*]} " in *" dotnet "*|*" dotnet9 "*) echo "ENV DOTNET_CLI_TELEMETRY_OPTOUT=1"; echo "ENV DOTNET_NOLOGO=1" ;; esac
# Debian ships no unversioned `lua`: the gate and the runtime self-test take
# the interpreter from LUA.
case " $* " in *" lua "*) have lua || echo "ENV LUA=lua5.4" ;; esac
case " $* " in
  *" softhsm "*)
    [ -e "$SOFTHSM_MODULE" ] || { echo "provision: no PKCS#11 module at $SOFTHSM_MODULE" >&2; exit 1; }
    echo "ENV ZERODDS_TEST_PKCS11_MODULE=$SOFTHSM_MODULE" ;;
esac
