"""
Lightweight Token Estimator.
"""

import math


def estimate_tokens(text: str) -> int:
    """
    Estimates token count based on character length.
    Rule of thumb for GPT-4 (cl100k_base): ~4 characters per token.
    This avoids heavy dependencies like tiktoken while providing a useful metric.
    """
    if not text:
        return 0
    return math.ceil(len(text) / 4)
