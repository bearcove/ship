#!/usr/bin/env python3
# /// script
# requires-python = ">=3.11"
# dependencies = [
#   "pocket-tts",
# ]
# ///
"""
Pocket TTS worker for ship-server. Reads length-prefixed UTF-8 text requests
from stdin, writes raw f32-le PCM chunks to stdout using a simple framing
protocol:
  - stdin: 4-byte little-endian u32 text length, followed by UTF-8 bytes
  - stdout: 4-byte little-endian u32 chunk length in bytes (0 = end of utterance)
  - stdout: N bytes of f32-le PCM at 24kHz mono
A zero-length stdout frame signals end of audio for that utterance.
"""

import struct
import sys


def write_frame(data: bytes) -> None:
    sys.stdout.buffer.write(struct.pack("<I", len(data)))
    if data:
        sys.stdout.buffer.write(data)
    sys.stdout.buffer.flush()


def read_exact(size: int) -> bytes | None:
    data = bytearray()
    while len(data) < size:
        chunk = sys.stdin.buffer.read(size - len(data))
        if not chunk:
            if not data:
                return None
            raise EOFError("truncated TTS request")
        data.extend(chunk)
    return bytes(data)


def read_text_request() -> str | None:
    len_buf = read_exact(4)
    if len_buf is None:
        return None
    (size,) = struct.unpack("<I", len_buf)
    payload = read_exact(size)
    if payload is None:
        raise EOFError("truncated TTS request payload")
    return payload.decode("utf-8")


def main() -> None:
    import os
    voice = os.environ.get("TTS_VOICE", "marius")

    from pocket_tts import TTSModel
    print(f"[tts_worker] loading model, voice={voice}", file=sys.stderr, flush=True)
    model = TTSModel.load_model()
    voice_state = model.get_state_for_audio_prompt(voice)
    print(f"[tts_worker] ready, sample_rate={model.sample_rate}", file=sys.stderr, flush=True)

    while True:
        text = read_text_request()
        if text is None:
            break
        print(f"[tts_worker] synthesizing: {text!r}", file=sys.stderr, flush=True)
        for chunk in model.generate_audio_stream(voice_state, text):
            pcm_bytes = chunk.numpy().astype("float32").tobytes()
            write_frame(pcm_bytes)
        write_frame(b"")  # end-of-utterance

    write_frame(b"")  # end-of-stream


if __name__ == "__main__":
    main()
