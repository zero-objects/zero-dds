# Yocto recipe for ZeroDDS — WP 5.E.4
#
# Inherits cargo_bin.bbclass from meta-rust; builds the ZeroDDS workspace
# for the Yocto target. Default: only the libzerodds bundle (cdylib +
# zerodds.h). Options via PACKAGECONFIG.

SUMMARY = "ZeroDDS — pure-Rust DDS implementation (RTPS 2.5, XTypes, Security)"
DESCRIPTION = "ZeroDDS provides a complete DDS/RTPS stack \
in Rust: SPDP+SEDP discovery, Reliable+Best-Effort QoS, XTypes 1.3, \
DDS-Security 1.2, plus bridges to CoAP/MQTT/WebSocket/gRPC."
HOMEPAGE = "https://zerodds.io/"
LICENSE = "Apache-2.0"
LIC_FILES_CHKSUM = "file://LICENSE;md5=PLACEHOLDER"

SECTION = "libs"

inherit cargo_bin

# Source: a local tarball or a git checkout. Git here as the
# default for CI reproducibility.
SRC_URI = "git://github.com/zero-objects/zero-dds.git;protocol=https;branch=main"
SRCREV = "${AUTOREV}"

S = "${WORKDIR}/git"

# Cargo build with the same configuration as target/release.
CARGO_BUILD_FLAGS = "-p dds-c-api --release"

# Set the cross-compile target — meta-rust maps it automatically
# for aarch64/armv7/musl-libc.

# PACKAGECONFIG knobs, as in Buildroot:
#   rtps-only    slim build without security/bridges/CCM/CORBA
#   security     with DDS-Security 1.2
#   bridges      with CoAP/MQTT/WebSocket/gRPC/AMQP
#   tools        with dds-replay/dds-chaos/dds-dashboard
PACKAGECONFIG ??= "security"
PACKAGECONFIG[rtps-only] = ",--no-default-features"
PACKAGECONFIG[security] = "--features security,,"
PACKAGECONFIG[bridges] = ",,coap-bridge mqtt-bridge websocket-bridge"
PACKAGECONFIG[tools] = ",,dds-replay dds-chaos"

# Build phase through cargo_bin's do_compile.
do_compile:prepend() {
    bbnote "ZeroDDS Cargo: ${CARGO_BUILD_FLAGS}"
}

# Install: install libzerodds.so + zerodds.h into the target/SDK.
do_install() {
    install -d ${D}${libdir}
    install -m 0755 ${B}/target/${RUST_TARGET}/release/libzerodds.so \
        ${D}${libdir}/libzerodds.so.${PV}
    cd ${D}${libdir} && ln -s libzerodds.so.${PV} libzerodds.so

    install -d ${D}${includedir}/zerodds
    install -m 0644 ${S}/crates/dds-c-api/include/zerodds.h \
        ${D}${includedir}/zerodds/zerodds.h
}

# If the `tools` knob is active: install the tool binaries as well.
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

# Enable the rust targets deliberately.
COMPATIBLE_HOST = "(aarch64|x86_64|arm).*-linux"
