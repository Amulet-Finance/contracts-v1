FROM golang:1.21-bullseye as builder

ARG LDFLAGS
RUN mkdir /app
WORKDIR /app
COPY go.mod go.sum ./
RUN go mod download
COPY . .
RUN go build -ldflags "${LDFLAGS}" -a -o build/neutron_query_relayer ./cmd/neutron_query_relayer/*.go

FROM debian:buster
WORKDIR /
RUN apt update && apt install ca-certificates curl -y && apt-get clean && apt-get install -y dos2unix
ADD ["https://github.com/CosmWasm/wasmvm/releases/download/v1.5.2/libwasmvm.x86_64.so","https://github.com/CosmWasm/wasmvm/releases/download/v1.5.2/libwasmvm.aarch64.so","/lib/"]
COPY --from=builder /app/build/neutron_query_relayer /bin/neutron_query_relayer

COPY run.sh /run.sh
RUN dos2unix /run.sh
RUN chmod +x /run.sh /bin/neutron_query_relayer

EXPOSE 9999

ENTRYPOINT ["./run.sh"]

