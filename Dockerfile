FROM alpine:3.23 AS build

RUN apk add --update --no-cache \
        curl \
        gcc \
        musl-dev

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs/ | sh -s -- -y

COPY . .

WORKDIR /gwas-utils

RUN source $HOME/.cargo/env && \
    cargo build --release


FROM scratch

COPY --from=build /gwas-utils/target/release/gu /usr/local/bin/gu

CMD ["gu", "-h"]
