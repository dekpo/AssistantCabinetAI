/**
 * Plays audio a worker produced (a WAV in memory) through Web Audio: `decodeAudioData` and a buffer
 * source. Nothing here makes an object URL or a blob, so the content security policy needs neither
 * `media-src` nor `blob:`. The audio is not kept once it has been played.
 */

/** The parts of an `AudioContext` and of a source node this helper uses. */
export interface BufferSourceLike {
  buffer: unknown;
  onended: (() => void) | null;
  connect: (destination: unknown) => void;
  start: () => void;
  stop: () => void;
  disconnect: () => void;
}

export interface AudioContextLike {
  readonly destination: unknown;
  readonly state: string;
  decodeAudioData: (data: ArrayBuffer) => Promise<unknown>;
  createBufferSource: () => BufferSourceLike;
  resume: () => Promise<void>;
  close: () => Promise<void>;
}

export interface BufferPlayer {
  /** Plays one clip at its natural speed (the synthesiser already applied the reading speed).
   * Resolves when it ended or was stopped; rejects when it cannot be decoded. */
  play: (wav: Uint8Array) => Promise<void>;
  stop: () => void;
  /** Releases the audio context. The player can be used again: it makes a new one. */
  dispose: () => Promise<void>;
}

export function createBufferPlayer(
  makeContext: () => AudioContextLike = () => new AudioContext() as unknown as AudioContextLike,
): BufferPlayer {
  let context: AudioContextLike | null = null;
  let current: { source: BufferSourceLike; finish: () => void } | null = null;
  let generation = 0;

  const stop = () => {
    generation += 1;
    const playing = current;
    current = null;
    if (playing !== null) {
      playing.source.onended = null;
      try {
        playing.source.stop();
      } catch {
        // A source that never started or already ended cannot be stopped again.
      }
      playing.source.disconnect();
      playing.finish();
    }
  };

  return {
    play: async (wav) => {
      stop();
      const mine = generation;
      context ??= makeContext();
      if (context.state === "suspended") {
        await context.resume();
      }
      // `decodeAudioData` takes the buffer it is given, so it gets a copy.
      const decoded = await context.decodeAudioData(wav.slice().buffer);
      if (mine !== generation) {
        return;
      }
      const source = context.createBufferSource();
      source.buffer = decoded;
      source.connect(context.destination);
      await new Promise<void>((resolve) => {
        current = { source, finish: resolve };
        source.onended = () => {
          if (current?.source === source) {
            current = null;
          }
          source.disconnect();
          resolve();
        };
        source.start();
      });
    },
    stop,
    dispose: async () => {
      stop();
      const closing = context;
      context = null;
      await closing?.close();
    },
  };
}
