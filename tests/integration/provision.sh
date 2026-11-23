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
# The OpenTelemetry Collector (core distribution `otelcol`, its release
# archive's sha256): the OTLP receiver of zerodds-grpc-bridge's span test
# (zerodds-grpc-bridge-1.0 §8.3), under /opt/otelcol.
OTELCOL_VERSION="${OTELCOL_VERSION:-0.115.0}"
OTELCOL_SHA256="${OTELCOL_SHA256:-cf6268c459cfc92cc3c2bcab9a631ca1024d5e60bc727e760870f9f9d923dbb9}"
# ghz, the gRPC load tester (grpc-go), its release archive's sha256: a client
# of zerodds-grpc-bridge's cross_vendor tests (zerodds-grpc-bridge-1.0 §10),
# under /opt/ghz.
GHZ_VERSION="${GHZ_VERSION:-0.121.0}"
GHZ_SHA256="${GHZ_SHA256:-9ae3b93f2c46dac9136e29e81b4a1de8d4e56f092a6fe46884a25c9c83cb2324}"
# BloomRPC, the gRPC GUI client (Electron), its Debian package's sha256,
# unpacked under /opt/bloomrpc, with Xvfb and the libraries Electron loads:
# a client of the same tests, driven over the DevTools protocol
# (crates/grpc-bridge/tests/interop/bloomrpc_drive.py).
BLOOMRPC_VERSION="${BLOOMRPC_VERSION:-1.5.3}"
BLOOMRPC_SHA256="${BLOOMRPC_SHA256:-e04432733f1d868845992f17e5b92b2832415b709f97a6f5d305f52554547504}"
# Eclipse Californium (californium-core, element-connector, slf4j-api from
# Maven Central, their sha256 below): the Java CoAP client of
# zerodds-coap-bridge's cross_vendor tests, which find the jars under
# /opt/californium (CALIFORNIUM_CP); it runs on the default JDK.
CALIFORNIUM_VERSION="${CALIFORNIUM_VERSION:-3.14.0}"
SLF4J_VERSION="${SLF4J_VERSION:-1.7.36}"
CALIFORNIUM_CORE_SHA256="${CALIFORNIUM_CORE_SHA256:-dfd4c0b97dfe716ba285f310e6d9c8958cc8c78813d541fbcb9b9643a494a5a3}"
ELEMENT_CONNECTOR_SHA256="${ELEMENT_CONNECTOR_SHA256:-03edf864dacd068e08f844d4fab0f9467ae4a28a8532c85d5bbd66d8cb93950c}"
SLF4J_API_SHA256="${SLF4J_API_SHA256:-d3ef575e3e4979678dc01bf1dcce51021493b4d11fb7f1be8ad982877c16a1c0}"
# Eclipse Wakaama's LwM2M client (examples/client/udp), built from the tag
# below at its commit: the LwM2M stack of the same tests, under
# /opt/wakaama/lwm2mclient (WAKAAMA_LWM2MCLIENT).
WAKAAMA_TAG="${WAKAAMA_TAG:-snapshots/2026-05-26}"
WAKAAMA_COMMIT="${WAKAAMA_COMMIT:-94ff56f77a2d24a5890e0e703809a47633aa7d4b}"
# JacORB and the Java 8 JDK it runs on (Temurin): the foreign ORB of the
# zerodds-corba-dds-bridge and zerodds-corba-interop tests, which find them
# under /opt/jacorb and /opt/jdk8 (JACORB_HOME, JACORB_JAVA_HOME).
JACORB_VERSION="${JACORB_VERSION:-3.9}"
TEMURIN8_RELEASE="${TEMURIN8_RELEASE:-jdk8u492-b09}"
# Apache ActiveMQ Artemis, its binary release archive and the archive's
# sha512 from archive.apache.org: the transactional resource of
# zerodds-amqp-endpoint's artemis_txn_e2e tests (OASIS AMQP 1.0 Part 4),
# which find it under /opt/artemis (ARTEMIS_HOME); it runs on the default
# JDK (Java 17 or later).
ARTEMIS_VERSION="${ARTEMIS_VERSION:-2.44.0}"
ARTEMIS_SHA512="${ARTEMIS_SHA512:-d709eafe1ead38234216d324e9876ee0f66265a2deba36b526f64512de4da1e25ae6b5430a03dc002071ed91553c72f3ef01211d7414b14752272cbec66883b4}"
# Apache Qpid Broker-J, its binary release archive and the archive's sha512
# from archive.apache.org: the broker with an OAuth2 provider (SASL XOAUTH2)
# of zerodds-amqp-endpoint's bridge_brokerj_e2e test, which finds it under
# /opt/qpid-broker-j (QPID_HOME); it runs on the default JDK (Java 17 or
# later).
BROKERJ_VERSION="${BROKERJ_VERSION:-9.2.1}"
BROKERJ_SHA512="${BROKERJ_SHA512:-48a1633e2195b77c8d458d92ef5cc5d5e0a45ea93b5964185007570ea51cb2a58df764189a505f64d810a14cd254acfbb82b8ed7e2597e9f50bb708474802e7f}"
# Apache Qpid Dispatch Router, its source release (the last, 1.19.0) and
# the archive's sha512 from archive.apache.org: the router of
# zerodds-amqp-endpoint's bridge_dispatch_e2e tests, which find it under
# /opt/qpid-dispatch (QPID_DISPATCH_HOME). It embeds Python, and 1.19.0
# ends with SIGSEGV on Python 3.13 (PyUnicode_FromString before
# Py_Initialize in qd_python_initialize, measured on Debian trixie), so it
# is built against the CPython 3.11 of python-build-standalone below, which
# goes to /opt/qpid-dispatch/python (the tests run the router with
# PYTHONHOME there). GCC 14 refuses the implicit declaration of
# gettimeofday in src/dispatch.c: the build includes sys/time.h.
DISPATCH_VERSION="${DISPATCH_VERSION:-1.19.0}"
DISPATCH_SHA512="${DISPATCH_SHA512:-f6bfc9f14c1badb59f0934f3ea0832416d112b8f2081c539902807e980edafc9cf962c6b0fff07624c12d8d2dc1b67b9770bd7028c3013ac1a240a0a07825b65}"
DISPATCH_PYTHON_URL="${DISPATCH_PYTHON_URL:-https://github.com/astral-sh/python-build-standalone/releases/download/20241016/cpython-3.11.10+20241016-x86_64-unknown-linux-gnu-install_only.tar.gz}"
DISPATCH_PYTHON_SHA256="${DISPATCH_PYTHON_SHA256:-8b50a442b04724a24c1eebb65a36a0c0e833d35374dbdf9c9470d8a97b164cd9}"
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
# The shape_main of RTI Connext DDS that the OMG dds-rtps suite releases
# (tests/interop/omg/vendors.tsv, the same release asset and sha256): the
# Connext participant of zerodds-websocket-bridge's cross_vendor tests, which
# find it under /opt/omg-connext (CONNEXT_SHAPE_MAIN).
CONNEXT_SHAPE_MAIN_RELEASE="${CONNEXT_SHAPE_MAIN_RELEASE:-v2.1.2026}"
CONNEXT_SHAPE_MAIN_ASSET="${CONNEXT_SHAPE_MAIN_ASSET:-connext_dds-7.7.0_shape_main_linux}"
CONNEXT_SHAPE_MAIN_SHA256="${CONNEXT_SHAPE_MAIN_SHA256:-98e19c5cb38e77becf606f738b1de4b6bc2d1a12600b973cffd223d14ce8132c}"

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
    # Cyclone DDS's C library, headers and idlc, and a C compiler: the
    # foreign DDS participant of zerodds-coap-bridge's cross_vendor tests
    # (tests/interop/cyclone_reading.c, built by the test).
    cyclonedds-dev) [ -e /usr/include/dds/dds.h ] && have idlc && have cc && present+=(cyclonedds-dev) \
        || want_packages cyclonedds-dev cyclonedds-tools gcc libc6-dev ;;
    # Fast DDS 3 (Debian's libfastdds-dev, 3.1 in trixie) and a C++ compiler:
    # the foreign DDS participant of zerodds-websocket-bridge's cross_vendor
    # tests (tests/interop/fastdds_reading.cpp, built by the test).
    fastdds-dev) [ -e /usr/include/fastdds/dds/domain/DomainParticipant.hpp ] && have c++ \
        && present+=(fastdds-dev) || want_packages libfastdds-dev g++ ;;
    # aiocoap (Python): a CoAP client of the same tests.
    aiocoap) py_has aiocoap && present+=(aiocoap) || want_packages python3-aiocoap ;;
    mosquitto)
      if have mosquitto || [ -x /usr/sbin/mosquitto ]; then
        have mosquitto_sub && present+=(mosquitto) || want_packages mosquitto-clients
      else
        want_packages mosquitto mosquitto-clients
      fi ;;
    libcoap) have coap-client-notls || have coap-client-openssl && present+=(libcoap) || want_packages libcoap3-bin ;;
    websockets) py_has websockets.sync.client && present+=(websockets) || want_packages python3-websockets ;;
    # Eclipse Paho's Python MQTT client (transport websockets).
    paho) py_has paho.mqtt.client && present+=(paho) || want_packages python3-paho-mqtt ;;
    grpcio) py_has grpc && present+=(grpcio) || want_packages python3-grpcio ;;
    # protoc: the stubs of the gRPC clients of zerodds-grpc-bridge's
    # cross_vendor tests (crates/grpc-bridge/tests/interop/build_peers.sh).
    protoc) have protoc && present+=(protoc) || want_packages protobuf-compiler ;;
    # grpc-go from Debian's sources (built in GOPATH mode, nothing
    # downloaded) with protoc-gen-go and protoc-gen-go-grpc: the grpc-go
    # client of the same tests.
    grpc-go) have go && have protoc-gen-go && have protoc-gen-go-grpc \
        && [ -d /usr/share/gocode/src/google.golang.org/grpc ] && present+=(grpc-go) \
        || want_packages golang-go golang-google-grpc-dev golang-google-protobuf-dev \
             protoc-gen-go protoc-gen-go-grpc ;;
    # grpc-java (Debian's libgrpc-java with the Netty transport), protoc's
    # grpc-java plugin and the javax.annotation.Generated its stubs carry:
    # the grpc-java client of the same tests.
    grpc-java) [ -f /usr/share/java/grpc-netty.jar ] && have grpc_java_plugin && have javac \
        && [ -f /usr/share/java/geronimo-annotation-1.3-spec.jar ] && present+=(grpc-java) \
        || want_packages libgrpc-java protobuf-compiler-grpc-java-plugin \
             libgeronimo-annotation-1.3-spec-java default-jdk-headless ;;
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
    zig|nim|julia|dotnet|dotnet9|kotlin|swift|grpcurl|otelcol|ghz|bloomrpc|jacorb|gradle|wasm|flutter|californium|wakaama|connext-shape-main|artemis|brokerj|dispatch) archives+=("$tool") ;;
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
    otelcol) on_path_or_opt otelcol /opt/otelcol ;;
    ghz) on_path_or_opt ghz /opt/ghz ;;
    bloomrpc) [ -x /opt/bloomrpc/bloomrpc ] && have xvfb-run && have xauth \
        && py_has websockets.sync.client && ! ldd /opt/bloomrpc/bloomrpc | grep -q 'not found' ;;
    jacorb) [ -f /opt/jacorb/lib/idl.jar ] && [ -x /opt/jdk8/bin/javac ] ;;
    gradle) on_path_or_opt gradle /opt/gradle/bin ;;
    # Rust's wasm32-unknown-unknown and the wasm-bindgen CLI of the pin.
    wasm) rustup target list --installed 2>/dev/null | grep -qx wasm32-unknown-unknown \
        && on_path_or_opt wasm-bindgen /opt/wasm-bindgen \
        && [ "$(wasm-bindgen --version | awk '{print $2}')" = "$WASM_BINDGEN_VERSION" ] ;;
    flutter) on_path_or_opt flutter /opt/flutter/bin ;;
    californium) [ -f "/opt/californium/californium-core-${CALIFORNIUM_VERSION}.jar" ] && have java ;;
    wakaama) [ -x /opt/wakaama/lwm2mclient ] ;;
    connext-shape-main) [ -x "/opt/omg-connext/$CONNEXT_SHAPE_MAIN_ASSET" ] ;;
    artemis) [ -x /opt/artemis/bin/artemis ] && have java ;;
    brokerj) [ -x /opt/qpid-broker-j/bin/qpid-server ] && have java ;;
    # The router and the Proton libraries it links (Debian's).
    dispatch) [ -x /opt/qpid-dispatch/sbin/qdrouterd ] \
        && ! ldd /opt/qpid-dispatch/sbin/qdrouterd | grep -q 'not found' ;;
  esac && { present+=("$sdk"); continue; }
  missing+=("$sdk")
  case "$sdk" in
    kotlin) have javac || want_packages default-jdk-headless; have unzip || want_packages unzip ;;
    jacorb) have unzip || want_packages unzip ;;
    bloomrpc) want_packages xvfb xauth python3-websockets libgtk-3-0t64 libxss1 libxtst6 libcups2t64 \
        libnotify4 libsecret-1-0 libatk-bridge2.0-0t64 libnss3 libasound2t64 libgbm1 ;;
    connext-shape-main) have unzip || want_packages unzip ;;
    californium) have java || want_packages default-jdk-headless ;;
    artemis) have java || want_packages default-jdk-headless ;;
    brokerj) have java || want_packages default-jdk-headless ;;
    dispatch) want_packages libqpid-proton11-dev; have cmake || want_packages cmake
      have cc || want_packages gcc libc6-dev; have make || want_packages make ;;
    wakaama) have git || want_packages git; have cmake || want_packages cmake
      have cc || want_packages gcc libc6-dev; have make || want_packages make ;;
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
    otelcol) echo "otelcol-$OTELCOL_VERSION" ;;
    ghz) echo "ghz-$GHZ_VERSION" ;;
    bloomrpc) echo "bloomrpc-$BLOOMRPC_VERSION" ;;
    flutter) echo "flutter-$FLUTTER_VERSION" ;;
    jacorb) echo "jacorb-$JACORB_VERSION-$TEMURIN8_RELEASE" ;;
    californium) echo "californium-$CALIFORNIUM_VERSION" ;;
    wakaama) echo "wakaama-$WAKAAMA_COMMIT" ;;
    connext-shape-main) echo "connext-shape-main-$CONNEXT_SHAPE_MAIN_SHA256" ;;
    artemis) echo "artemis-$ARTEMIS_VERSION" ;;
    brokerj) echo "brokerj-$BROKERJ_VERSION" ;;
    dispatch) echo "dispatch-$DISPATCH_VERSION-py311" ;;
  esac
}
cache_dir_of() {
  case "$1" in
    zig) echo /opt/zig ;; nim) echo /opt/nim ;; julia) echo /opt/julia ;;
    dotnet|dotnet9) echo /opt/dotnet ;; kotlin) echo /opt/kotlinc ;; gradle) echo /opt/gradle ;;
    swift) echo /opt/swift ;; grpcurl) echo /opt/grpcurl ;; flutter) echo /opt/flutter ;;
    otelcol) echo /opt/otelcol ;; ghz) echo /opt/ghz ;; bloomrpc) echo /opt/bloomrpc ;;
    jacorb) echo /opt/jacorb-sdk ;;
    californium) echo /opt/californium ;; wakaama) echo /opt/wakaama ;;
    connext-shape-main) echo /opt/omg-connext ;;
    artemis) echo /opt/artemis ;;
    brokerj) echo /opt/qpid-broker-j ;;
    dispatch) echo /opt/qpid-dispatch ;;
  esac
}
cache_path() {
  case "$1" in
    zig) echo "PATH+=/opt/zig" ;; nim) echo "PATH+=/opt/nim/bin" ;; julia) echo "PATH+=/opt/julia/bin" ;;
    dotnet|dotnet9) echo "PATH+=/opt/dotnet" ;; kotlin) echo "PATH+=/opt/kotlinc/kotlinc/bin" ;;
    gradle) echo "PATH+=/opt/gradle/bin" ;; swift) echo "PATH+=/opt/swift/usr/bin" ;;
    grpcurl) echo "PATH+=/opt/grpcurl" ;; flutter) echo "PATH+=/opt/flutter/bin" ;;
    otelcol) echo "PATH+=/opt/otelcol" ;; ghz) echo "PATH+=/opt/ghz" ;;
    # JacORB and its JDK go into the cache as one directory; the tests find
    # them under /opt/jacorb and /opt/jdk8 (JACORB_HOME, JACORB_JAVA_HOME).
    jacorb) rm -rf /opt/jacorb /opt/jdk8
      ln -s /opt/jacorb-sdk/jacorb /opt/jacorb && ln -s /opt/jacorb-sdk/jdk8 /opt/jdk8 ;;
    californium|wakaama|bloomrpc|connext-shape-main|artemis|brokerj|dispatch) : ;;
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
    otelcol)
      python3 ci/download_tool.py \
        "https://github.com/open-telemetry/opentelemetry-collector-releases/releases/download/v${OTELCOL_VERSION}/otelcol_${OTELCOL_VERSION}_linux_amd64.tar.gz" \
        /tmp/otelcol.tgz --sha256 "$OTELCOL_SHA256"
      rm -rf /opt/otelcol && mkdir -p /opt/otelcol && tar xzf /tmp/otelcol.tgz -C /opt/otelcol otelcol
      echo "PATH+=/opt/otelcol" ;;
    ghz)
      python3 ci/download_tool.py \
        "https://github.com/bojand/ghz/releases/download/v${GHZ_VERSION}/ghz-linux-x86_64.tar.gz" \
        /tmp/ghz.tgz --sha256 "$GHZ_SHA256"
      rm -rf /opt/ghz && mkdir -p /opt/ghz && tar xzf /tmp/ghz.tgz -C /opt/ghz ghz
      echo "PATH+=/opt/ghz" ;;
    bloomrpc)
      python3 ci/download_tool.py \
        "https://github.com/bloomrpc/bloomrpc/releases/download/${BLOOMRPC_VERSION}/bloomrpc_${BLOOMRPC_VERSION}_amd64.deb" \
        /tmp/bloomrpc.deb --sha256 "$BLOOMRPC_SHA256"
      rm -rf /tmp/bloomrpc-unpack /opt/bloomrpc && dpkg-deb -x /tmp/bloomrpc.deb /tmp/bloomrpc-unpack
      mv /tmp/bloomrpc-unpack/opt/BloomRPC /opt/bloomrpc && rm -rf /tmp/bloomrpc-unpack /tmp/bloomrpc.deb ;;
    jacorb)
      fetch "https://www.jacorb.org/releases/${JACORB_VERSION}/jacorb-${JACORB_VERSION}-binary.zip" /tmp/jacorb.zip
      rm -rf /tmp/jacorb-unpack && mkdir -p /tmp/jacorb-unpack && unzip -q /tmp/jacorb.zip -d /tmp/jacorb-unpack
      rm -rf /opt/jacorb-sdk && mkdir -p /opt/jacorb-sdk/jdk8
      mv /tmp/jacorb-unpack/jacorb-"${JACORB_VERSION}" /opt/jacorb-sdk/jacorb
      build="${TEMURIN8_RELEASE#jdk}"
      fetch "https://github.com/adoptium/temurin8-binaries/releases/download/${TEMURIN8_RELEASE}/OpenJDK8U-jdk_x64_linux_hotspot_${build/-/}.tar.gz" /tmp/jdk8.tgz
      tar xzf /tmp/jdk8.tgz -C /opt/jacorb-sdk/jdk8 --strip-components=1
      cache_path jacorb ;;
    californium)
      maven=https://repo1.maven.org/maven2
      mkdir -p /opt/californium
      for jar in "org/eclipse/californium/californium-core/${CALIFORNIUM_VERSION}/californium-core-${CALIFORNIUM_VERSION}.jar $CALIFORNIUM_CORE_SHA256" \
                 "org/eclipse/californium/element-connector/${CALIFORNIUM_VERSION}/element-connector-${CALIFORNIUM_VERSION}.jar $ELEMENT_CONNECTOR_SHA256" \
                 "org/slf4j/slf4j-api/${SLF4J_VERSION}/slf4j-api-${SLF4J_VERSION}.jar $SLF4J_API_SHA256"; do
        set -- $jar
        python3 ci/download_tool.py "$maven/$1" "/opt/californium/${1##*/}" --sha256 "$2"
      done ;;
    artemis)
      fetch "https://archive.apache.org/dist/activemq/activemq-artemis/${ARTEMIS_VERSION}/apache-artemis-${ARTEMIS_VERSION}-bin.tar.gz" /tmp/artemis.tgz
      echo "$ARTEMIS_SHA512  /tmp/artemis.tgz" | sha512sum -c --quiet - \
        || { echo "provision: artemis $ARTEMIS_VERSION: the archive's sha512 differs" >&2; exit 1; }
      rm -rf /opt/artemis && mkdir -p /opt/artemis
      tar xzf /tmp/artemis.tgz -C /opt/artemis --strip-components=1 ;;
    brokerj)
      fetch "https://archive.apache.org/dist/qpid/broker-j/${BROKERJ_VERSION}/binaries/apache-qpid-broker-j-${BROKERJ_VERSION}-bin.tar.gz" /tmp/brokerj.tgz
      echo "$BROKERJ_SHA512  /tmp/brokerj.tgz" | sha512sum -c --quiet - \
        || { echo "provision: brokerj $BROKERJ_VERSION: the archive's sha512 differs" >&2; exit 1; }
      # The archive's root is qpid-broker/<version>/.
      rm -rf /opt/qpid-broker-j && mkdir -p /opt/qpid-broker-j
      tar xzf /tmp/brokerj.tgz -C /opt/qpid-broker-j --strip-components=2 ;;
    dispatch)
      python3 ci/download_tool.py "$DISPATCH_PYTHON_URL" /tmp/dispatch-python.tgz \
        --sha256 "$DISPATCH_PYTHON_SHA256"
      fetch "https://archive.apache.org/dist/qpid/dispatch/${DISPATCH_VERSION}/qpid-dispatch-${DISPATCH_VERSION}.tar.gz" /tmp/dispatch.tgz
      echo "$DISPATCH_SHA512  /tmp/dispatch.tgz" | sha512sum -c --quiet - \
        || { echo "provision: dispatch $DISPATCH_VERSION: the archive's sha512 differs" >&2; exit 1; }
      rm -rf /opt/qpid-dispatch /tmp/dispatch && mkdir -p /opt/qpid-dispatch/python /tmp/dispatch
      tar xzf /tmp/dispatch-python.tgz -C /opt/qpid-dispatch/python --strip-components=1
      tar xzf /tmp/dispatch.tgz -C /tmp/dispatch --strip-components=1
      py=/opt/qpid-dispatch/python
      cmake -S /tmp/dispatch -B /tmp/dispatch/build -DCMAKE_BUILD_TYPE=Release \
        -DCMAKE_INSTALL_PREFIX=/opt/qpid-dispatch -DCMAKE_INSTALL_RPATH="$py/lib" \
        -DPython_EXECUTABLE="$py/bin/python3" -DPython_ROOT_DIR="$py" -DPython_FIND_STRATEGY=LOCATION \
        -DUSE_LIBWEBSOCKETS=OFF -DBUILD_TESTING=OFF -DBUILD_DOCS=OFF \
        "-DCMAKE_C_FLAGS=-include sys/time.h -Wno-error" >/dev/null
      cmake --build /tmp/dispatch/build -j"$(nproc)" >/dev/null
      cmake --install /tmp/dispatch/build >/dev/null 2>&1
      # The packages the router imports beside qpid_dispatch_internal: the
      # install puts qpid_dispatch through setup.py, which does not reach a
      # directory on the router's path.
      cp -r /tmp/dispatch/python/qpid_dispatch /tmp/dispatch/build/python/qpid_dispatch_site.py \
        /opt/qpid-dispatch/lib/qpid-dispatch/python/ ;;
    wakaama)
      rm -rf /tmp/wakaama && git -c advice.detachedHead=false clone -q --depth 1 --branch "$WAKAAMA_TAG" \
        https://github.com/eclipse-wakaama/wakaama.git /tmp/wakaama
      [ "$(git -C /tmp/wakaama rev-parse HEAD)" = "$WAKAAMA_COMMIT" ] \
        || { echo "provision: wakaama $WAKAAMA_TAG is not $WAKAAMA_COMMIT" >&2; exit 1; }
      # wakaama.cmake looks for the tinydtls submodule also for the UDP
      # client; the unit tests need CUnit, which the client does not.
      git -C /tmp/wakaama submodule update --init --depth 1 -q
      cmake -S /tmp/wakaama/examples/client/udp -B /tmp/wakaama/build -DWAKAAMA_UNIT_TESTS=OFF >/dev/null
      cmake --build /tmp/wakaama/build -j"$(nproc)" --target lwm2mclient >/dev/null
      mkdir -p /opt/wakaama && cp /tmp/wakaama/build/lwm2mclient /opt/wakaama/ ;;
    connext-shape-main)
      python3 ci/download_tool.py \
        "https://github.com/omg-dds/dds-rtps/releases/download/${CONNEXT_SHAPE_MAIN_RELEASE}/${CONNEXT_SHAPE_MAIN_ASSET}.zip" \
        /tmp/connext-shape-main.zip --sha256 "$CONNEXT_SHAPE_MAIN_SHA256"
      rm -rf /opt/omg-connext && mkdir -p /opt/omg-connext
      unzip -q -o /tmp/connext-shape-main.zip -d /opt/omg-connext
      chmod +x "/opt/omg-connext/$CONNEXT_SHAPE_MAIN_ASSET"
      rm -f /tmp/connext-shape-main.zip ;;
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
