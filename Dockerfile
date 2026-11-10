FROM alpine:3.23 AS build

RUN apk add --update --no-cache \
        build-base \
        cmake \
        curl

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs/ | sh -s -- -y

COPY . gwas-utils/

WORKDIR /gwas-utils

RUN source $HOME/.cargo/env && \
    cargo build --release


FROM scratch

LABEL org.opencontainers.image.title="gwas-utils" \
      org.opencontainers.image.description="Command-line utilities for working with GWAS summary statistics files" \
      org.opencontainers.image.source="https://github.com/MRCIEU/gwas-utils" \
      org.opencontainers.image.licenses="MIT"

COPY --from=build /gwas-utils/target/release/gu /usr/local/bin/gu
COPY --from=build /gwas-utils/LICENSE /gwas-utils/NOTICE /licenses/

CMD ["gu", "-h"]
