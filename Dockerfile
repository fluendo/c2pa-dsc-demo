FROM c2pa-dsc-base:latest

# facebl0r (frei0r) fallback anonymizer: filters + OpenCV cascade data.
RUN apt-get update && apt-get install -y --no-install-recommends \
        frei0r-plugins opencv-data \
    && rm -rf /var/lib/apt/lists/*
ENV FREI0R_PATH=/usr/lib/frei0r-1

WORKDIR /root/c2pa-dsc-live-demo

COPY . .

RUN cargo build --release

RUN mkdir -p /tmp/c2pa-certs \
    && openssl req -x509 -newkey rsa:2048 -nodes \
        -keyout /tmp/c2pa-certs/ca.key -out /tmp/c2pa-certs/ca.crt -days 3650 \
        -subj "/C=ES/O=Fluendo S.A./CN=Fluendo DSC Root CA" \
    && openssl req -new -newkey rsa:2048 -nodes \
        -keyout /tmp/c2pa-certs/provider.key -out /tmp/c2pa-certs/provider.csr \
        -subj "/C=ES/O=Fluendo S.A./CN=Fluendo DSC Signer" \
    && openssl x509 -req -in /tmp/c2pa-certs/provider.csr \
        -CA /tmp/c2pa-certs/ca.crt -CAkey /tmp/c2pa-certs/ca.key -CAcreateserial \
        -out /tmp/c2pa-certs/provider.crt -days 365 \
    && rm /tmp/c2pa-certs/provider.csr

COPY entrypoint.sh /root/entrypoint.sh
RUN chmod +x /root/entrypoint.sh

ENTRYPOINT ["/root/entrypoint.sh"]
