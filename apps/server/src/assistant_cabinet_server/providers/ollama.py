"""Ollama implementation of `AIProvider`.

The only module in the gateway that knows this runtime exists. It never logs a request body and
it never puts an upstream body into an error, because an upstream body can echo the prompt.
"""

from __future__ import annotations

import json
import time
from collections.abc import AsyncIterator

import httpx

from ..core.errors import ErrorCode, GatewayError
from .base import (
    EmbeddingRequest,
    EmbeddingResult,
    GenerationChunk,
    GenerationRequest,
    ProviderHealth,
    RerankRequest,
    RerankResult,
)


class OllamaProvider:
    name = "ollama"

    def __init__(
        self,
        base_url: str,
        *,
        request_timeout_seconds: float = 180.0,
        health_timeout_seconds: float = 3.0,
    ) -> None:
        self._client = httpx.AsyncClient(
            base_url=base_url.rstrip("/"),
            timeout=httpx.Timeout(request_timeout_seconds, connect=10.0),
        )
        self._health_timeout_seconds = health_timeout_seconds

    async def generate(self, request: GenerationRequest) -> AsyncIterator[GenerationChunk]:
        payload: dict[str, object] = {
            "model": request.model,
            "messages": [message.model_dump() for message in request.messages],
            "stream": True,
        }
        options: dict[str, object] = {}
        if request.temperature is not None:
            options["temperature"] = request.temperature
        if request.max_output_tokens is not None:
            options["num_predict"] = request.max_output_tokens
        if options:
            payload["options"] = options

        try:
            async with self._client.stream("POST", "/api/chat", json=payload) as response:
                self._raise_for_status(response.status_code)
                async for line in response.aiter_lines():
                    chunk = self._parse_stream_line(line)
                    if chunk is not None:
                        yield chunk
        except httpx.ConnectError as error:
            raise self._unreachable() from error
        except httpx.TimeoutException as error:
            raise GatewayError(
                ErrorCode.provider_error,
                status_code=504,
                data={"provider": self.name, "reason": "timeout"},
            ) from error

    async def embed(self, request: EmbeddingRequest) -> EmbeddingResult:
        try:
            response = await self._client.post(
                "/api/embed", json={"model": request.model, "input": request.inputs}
            )
        except httpx.ConnectError as error:
            raise self._unreachable() from error
        self._raise_for_status(response.status_code)
        body = response.json()
        return EmbeddingResult(
            vectors=body.get("embeddings", []),
            prompt_tokens=body.get("prompt_eval_count"),
        )

    async def rerank(self, request: RerankRequest) -> RerankResult:
        """Not offered by this runtime.

        Refusing with a code is the honest answer: a fabricated ranking would silently degrade
        retrieval. Retrieval ranks locally until a provider supports it.
        """
        raise GatewayError(
            ErrorCode.provider_capability_unsupported,
            status_code=501,
            data={"provider": self.name, "capability": "rerank"},
        )

    async def check_health(self) -> ProviderHealth:
        started = time.monotonic()
        try:
            response = await self._client.get("/api/tags", timeout=self._health_timeout_seconds)
        except httpx.HTTPError:
            return ProviderHealth(reachable=False, error_code=ErrorCode.provider_unreachable.value)
        latency_ms = int((time.monotonic() - started) * 1000)
        if response.status_code >= 400:
            return ProviderHealth(
                reachable=False,
                latency_ms=latency_ms,
                error_code=ErrorCode.provider_error.value,
            )
        models = response.json().get("models", [])
        return ProviderHealth(reachable=True, latency_ms=latency_ms, model_count=len(models))

    async def aclose(self) -> None:
        await self._client.aclose()

    @staticmethod
    def _parse_stream_line(line: str) -> GenerationChunk | None:
        line = line.strip()
        if not line:
            return None
        try:
            event = json.loads(line)
        except json.JSONDecodeError as error:
            raise GatewayError(
                ErrorCode.provider_error,
                status_code=502,
                data={"provider": OllamaProvider.name, "reason": "malformed_stream"},
            ) from error
        if event.get("error"):
            # The upstream message may quote the prompt, so only the fact is reported.
            raise GatewayError(
                ErrorCode.provider_error,
                status_code=502,
                data={"provider": OllamaProvider.name, "reason": "runtime_error"},
            )
        done = bool(event.get("done"))
        return GenerationChunk(
            delta=event.get("message", {}).get("content", ""),
            done=done,
            prompt_tokens=event.get("prompt_eval_count") if done else None,
            completion_tokens=event.get("eval_count") if done else None,
        )

    def _unreachable(self) -> GatewayError:
        return GatewayError(
            ErrorCode.provider_unreachable,
            status_code=503,
            data={"provider": self.name},
        )

    @staticmethod
    def _raise_for_status(status_code: int) -> None:
        if status_code < 400:
            return
        if status_code == 404:
            raise GatewayError(
                ErrorCode.provider_error,
                status_code=502,
                data={"provider": OllamaProvider.name, "reason": "model_not_installed"},
            )
        raise GatewayError(
            ErrorCode.provider_error,
            status_code=502,
            data={"provider": OllamaProvider.name, "upstream_status": status_code},
        )
