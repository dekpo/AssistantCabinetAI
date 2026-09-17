# Gateway image. Published on 127.0.0.1 only by compose.yaml; the runtime behind it (Ollama)
# is never exposed. Same image on the Windows workstation and on the Mac mini.
FROM python:3.13-slim

COPY --from=ghcr.io/astral-sh/uv:0.9 /uv /usr/local/bin/uv

ENV PYTHONUNBUFFERED=1 \
    PYTHONDONTWRITEBYTECODE=1 \
    UV_COMPILE_BYTECODE=1 \
    UV_LINK_MODE=copy \
    UV_PROJECT_ENVIRONMENT=/opt/venv \
    PATH="/opt/venv/bin:$PATH"

WORKDIR /app

# The lock file is used when it exists; the glob keeps the build working before it is generated.
COPY pyproject.toml uv.lock* ./
COPY src ./src
RUN uv sync --no-dev

# Run unprivileged: the gateway needs no disk, so it has no reason to own one.
RUN useradd --create-home --uid 10001 gateway
USER gateway

EXPOSE 8080

HEALTHCHECK --interval=15s --timeout=5s --start-period=10s --retries=5 \
    CMD python -c "import urllib.request,sys; sys.exit(0 if urllib.request.urlopen('http://127.0.0.1:8080/health', timeout=4).status == 200 else 1)"

CMD ["uvicorn", "assistant_cabinet_server.main:app", "--host", "0.0.0.0", "--port", "8080"]
