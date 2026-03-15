"""Type stubs for sonnetics_core - wake-word inference extension."""

def init(
    files: dict[str, bytes],
    sample_rate: int,
    channels: int,
) -> PyWakeEngine: ...

class PyWakeEngine:
    """Wake-word inference engine. Create with init(), then call detect()."""

    def reset(self) -> None: ...
    def detect(
        self,
        audio: list[float],
        threshold: float,
    ) -> str | None: ...
