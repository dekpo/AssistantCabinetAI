"""Inference providers.

The rest of the gateway depends on `AIProvider` only. This package is the single place allowed
to know that the runtime behind it is Ollama, or to name a model weight.
"""

from .base import (
    AIProvider,
    EmbeddingRequest,
    EmbeddingResult,
    GenerationChunk,
    GenerationRequest,
    Message,
    ProviderHealth,
    RerankRequest,
    RerankResult,
    Role,
)

__all__ = [
    "AIProvider",
    "EmbeddingRequest",
    "EmbeddingResult",
    "GenerationChunk",
    "GenerationRequest",
    "Message",
    "ProviderHealth",
    "RerankRequest",
    "RerankResult",
    "Role",
]
