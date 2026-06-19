%global crate_name        zerodds
%global crate_version     0.0.0
%global rust_version_min  1.85

Name:           zerodds
Version:        %{crate_version}
Release:        1%{?dist}
Summary:        Pure-Rust DDS implementation (OMG DDS 1.4 + RTPS 2.5)

License:        Apache-2.0
URL:            https://zerodds.io
Source0:        %{name}-%{version}.tar.gz

BuildRequires:  cargo >= %{rust_version_min}
BuildRequires:  rust  >= %{rust_version_min}
BuildRequires:  pkgconfig
BuildRequires:  openssl-devel

%description
ZeroDDS is a pure-Rust DDS (Data Distribution Service) implementation
conformant to OMG DDS 1.4, DDSI-RTPS 2.5, DDS-Security 1.2, and
DDS-XTypes 1.3.

This is the umbrella package; install one of the sub-packages:

  * zerodds-tools          — CLI utilities (admin / perf / idlc / xmlc / replay)
  * zerodds-devel          — C/C++ headers + import-libs
  * zerodds-libs           — runtime shared library
  * zerodds-rmw            — ROS-2 RMW plugin

%package tools
Summary:        ZeroDDS command-line utilities
Requires:       %{name}-libs%{?_isa} = %{version}-%{release}

%description tools
User-facing CLI tools: dds-admin, dds-perf, dds-idlc, dds-xmlc,
dds-traceability, dds-chaos, dds-replay, roundtrip-1us.

%package libs
Summary:        ZeroDDS runtime shared library

%description libs
libzerodds.so — runtime FFI library required by C/C++ applications and
ROS-2 RMW shim consumers.

%package devel
Summary:        ZeroDDS C-FFI development headers
Requires:       %{name}-libs%{?_isa} = %{version}-%{release}

%description devel
zerodds.h header + import-library for linking native applications
against the ZeroDDS C-FFI.

%package rmw
Summary:        ROS-2 RMW implementation backed by ZeroDDS
Requires:       %{name}-libs%{?_isa} = %{version}-%{release}

%description rmw
ROS-2 plugin (ament-cmake-package) implementing the RMW (ROS Middleware)
interface. Supports ROS-2 Humble, Iron, Jazzy.

%prep
%autosetup -n %{crate_name}-%{version}

%build
RUSTFLAGS="-C link-arg=-Wl,--as-needed" \
cargo build --release \
  -p dds-admin -p dds-perf -p dds-idlc -p dds-xmlc \
  -p dds-traceability -p dds-chaos -p amqp-dds-endpoint \
  -p isolation-smoke -p dds-bench-suite \
  -p dds-c-api -p rmw-zerodds-shim

%install
# Tools-Pakete
install -d %{buildroot}%{_bindir}
for b in dds-admin dds-perf dds-idlc dds-xmlc dds-traceability \
         dds-chaos amqp-dds-endpoint isolation-smoke roundtrip-1us; do
  install -m 0755 target/release/$b %{buildroot}%{_bindir}/
done

# Libs
install -d %{buildroot}%{_libdir}
install -m 0755 target/release/libzerodds.so \
  %{buildroot}%{_libdir}/libzerodds.so.0
ln -s libzerodds.so.0 %{buildroot}%{_libdir}/libzerodds.so

install -m 0755 target/release/librmw_zerodds.so \
  %{buildroot}%{_libdir}/

# Devel: Header
install -d %{buildroot}%{_includedir}
install -m 0644 crates/dds-c-api/include/zerodds.h \
  %{buildroot}%{_includedir}/

%check
# Doc-Tests + Lib-Tests; Integration/E2E brauchen UDP-Stack im Builder.
cargo test --release --workspace --exclude dds-chaos --lib \
  -- --skip integration --skip e2e || :

%files tools
%{_bindir}/dds-admin
%{_bindir}/dds-perf
%{_bindir}/dds-idlc
%{_bindir}/dds-xmlc
%{_bindir}/dds-traceability
%{_bindir}/dds-chaos
%{_bindir}/amqp-dds-endpoint
%{_bindir}/isolation-smoke
%{_bindir}/roundtrip-1us

%files libs
%{_libdir}/libzerodds.so.0

%files devel
%{_includedir}/zerodds.h
%{_libdir}/libzerodds.so

%files rmw
%{_libdir}/librmw_zerodds.so

%changelog
* Sat May 03 2026 ZeroDDS Maintainers <maintainers@zerodds.io> - 0.0.0-1
- Initial RPM spec (Phase-5 E.2).
