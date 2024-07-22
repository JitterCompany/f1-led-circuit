#!/bin/bash

SERVER="jsonplaceholder.typicode.com:443"
CERT_FILE="placeholder_cert.pem"

# Extract certificates
echo | openssl s_client -showcerts -servername jsonplaceholder.typicode.com -connect $SERVER 2>/dev/null > temp_certs.pem

# Extract the root certificate and save it to root_cert.pem
awk 'BEGIN { found=0 } 
     /-----BEGIN CERTIFICATE-----/ { found=1 }
     { if (found) print }
     /-----END CERTIFICATE-----/ { found=0 }' temp_certs.pem > $CERT_FILE

# Clean up temporary files
rm temp_certs.pem

echo "Root certificate saved to $CERT_FILE"
