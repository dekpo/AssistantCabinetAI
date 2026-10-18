import { describe, expect, it, vi } from "vitest";
import { createBufferPlayer, type AudioContextLike, type BufferSourceLike } from "./playback";

function fakeContext(state = "running") {
  const sources: Array<BufferSourceLike & { started: boolean; stopped: boolean }> = [];
  const decoded: ArrayBuffer[] = [];
  const context: AudioContextLike = {
    destination: { destination: true },
    state,
    decodeAudioData: vi.fn(async (data: ArrayBuffer) => {
      decoded.push(data);
      return { decoded: true };
    }),
    createBufferSource: () => {
      const source = {
        buffer: null as unknown,
        onended: null as (() => void) | null,
        started: false,
        stopped: false,
        connect: vi.fn(),
        disconnect: vi.fn(),
        start() {
          this.started = true;
        },
        stop() {
          this.stopped = true;
        },
      };
      sources.push(source);
      return source;
    },
    resume: vi.fn(async () => {}),
    close: vi.fn(async () => {}),
  };
  return { context, sources, decoded };
}

const flush = async () => {
  for (let index = 0; index < 8; index += 1) {
    await Promise.resolve();
  }
};

describe("playing worker audio through Web Audio", () => {
  it("decodes a copy of the clip, plays it and resolves when it ends", async () => {
    const { context, sources, decoded } = fakeContext();
    const player = createBufferPlayer(() => context);
    const wav = new Uint8Array([1, 2, 3, 4]);

    const playing = player.play(wav);
    await flush();

    expect(new Uint8Array(decoded[0] as ArrayBuffer)).toStrictEqual(wav);
    // The context took its own copy: the caller's bytes are untouched and not the same buffer.
    expect(decoded[0]).not.toBe(wav.buffer);
    const source = sources[0];
    expect(source?.buffer).toStrictEqual({ decoded: true });
    expect(source?.connect).toHaveBeenCalledWith(context.destination);
    expect(source?.started).toBe(true);

    source?.onended?.();
    await expect(playing).resolves.toBeUndefined();
  });

  it("stops the clip in progress and resolves, once", async () => {
    const { context, sources } = fakeContext();
    const player = createBufferPlayer(() => context);

    const playing = player.play(new Uint8Array(2));
    await flush();
    player.stop();

    await expect(playing).resolves.toBeUndefined();
    expect(sources[0]?.stopped).toBe(true);
    expect(sources[0]?.onended).toBeNull();
  });

  it("a stop during the decoding means the clip never starts", async () => {
    const { context, sources } = fakeContext();
    let finish: (value: unknown) => void = () => {};
    context.decodeAudioData = () => new Promise((resolve) => (finish = resolve));
    const player = createBufferPlayer(() => context);

    const playing = player.play(new Uint8Array(2));
    await flush();
    player.stop();
    finish({});

    await expect(playing).resolves.toBeUndefined();
    expect(sources).toHaveLength(0);
  });

  it("wakes a suspended context before decoding", async () => {
    const { context } = fakeContext("suspended");
    const player = createBufferPlayer(() => context);

    void player.play(new Uint8Array(2));
    await flush();

    expect(context.resume).toHaveBeenCalled();
  });

  it("releases the context on dispose and makes a new one for the next clip", async () => {
    const worlds = [fakeContext(), fakeContext()];
    const makeContext = vi.fn(() => (worlds.shift() as ReturnType<typeof fakeContext>).context);
    const player = createBufferPlayer(makeContext);

    void player.play(new Uint8Array(2));
    await flush();
    await player.dispose();
    void player.play(new Uint8Array(2));
    await flush();

    expect(makeContext).toHaveBeenCalledTimes(2);
  });

  it("passes on a clip that cannot be decoded", async () => {
    const { context } = fakeContext();
    context.decodeAudioData = async () => {
      throw new Error("bad data");
    };
    const player = createBufferPlayer(() => context);

    await expect(player.play(new Uint8Array(2))).rejects.toThrow("bad data");
  });
});
