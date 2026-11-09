#!/usr/bin/env python3
"""The PKI one `dds-security` job needs, made from the suite's own TESTONLY CAs.

The suite ships its identity and permissions CAs with their private keys
(`identity_ca_files/`, `permissions_ca_files/`), the vendors' permissions
documents for domain 0 and a governance document for domain 0. A job here runs
on a leased domain, so every signed document it uses is signed again for that
domain, and ZeroDDS gets an identity and permissions of its own, from the
suite's `zerodds_certs/` where the checkout has it (the pull request's), else
made here:

- identity: a new key, a request with ZeroDDS' subject, signed by the
  identity CA with `openssl ca` (a v1 certificate, as the suite's own are,
  hence `-acceptX509v1IdentityCerts` for ZeroDDS' client);
- permissions: the RTI documents of the suite with ZeroDDS' subject and the
  domain replaced, signed by the permissions CA with `openssl smime`;
- the governance and each vendor's permissions: the suite's documents with the
  domain replaced, signed again.

Everything lands in `<work>`, nothing in the suite checkout changes. Returns
the two profiles `tools/omg-security-main/run_matrix.py` reads.
"""
import json
import re
import shutil
import subprocess
from pathlib import Path

PERMISSIONS = ("Join", "Read", "Write", "ReadWrite", "AllowAll")
SUBJECT = "/C=DE/ST=HE/O=ZeroDDS/CN=CN_TEST_DDS-SECURITY_ZERODDS"
OPENSSL_CNF = """[ req ]
prompt = no
distinguished_name = dn
[ dn ]
countryName = DE
stateOrProvinceName = HE
organizationName = ZeroDDS
commonName = CN_TEST_DDS-SECURITY_ZERODDS
"""
# The vendors of the suite's release, by the start of the executable's name:
# the directory of their documents and the stem of the file names.
VENDOR_DOCUMENTS = {
    "rti_connext_dds": ("rti_connext_dds_certs", "TESTONLY_rti_connext_dds_permissions"),
    "toc_coredx_dds": ("toc_coredx_dds_certs", "TESTONLY_toc_coredx_dds_permissions"),
}


def run(args, cwd):
    subprocess.run(args, cwd=cwd, check=True, capture_output=True)


def with_domain(xml, domain):
    return re.sub(r"<id>\s*0\s*</id>", f"<id>{domain}</id>", xml)


def sign(ca, source_text, target):
    target.with_suffix(".xml").write_text(source_text)
    run(["openssl", "smime", "-sign", "-in", str(target.with_suffix(".xml")), "-text",
         "-out", str(target), "-signer", "TESTONLY_permissions_ca_cert.pem",
         "-inkey", "permissions_ca_files/private/TESTONLY_permissions_ca_private_key.pem"], ca)


