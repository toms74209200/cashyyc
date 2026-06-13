import threading
from dataclasses import dataclass

from opentelemetry.sdk.trace import TracerProvider
from opentelemetry.sdk.trace.export import SimpleSpanProcessor
from opentelemetry.sdk.trace.export.in_memory_span_exporter import InMemorySpanExporter
from opentelemetry.trace import Span

_TEST_HEADER = "Test"
_GIVEN_HEADER = "Given (ms)"
_WHEN_HEADER = "When (ms)"
_THEN_HEADER = "Then (ms)"
_TOTAL_HEADER = "Total (ms)"
_MAX_TEST_COLUMN_WIDTH = 60


@dataclass
class TestTiming:
    test_module: str
    test_name: str
    given_nanos: int
    when_nanos: int
    then_nanos: int


class TestTimingCollector:
    def __init__(self) -> None:
        self._timings: list[TestTiming] = []
        self._lock = threading.Lock()

    def record(self, timing: TestTiming) -> None:
        with self._lock:
            self._timings.append(timing)

    def all(self) -> list[TestTiming]:
        with self._lock:
            return list(self._timings)


_collector = TestTimingCollector()


def get_collector() -> TestTimingCollector:
    return _collector


class StepTimer:
    def __init__(self) -> None:
        self._exporter = InMemorySpanExporter()
        provider = TracerProvider()
        provider.add_span_processor(SimpleSpanProcessor(self._exporter))
        self._tracer = provider.get_tracer("acceptance-test")
        self._current_span: Span | None = None

    def start_step(self, step_type: str) -> None:
        self.end_step()
        self._current_span = self._tracer.start_span(step_type)

    def end_step(self) -> None:
        if self._current_span is not None:
            self._current_span.end()
            self._current_span = None

    def collect_phase_nanos(self) -> dict[str, int]:
        result = {"given": 0, "when": 0, "then": 0}
        for span in self._exporter.get_finished_spans():
            if span.name in result:
                result[span.name] += span.end_time - span.start_time
        self._exporter.clear()
        return result


def render_table(timings: list[TestTiming]) -> str:
    def fmt(value: float) -> str:
        return f"{value:.2f}"

    rows = sorted(
        [
            {
                "test": f"{t.test_module} > {t.test_name}",
                "given_ms": t.given_nanos / 1_000_000.0,
                "when_ms": t.when_nanos / 1_000_000.0,
                "then_ms": t.then_nanos / 1_000_000.0,
                "total_ms": (t.given_nanos + t.when_nanos + t.then_nanos) / 1_000_000.0,
            }
            for t in timings
        ],
        key=lambda r: r["total_ms"],
        reverse=True,
    )

    test_width = min(
        _MAX_TEST_COLUMN_WIDTH,
        max(len(_TEST_HEADER), max(len(r["test"]) for r in rows)),
    )
    given_width = max(len(_GIVEN_HEADER), max(len(fmt(r["given_ms"])) for r in rows))
    when_width = max(len(_WHEN_HEADER), max(len(fmt(r["when_ms"])) for r in rows))
    then_width = max(len(_THEN_HEADER), max(len(fmt(r["then_ms"])) for r in rows))
    total_width = max(len(_TOTAL_HEADER), max(len(fmt(r["total_ms"])) for r in rows))

    def make_row(
        test_lines: list[str], given: str, when: str, then: str, total: str
    ) -> str:
        result_lines = []
        for i, line in enumerate(test_lines):
            g = given if i == 0 else ""
            w = when if i == 0 else ""
            th = then if i == 0 else ""
            tot = total if i == 0 else ""
            result_lines.append(
                f"{line:<{test_width}} | {g:>{given_width}} | {w:>{when_width}} | "
                f"{th:>{then_width}} | {tot:>{total_width}}"
            )
        return "\n".join(result_lines)

    def wrap_test_name(name: str) -> list[str]:
        if len(name) <= test_width:
            return [name]
        lines: list[str] = []
        remaining = name
        while len(remaining) > test_width:
            break_at = remaining.rfind(" ", 0, test_width)
            if break_at <= 0:
                break_at = test_width
            lines.append(remaining[:break_at].rstrip())
            remaining = remaining[break_at:].lstrip()
        lines.append(remaining)
        return lines

    header = make_row(
        [_TEST_HEADER], _GIVEN_HEADER, _WHEN_HEADER, _THEN_HEADER, _TOTAL_HEADER
    )
    separator = "".join("+" if c == "|" else "-" for c in header.split("\n")[0])
    body_rows = [
        make_row(
            wrap_test_name(r["test"]),
            fmt(r["given_ms"]),
            fmt(r["when_ms"]),
            fmt(r["then_ms"]),
            fmt(r["total_ms"]),
        )
        for r in rows
    ]

    return "\n".join([header, separator] + body_rows)
