"""Logging utilities for StonepyreAssetForge."""

import logging
import sys

RESET = "\033[0m"
BOLD = "\033[1m"
RED = "\033[31m"
YELLOW = "\033[33m"
CYAN = "\033[36m"
GREEN = "\033[32m"
DIM = "\033[2m"


class _StepFormatter(logging.Formatter):
    """Colour-coded formatter that prefixes every line with [StonepyreAssetForge]."""

    _LEVEL_COLOURS = {
        logging.DEBUG: DIM,
        logging.INFO: RESET,
        logging.WARNING: YELLOW,
        logging.ERROR: RED,
        logging.CRITICAL: RED + BOLD,
    }

    def format(self, record: logging.LogRecord) -> str:
        colour = self._LEVEL_COLOURS.get(record.levelno, RESET)
        prefix = f"{CYAN}[StonepyreAssetForge]{RESET} "
        if record.levelno >= logging.ERROR:
            prefix = f"{RED}[StonepyreAssetForge]{RESET} "
        elif record.levelno == logging.WARNING:
            prefix = f"{YELLOW}[StonepyreAssetForge]{RESET} "
        message = super().format(record)
        return f"{prefix}{colour}{message}{RESET}"


def get_logger(name: str = "stonepyre_asset_forge", verbose: bool = False) -> logging.Logger:
    logger = logging.getLogger(name)
    if logger.handlers:
        return logger

    level = logging.DEBUG if verbose else logging.INFO
    logger.setLevel(level)

    handler = logging.StreamHandler(sys.stdout)
    handler.setLevel(level)
    handler.setFormatter(_StepFormatter())
    logger.addHandler(handler)
    logger.propagate = False
    return logger


def log_step(logger: logging.Logger, step: int, total: int, message: str) -> None:
    logger.info(f"[{step}/{total}] {message}")
