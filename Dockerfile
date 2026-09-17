# ---------------------------------------------------
# ETAPA 1: Construcción (Builder)
# ---------------------------------------------------
FROM rust:slim-bookworm AS builder

# Establecemos el directorio de trabajo
WORKDIR /usr/src/event_api

# Copiamos los archivos de dependencias y el código fuente
COPY Cargo.toml Cargo.lock ./
COPY src ./src

# Compilamos el proyecto en modo release (optimizado para producción)
RUN cargo build --release

# ---------------------------------------------------
# ETAPA 2: Ejecución (Runtime)
# ---------------------------------------------------
FROM debian:bookworm-slim

WORKDIR /app

# Instalamos dependencias del sistema necesarias (como certificados SSL)
RUN apt-get update && apt-get install -y libssl3 ca-certificates && rm -rf /var/lib/apt/lists/*

# Copiamos el binario compilado desde la etapa anterior
COPY --from=builder /usr/src/event_api/target/release/eventManagement_api /app/event_api

# Exponemos el puerto de Axum
EXPOSE 3000

# Comando para ejecutar el microservicio
CMD ["./event_api"]