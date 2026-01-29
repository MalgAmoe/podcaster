# Build stage
FROM hexpm/elixir:1.17.3-erlang-27.1.2-debian-bookworm-20241016-slim AS build

RUN apt-get update -y && apt-get install -y build-essential git curl \
    && apt-get clean && rm -rf /var/lib/apt/lists/*

# Install Node.js for asset building
RUN curl -fsSL https://deb.nodesource.com/setup_20.x | bash - \
    && apt-get install -y nodejs

WORKDIR /app

# Install hex + rebar
RUN mix local.hex --force && \
    mix local.rebar --force

ENV MIX_ENV=prod

# Install mix dependencies
COPY mix.exs mix.lock ./
RUN mix deps.get --only $MIX_ENV
RUN mkdir config
COPY config/config.exs config/${MIX_ENV}.exs config/
RUN mix deps.compile

# Build assets
COPY assets/package.json assets/package-lock.json ./assets/
RUN cd assets && npm ci

COPY priv priv
COPY assets assets
COPY lib lib

RUN mix assets.deploy

# Compile and build release
COPY config/runtime.exs config/
RUN mix compile
RUN mix release

# Runtime stage
FROM debian:bookworm-slim AS runtime

RUN apt-get update -y && \
    apt-get install -y libstdc++6 openssl libncurses6 locales ca-certificates \
    && apt-get clean && rm -rf /var/lib/apt/lists/*

# Set locale
RUN sed -i '/en_US.UTF-8/s/^# //g' /etc/locale.gen && locale-gen
ENV LANG en_US.UTF-8
ENV LANGUAGE en_US:en
ENV LC_ALL en_US.UTF-8

WORKDIR /app
RUN chown nobody /app

ENV MIX_ENV=prod
ENV PHX_SERVER=true

COPY --from=build --chown=nobody:root /app/_build/${MIX_ENV}/rel/poddyclip_backend ./

USER nobody

CMD ["/app/bin/poddyclip_backend", "start"]
