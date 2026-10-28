Test vectors of RSASSA-PSS (SHA-256, MGF1-SHA256) with different salt lengths,
made with OpenSSL 4.0.3:

  openssl dgst -sha256 -sigopt rsa_padding_mode:pss -sigopt rsa_pss_saltlen:N \
      -sigopt rsa_mgf1_md:sha256 -sign key.pem -out sig<bits>_N.bin msg.bin

msg.bin is signed. pub<bits>.der is the RSAPublicKey (PKCS #1) of the key,
cert2048.der a self-signed certificate of the 2048-bit key. The private keys
are not kept.
