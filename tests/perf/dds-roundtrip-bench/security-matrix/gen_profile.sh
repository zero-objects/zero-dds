#!/usr/bin/env bash
# Generiert pro Security-Profil XSD-konforme governance + per-Vendor-Assets:
#   profiles/<N>/text/{governance.p7s,permissions_<role>.p7s,certs->}  (cyclone/fastdds/zerodds, CMS -text)
#   profiles/<N>/raw/{governance.p7s,permissions_<role>.p7s,certs->}   (opendds, raw CMS)
#   profiles/<N>/cyclone_<role>.xml                                     (cyclone, file://-Pfade auf text/)
#
# REPRODUZIERBAR aus jedem Checkout: certs + permissions kommen aus dem
# in-repo `../security/` (einmal `bash ../security/gen.sh` fuer den cert-Tree);
# die generierten Profile landen in `./profiles/` (gitignored). Keine
# /tmp/dds-bench-security-Hardcodes mehr. Per env ueberschreibbar:
#   SECDIR       Quelle certs/ + permissions_*.xml  (Default: ../security)
#   PROFILES_DIR Ausgabe-Wurzel                      (Default: ./profiles)
#
# Aufruf: gen_profile.sh <name> <disc> <liv> <rtps> <meta> <data> <join_ac> <rw_ac> <en_disc> <en_liv>
set -e
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SECDIR="${SECDIR:-$(cd "$HERE/../security" && pwd)}"
PROFILES_DIR="${PROFILES_DIR:-$HERE/profiles}"

N="$1"; DISC="$2"; LIV="$3"; RTPS="$4"; META="$5"; DATA="$6"; JOINAC="$7"; RWAC="$8"; ENDISC="$9"; ENLIV="${10}"
CERTS="$SECDIR/certs"
OUT="$PROFILES_DIR/$N"
mkdir -p "$OUT/text" "$OUT/raw"
cat > "$OUT/governance.xml" <<XML
<?xml version="1.0" encoding="UTF-8"?>
<dds xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:noNamespaceSchemaLocation="http://www.omg.org/spec/DDS-SECURITY/20170901/omg_shared_ca_governance.xsd">
  <domain_access_rules>
    <domain_rule>
      <domains>
        <id>200</id>
      </domains>
      <allow_unauthenticated_participants>false</allow_unauthenticated_participants>
      <enable_join_access_control>$JOINAC</enable_join_access_control>
      <discovery_protection_kind>$DISC</discovery_protection_kind>
      <liveliness_protection_kind>$LIV</liveliness_protection_kind>
      <rtps_protection_kind>$RTPS</rtps_protection_kind>
      <topic_access_rules>
        <topic_rule>
          <topic_expression>*</topic_expression>
          <enable_discovery_protection>$ENDISC</enable_discovery_protection>
          <enable_liveliness_protection>$ENLIV</enable_liveliness_protection>
          <enable_read_access_control>$RWAC</enable_read_access_control>
          <enable_write_access_control>$RWAC</enable_write_access_control>
          <metadata_protection_kind>$META</metadata_protection_kind>
          <data_protection_kind>$DATA</data_protection_kind>
        </topic_rule>
      </topic_access_rules>
    </domain_rule>
  </domain_access_rules>
</dds>
XML
# CMS-Signer = Permissions-CA DIREKT (nicht ein EE-"authority"-Cert), sonst
# cyclone: PKCS7_get0_signers: signer certificate not found. text/ (PKCS7_TEXT)
# fuer cyclone/fastdds/zerodds, raw/ als Reserve fuer OpenDDS (diese Version
# akzeptiert ebenfalls text/). Siehe README + cross-vendor-matrix-findings.md.
SIGN="-signer $CERTS/permissions_ca.pem -inkey $CERTS/permissions_ca_key.pem"
openssl smime -sign -in "$OUT/governance.xml" -text -out "$OUT/text/governance.p7s" $SIGN
openssl smime -sign -in "$OUT/governance.xml"       -out "$OUT/raw/governance.p7s"  $SIGN
for who in ping pong; do
  openssl smime -sign -in "$SECDIR/permissions_${who}.xml" -text -out "$OUT/text/permissions_${who}.p7s" $SIGN
  openssl smime -sign -in "$SECDIR/permissions_${who}.xml"       -out "$OUT/raw/permissions_${who}.p7s"  $SIGN
done
ln -sfn "$CERTS" "$OUT/text/certs"
ln -sfn "$CERTS" "$OUT/raw/certs"
# Per-Profil cyclone-XML (CYCLONEDDS_URI braucht absolute file://-Pfade:
# shared certs + profil-text-governance/permissions).
for who in ping pong; do
cat > "$OUT/cyclone_${who}.xml" <<CYC
<CycloneDDS xmlns="https://cdds.io/config">
  <Domain>
    <Security>
      <Authentication>
        <Library finalizeFunction="finalize_authentication" initFunction="init_authentication" path="dds_security_auth"/>
        <IdentityCertificate>file://$CERTS/${who}_cert.pem</IdentityCertificate>
        <IdentityCA>file://$CERTS/identity_ca.pem</IdentityCA>
        <PrivateKey>file://$CERTS/${who}_key.pem</PrivateKey>
      </Authentication>
      <AccessControl>
        <Library finalizeFunction="finalize_access_control" initFunction="init_access_control" path="dds_security_ac"/>
        <PermissionsCA>file://$CERTS/permissions_ca.pem</PermissionsCA>
        <Governance>file://$OUT/text/governance.p7s</Governance>
        <Permissions>file://$OUT/text/permissions_${who}.p7s</Permissions>
      </AccessControl>
      <Cryptographic>
        <Library finalizeFunction="finalize_crypto" initFunction="init_crypto" path="dds_security_crypto"/>
      </Cryptographic>
    </Security>
  </Domain>
</CycloneDDS>
CYC
done
echo "OK $N: disc=$DISC liv=$LIV rtps=$RTPS meta=$META data=$DATA join_ac=$JOINAC rw_ac=$RWAC en_disc=$ENDISC en_liv=$ENLIV"
