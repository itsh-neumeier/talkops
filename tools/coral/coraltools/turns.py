"""Turn detector, ported from crates/talkops-media-worker/src/realtime.rs.

The benchmark uses the same rules as the realtime client of the worker, so
the measured segment latency matches the live preview: 20 ms frames, a
300 ms lead-in, 700 ms silence ends a turn, a turn is cut after 15 s, and a
turn with less than 200 ms of speech is dropped.
"""
from __future__ import annotations

import math
from collections import deque
from dataclasses import dataclass, field

FRAME_SECS = 0.02
LEAD_FRAMES = 15
END_FRAMES = 35
MIN_SPEECH_FRAMES = 10
MAX_TURN_SECS = 15.0


@dataclass
class Append:
    samples: list


@dataclass
class Commit:
    start: float
    end: float


@dataclass
class Clear:
    pass


@dataclass
class Turns:
    rate: int
    time: float = 0.0
    rest: list = field(default_factory=list)
    lead: deque = field(default_factory=deque)
    turn_start: float | None = None
    speech_frames: int = 0
    silent_frames: int = 0
    floor: float = 0.002

    def push(self, samples) -> list:
        frame = int(self.rate * FRAME_SECS)
        self.rest.extend(samples)
        out: list = []
        append: list = []
        at = 0
        while len(self.rest) - at >= frame:
            f = self.rest[at:at + frame]
            at += frame
            rms = math.sqrt(sum(s * s for s in f) / frame)
            speech = rms >= max(self.floor * 4.0, 0.006)
            if not speech:
                self.floor = min(self.floor * 0.98 + rms * 0.02, 0.05)
            end = self.time + FRAME_SECS
            if self.turn_start is None:
                if speech:
                    self.turn_start = self.time - len(self.lead) * FRAME_SECS
                    self.speech_frames, self.silent_frames = 1, 0
                    for lf in self.lead:
                        append.extend(lf)
                    self.lead.clear()
                    append.extend(f)
                else:
                    self.lead.append(f)
                    if len(self.lead) > LEAD_FRAMES:
                        self.lead.popleft()
            else:
                start = self.turn_start
                append.extend(f)
                if speech:
                    self.speech_frames += 1
                    self.silent_frames = 0
                else:
                    self.silent_frames += 1
                if self.silent_frames >= END_FRAMES or end - start >= MAX_TURN_SECS:
                    if append:
                        out.append(Append(append))
                        append = []
                    if self.speech_frames >= MIN_SPEECH_FRAMES:
                        out.append(Commit(start, end))
                    else:
                        out.append(Clear())
                    self.turn_start = end if self.silent_frames < END_FRAMES else None
                    self.speech_frames = 0
                    self.silent_frames = 0
            self.time = end
        del self.rest[:at]
        if append:
            out.append(Append(append))
        return out

    def finish(self):
        if self.turn_start is None:
            return None
        start, self.turn_start = self.turn_start, None
        return Commit(start, self.time) if self.speech_frames >= MIN_SPEECH_FRAMES else Clear()
