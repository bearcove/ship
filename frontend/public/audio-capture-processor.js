// AudioWorklet processor that captures mic input and posts Float32Array chunks.
// Runs at the AudioContext's native sample rate; resampling to 16kHz is done
// on the main thread before sending to the server.
class AudioCaptureProcessor extends AudioWorkletProcessor {
  constructor() {
    super();
    this._running = true;
    this.port.onmessage = (e) => {
      if (e.data === "stop") {
        this._running = false;
      }
    };
  }

  process(inputs) {
    if (!this._running) return false;
    const input = inputs[0];
    if (input && input[0] && input[0].length > 0) {
      // Post a copy of the mono channel samples
      this.port.postMessage(new Float32Array(input[0]));
    }
    return true;
  }
}

registerProcessor("audio-capture-processor", AudioCaptureProcessor);
