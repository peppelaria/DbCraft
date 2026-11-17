#!/usr/bin/env bash
# Throwaway certificates for the SSL integration tests (never use them for anything else):
# a CA, a server certificate for localhost/127.0.0.1 and a client certificate, in target/tls-test.
set -euo pipefail
dir="${1:-target/tls-test}"
mkdir -p "$dir"
cd "$dir"
if [ -f ca.crt ] && [ -f server.crt ] && [ -f client.crt ]; then
  exit 0
fi
openssl req -x509 -newkey rsa:2048 -nodes -days 3650 -subj "/CN=DbCraft Test CA" -keyout ca.key -out ca.crt 2>/dev/null
cat > server.ext <<EXT
basicConstraints=CA:FALSE
keyUsage=digitalSignature,keyEncipherment
extendedKeyUsage=serverAuth
subjectAltName=DNS:localhost,IP:127.0.0.1
EXT
openssl req -newkey rsa:2048 -nodes -subj "/CN=localhost" -keyout server.key -out server.csr 2>/dev/null
openssl x509 -req -in server.csr -CA ca.crt -CAkey ca.key -CAcreateserial -days 3650 -extfile server.ext -out server.crt 2>/dev/null
printf 'basicConstraints=CA:FALSE\nkeyUsage=digitalSignature\nextendedKeyUsage=clientAuth\n' > client.ext
# PostgreSQL maps the certificate's CN to the user name.
openssl req -newkey rsa:2048 -nodes -subj "/CN=postgres" -keyout client.key -out client.csr 2>/dev/null
openssl x509 -req -in client.csr -CA ca.crt -CAkey ca.key -CAcreateserial -days 3650 -extfile client.ext -out client.crt 2>/dev/null
chmod 644 ./*.crt ./*.key
rm -f ./*.csr ./*.ext ./*.srl
echo "certificates in $dir"
