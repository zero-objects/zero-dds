rsa_pss_*.smime.p7s: the permissions XML of permissions.xml, CMS-signed by an
RSA-2048 signer (CN=rsa-pss-signer, issued by the EC P-256 CA of
rsa_pss_ca_cert.pem) with RSASSA-PSS (SHA-256, MGF1-SHA256) and the salt
length in the file name (0, 20, 32, 64 octets). Made with OpenSSL 4.0.3:

  openssl cms -sign -in permissions.xml -text -signer signer_cert.pem \
      -inkey signer_key.pem -keyopt rsa_padding_mode:pss \
      -keyopt rsa_pss_saltlen:N -out rsa_pss_saltN.smime.p7s -nodetach

The keys are not kept. generate_fixtures.sh does not remake these files.
