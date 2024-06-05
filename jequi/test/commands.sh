rm ./*.csr
rm ./*.key
rm ./*.pem
rm ./*.srl

ext="
authorityKeyIdentifier=keyid,issuer
basicConstraints=CA:TRUE
keyUsage = keyCertSign, cRLSign, digitalSignature, nonRepudiation, keyEncipherment, dataEncipherment
"

openssl req -x509 -days 1825 -newkey rsa:2048 -nodes -keyout root-ca.key -out root-ca.pem -subj "/C=/ST=/L=/O=/OU=/CN=root" &&
	openssl req -nodes -newkey rsa:2048 -keyout intermediate.key -out intermediate.csr -subj "/C=/ST=/L=/O=/OU=/CN=intermediate" &&
	openssl x509 -req -CA root-ca.pem -CAkey root-ca.key -in intermediate.csr -out intermediate.pem -days 1550 -CAcreateserial -extfile <(echo "$ext") &&
	openssl req -nodes -newkey rsa:2048 -keyout leaf-cert.key -out leaf-cert.csr -subj "/C=/ST=/L=/O=/OU=/CN=localhost" &&
	openssl x509 -req -CA intermediate.pem -CAkey intermediate.key -in leaf-cert.csr -out leaf-cert.pem -days 1550 -CAcreateserial &&
	cat intermediate.pem >>leaf-cert.pem