def setup(suite, work, domain, ours, vendor):
    """Profiles (ours, vendor's) for `domain`; `ours` and `vendor` are the
    two executables."""
    suite = Path(suite).resolve()
    work = Path(work).resolve()
    if work.exists():
        shutil.rmtree(work)
    ca = work / "ca"
    ca.mkdir(parents=True)
    # `openssl ca` keeps its database in identity_ca_files/: a copy.
    shutil.copytree(suite / "identity_ca_files", ca / "identity_ca_files")
    shutil.copytree(suite / "permissions_ca_files", ca / "permissions_ca_files")
    for name in ("TESTONLY_identity_ca_cert.pem", "TESTONLY_permissions_ca_cert.pem"):
        shutil.copy(suite / name, ca / name)
    # A suite checkout with ZeroDDS's own directory (the state of the pull
    # request, `make_submission.py`) gives the identity and the permissions
    # documents; a checkout without it gets a new identity here.
    submitted = suite / "zerodds_certs"
    if submitted.is_dir():
        shutil.copy(submitted / "TESTONLY_zerodds_identity_cert.pem", ca / "zerodds_cert.pem")
        run(["openssl", "pkcs8", "-topk8", "-nocrypt", "-in",
             str(submitted / "private/TESTONLY_zerodds_identity_private_key.pem"),
             "-out", "zerodds_key_pkcs8.pem"], ca)
    else:
        (ca / "zerodds.cnf").write_text(OPENSSL_CNF)
        run(["openssl", "genrsa", "-out", "zerodds_key.pem", "2048"], ca)
        run(["openssl", "pkcs8", "-topk8", "-nocrypt", "-in", "zerodds_key.pem",
             "-out", "zerodds_key_pkcs8.pem"], ca)
        run(["openssl", "req", "-config", "zerodds.cnf", "-new", "-key", "zerodds_key.pem",
             "-out", "zerodds.csr"], ca)
        run(["openssl", "ca", "-batch", "-config", "identity_ca_files/identity_ca_openssl.cnf",
             "-days", "3650", "-in", "zerodds.csr", "-out", "zerodds_cert.pem"], ca)

    governance = ca / "governance_signed.p7s"
    sign(ca, with_domain((suite / "TESTONLY_governance.xml").read_text(), domain), governance)

    rti_dir, rti_stem = VENDOR_DOCUMENTS["rti_connext_dds"]
    ours_permissions = {}
    for perm in PERMISSIONS:
        if submitted.is_dir():
            text = (submitted / f"TESTONLY_zerodds_permissions_{perm.lower()}.xml").read_text()
        else:
            text = (suite / rti_dir / f"{rti_stem}_{perm.lower()}.xml").read_text()
            text = re.sub(r"<subject_name>[^<]*</subject_name>",
                          f"<subject_name>{SUBJECT}</subject_name>", text)
        target = ca / f"zerodds_permissions_{perm.lower()}_signed.p7s"
        sign(ca, with_domain(text, domain), target)
        ours_permissions[perm] = str(target)

    name = Path(vendor).name
    key = next((k for k in VENDOR_DOCUMENTS if name.startswith(k)), None)
    if key is None:
        raise SystemExit(f"security_setup.py: no permissions documents known for {name}")
    vendor_dir, vendor_stem = VENDOR_DOCUMENTS[key]
    vendor_permissions = {}
    for perm in PERMISSIONS:
        text = (suite / vendor_dir / f"{vendor_stem}_{perm.lower()}.xml").read_text()
        target = ca / f"{key}_permissions_{perm.lower()}_signed.p7s"
        sign(ca, with_domain(text, domain), target)
        vendor_permissions[perm] = str(target)

    common = ["-domain", str(domain)]
    ours_profile = {
        "name": "zerodds",
        "exe": str(Path(ours).resolve()),
        "cwd": str(suite / "srcCxx"),
        "common_args": common + [
            "-identityCa", str(suite / "TESTONLY_identity_ca_cert.pem"),
            "-identityCert", str(ca / "zerodds_cert.pem"),
            "-identityKey", str(ca / "zerodds_key_pkcs8.pem"),
            "-permissionsCa", str(suite / "TESTONLY_permissions_ca_cert.pem"),
            "-acceptX509v1IdentityCerts"],
        "pub_args": ["-color", "BLUE"],
        "sub_args": [],
        "governance": str(governance),
        "permissions": ours_permissions,
        "env": {},
    }
    vendor_profile = {
        "name": name,
        "exe": str(Path(vendor).resolve()),
        # The suite's clients read their identity relative to srcCxx.
        "cwd": str(suite / "srcCxx"),
        "common_args": common,
        "pub_args": ["-color", "BLUE"],
        "sub_args": [],
        "governance": str(governance),
        "permissions": vendor_permissions,
        "env": {},
    }
    (work / "zerodds.json").write_text(json.dumps(ours_profile, indent=1))
    (work / "vendor.json").write_text(json.dumps(vendor_profile, indent=1))
    return work / "zerodds.json", work / "vendor.json"
