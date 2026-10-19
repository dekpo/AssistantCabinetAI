import { chunkLevel } from "./wav";

/**
 * Cuts a long recording into passages while it is still being spoken, so a passage is transcribed
 * as the user goes on and a long note never waits for its end. Energy only, no model and no
 * dependency: a block is speech when its level is above a threshold that follows the room's noise.
 *
 * The cuts exist for latency, not for an engine limit (the engine took clips of 45 s without a
 * failure); the constants are named so they can be tuned on real recordings. Every sample of a
 * passage that holds speech is kept, none is changed, and nothing but a passage with no speech in
 * it is dropped. Pure: no clock, no React, no Tauri, no recorder.
 */

/** A cut is made only once the passage is at least this long (seconds). */
export const MIN_SEGMENT_SECONDS = 1.5;
/** A silence this long, after speech, ends a passage (seconds). The engine closes every passage with
 * a full stop, so a shorter silence put full stops in the middle of sentences (human test, 19 Oct
 * 2026: 0.7 s did; 1.2 s trades about half a second of delay for fewer of them). */
export const CUT_SILENCE_SECONDS = 1.2;
/** A passage never grows past this (seconds); it is then cut at its quietest block. */
export const MAX_SEGMENT_SECONDS = 25;
/** Silence kept in front of the first block of speech, so a word is not clipped at its start (seconds). */
export const PRE_ROLL_SECONDS = 0.25;
/** A passage with less speech than this is noise (a click, a cough) and is dropped (seconds). */
export const MIN_SPEECH_SECONDS = 0.2;

/** A block at or above this level (root mean square, 0 to 1) is speech whatever the room sounds like. */
export const SPEECH_LEVEL_FLOOR = 0.008;
/** Speech is also at least this many times the noise of the room. */
export const NOISE_RATIO = 3;
/** How fast the noise estimate rises towards a louder block, per block: slow, so that speech does
 * not become the noise within a passage. It falls at once to a quieter block. */
export const NOISE_RISE = 0.0002;

/** One passage: mono samples at `rate`, the very samples the microphone gave. */
export interface Segment {
  samples: Float32Array;
  rate: number;
}

export interface Segmenter {
  /** Feeds the next block of the microphone. May hand over one passage. */
  push: (block: Float32Array) => void;
  /** The recording ended: hands over what is left, if it holds speech. */
  finish: () => void;
}

interface Block {
  samples: Float32Array;
  level: number;
  speech: boolean;
}

function join(blocks: readonly Block[]): Float32Array {
  const joined = new Float32Array(blocks.reduce((total, block) => total + block.samples.length, 0));
  let offset = 0;
  for (const block of blocks) {
    joined.set(block.samples, offset);
    offset += block.samples.length;
  }
  return joined;
}

function lengthOf(blocks: readonly Block[]): number {
  return blocks.reduce((total, block) => total + block.samples.length, 0);
}

/**
 * `emit` receives each passage in order. `rate` is the sample rate of the blocks, and every length
 * is counted in samples from it.
 */
export function createSegmenter(rate: number, emit: (segment: Segment) => void): Segmenter {
  const seconds = (value: number) => Math.round(value * rate);
  const minSamples = seconds(MIN_SEGMENT_SECONDS);
  const silenceSamples = seconds(CUT_SILENCE_SECONDS);
  const maxSamples = seconds(MAX_SEGMENT_SECONDS);
  const preRollSamples = seconds(PRE_ROLL_SECONDS);
  const minSpeechSamples = seconds(MIN_SPEECH_SECONDS);

  /** The passage being built; empty until the first block of speech. */
  let current: Block[] = [];
  /** The silence heard since the last passage, newest last, at most `preRollSamples`. */
  let preRoll: Block[] = [];
  /* Starts as a quiet room, so that a recording that opens on speech is heard from its first block. */
  let noise = SPEECH_LEVEL_FLOOR / NOISE_RATIO;

  const speechSamplesOf = (blocks: readonly Block[]) =>
    blocks.reduce((total, block) => total + (block.speech ? block.samples.length : 0), 0);

  /** Samples of silence at the end of the passage. */
  const trailingSilence = (blocks: readonly Block[]) => {
    let total = 0;
    for (let index = blocks.length - 1; index >= 0 && !(blocks[index] as Block).speech; index -= 1) {
      total += (blocks[index] as Block).samples.length;
    }
    return total;
  };

  const hand = (blocks: readonly Block[]) => {
    if (speechSamplesOf(blocks) >= minSpeechSamples) {
      emit({ samples: join(blocks), rate });
    }
  };

  const cutAtQuietest = () => {
    // The first part keeps at least the minimum length; among equally quiet blocks the last one wins.
    let from = 0;
    let taken = 0;
    while (from < current.length - 1 && taken < minSamples) {
      taken += (current[from] as Block).samples.length;
      from += 1;
    }
    let quietest = Math.min(from, current.length - 1);
    for (let index = quietest; index < current.length; index += 1) {
      if ((current[index] as Block).level <= (current[quietest] as Block).level) {
        quietest = index;
      }
    }
    const first = current.slice(0, quietest + 1);
    const rest = current.slice(quietest + 1);
    hand(first);
    current = rest;
  };

  return {
    push: (samples) => {
      if (samples.length === 0) {
        return;
      }
      const level = chunkLevel(samples);
      noise = level < noise ? level : noise + (level - noise) * NOISE_RISE;
      const block: Block = {
        samples,
        level,
        speech: level >= Math.max(SPEECH_LEVEL_FLOOR, noise * NOISE_RATIO),
      };
      if (current.length === 0) {
        if (!block.speech) {
          preRoll.push(block);
          while (lengthOf(preRoll) - (preRoll[0] as Block).samples.length >= preRollSamples) {
            preRoll.shift();
          }
          return;
        }
        current = [...preRoll, block];
        preRoll = [];
        return;
      }
      current.push(block);
      const length = lengthOf(current);
      if (length >= maxSamples) {
        cutAtQuietest();
      } else if (length >= minSamples && trailingSilence(current) >= silenceSamples) {
        hand(current);
        current = [];
      }
    },
    finish: () => {
      hand(current);
      current = [];
      preRoll = [];
    },
  };
}
