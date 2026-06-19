# Yocto-Recipe fuer ZeroDDS — WP 5.E.4
#
# Erbt cargo_bin.bbclass aus meta-rust; baut den ZeroDDS-Workspace
# fuer das Yocto-Target. Default: nur das libzerodds-Bundle (cdylib +
# zerodds.h). Optionen via PACKAGECONFIG.

SUMMARY = "ZeroDDS — pure-Rust DDS implementation (RTPS 2.5, XTypes, Security)"
DESCRIPTION = "ZeroDDS bietet einen vollstaendigen DDS/RTPS-Stack \
in Rust: SPDP+SEDP-Discovery, Reliable+Best-Effort QoS, XTypes 1.3, \
DDS-Security 1.2, plus Bridges nach CoAP/MQTT/WebSocket/gRPC."
HOMEPAGE = "https://zerodds.io/"
LICENSE = "Apache-2.0"
LIC_FILES_CHKSUM = "file://LICENSE;md5=PLACEHOLDER"

SECTION = "libs"

inherit cargo_bin

# Source: lokal liegender Tarball oder git-Auspack. Hier git als
# Default fuer CI-Reproducability.
SRC_URI = "git://gitlab.sandra-kessler.eu/zerodds/zerodds.git;protocol=https;branch=main"
SRCREV = "${AUTOREV}"

S = "${WORKDIR}/git"

# Cargo-Build mit der gleichen Konfig wie target/release.
CARGO_BUILD_FLAGS = "-p dds-c-api --release"

# Cross-Compile-Target setzen — meta-rust mappt das automatisch
# fuer aarch64/armv7/musl-libc.

# PACKAGECONFIG-Knobs analog Buildroot:
#   rtps-only    Slim-Build ohne Security/Bridges/CCM/CORBA
#   security     mit DDS-Security 1.2
#   bridges      mit CoAP/MQTT/WebSocket/gRPC/AMQP
#   tools        mit dds-replay/dds-chaos/dds-dashboard
PACKAGECONFIG ??= "security"
PACKAGECONFIG[rtps-only] = ",--no-default-features"
PACKAGECONFIG[security] = "--features security,,"
PACKAGECONFIG[bridges] = ",,coap-bridge mqtt-bridge websocket-bridge"
PACKAGECONFIG[tools] = ",,dds-replay dds-chaos"

# Build-Phase ueber cargo_bin's do_compile.
do_compile:prepend() {
    bbnote "ZeroDDS Cargo: ${CARGO_BUILD_FLAGS}"
}

# Install: libzerodds.so + zerodds.h ins target/SDK installieren.
do_install() {
    install -d ${D}${libdir}
    install -m 0755 ${B}/target/${RUST_TARGET}/release/libzerodds.so \
        ${D}${libdir}/libzerodds.so.${PV}
    cd ${D}${libdir} && ln -s libzerodds.so.${PV} libzerodds.so

    install -d ${D}${includedir}/zerodds
    install -m 0644 ${S}/crates/dds-c-api/include/zerodds.h \
        ${D}${includedir}/zerodds/zerodds.h
}

# Wenn `tools`-Knob aktiv: Tool-Binaries mit installieren.
do_install:append() {
    if [ "${@bb.utils.contains('PACKAGECONFIG', 'tools', 'yes', 'no', d)}" = "yes" ]; then
        install -d ${D}${bindir}
        for tool in dds-replay dds-chaos dds-dashboard cargo-dag interop-matrix roundtrip-1us; do
            if [ -f ${B}/target/${RUST_TARGET}/release/${tool} ]; then
                install -m 0755 ${B}/target/${RUST_TARGET}/release/${tool} \
                    ${D}${bindir}/${tool}
            fi
        done
    fi
}

# Package-Splitting: libzerodds + libzerodds-dev + libzerodds-tools.
PACKAGES = "${PN} ${PN}-dev ${PN}-tools ${PN}-dbg"
FILES:${PN} = "${libdir}/libzerodds.so.*"
FILES:${PN}-dev = "${libdir}/libzerodds.so ${includedir}/zerodds"
FILES:${PN}-tools = "${bindir}/*"

# rust-Targets bewusst aktivieren.
COMPATIBLE_HOST = "(aarch64|x86_64|arm).*-linux"
