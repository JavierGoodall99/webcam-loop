import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { decodeStripRow, summarize, gateFailures, MARKER_LIVE, MARKER_SLATE } from './strip-codec.mjs';

const fixtures = JSON.parse(readFileSync(new URL('./fixtures.json', import.meta.url), 'utf8'));

for (const c of fixtures.cases) {
  test(`decodes the Python-encoded strip: ${c.name}`, () => {
    const row = new Uint8ClampedArray(Buffer.from(c.rowRgbaBase64, 'base64'));
    assert.deepEqual(decodeStripRow(row, c.width), { marker: c.marker, counter: c.counter });
  });
}

test('rejects noise and too-narrow rows', () => {
  const noise = new Uint8ClampedArray(1280 * 4);
  let x = 12345;
  for (let i = 0; i < noise.length; i++) { x = (x * 1103515245 + 12345) & 0x7fffffff; noise[i] = x & 255; }
  assert.equal(decodeStripRow(noise, 1280), null);
  assert.equal(decodeStripRow(new Uint8ClampedArray(100 * 4), 100), null);
});

const L = MARKER_LIVE;
const steady = (seconds = 3, fps = 30, start = 0) =>
  Array.from({ length: Math.floor(seconds * fps) }, (_, i) => [i / fps, L, start + i]);

test('a steady stream is clean', () => {
  const s = summarize(steady());
  assert.equal(s.dropped, 0);
  assert.equal(s.repeats, 0);
  assert.equal(s.freezeEvents, 0);
  assert.equal(s.invalidFrames, 0);
  assert.ok(s.fps > 29 && s.fps < 31);
  assert.deepEqual(gateFailures(s), []);
});

test('counter jumps count as dropped frames', () => {
  const a = steady(1);
  const b = a.slice(0, 10).concat(a.slice(10).map(([t, m, c]) => [t, m, c + 5]));
  assert.equal(summarize(b).dropped, 5);
});

test('an unchanging counter for 2.5 s is a freeze event', () => {
  const frozen = Array.from({ length: 25 }, (_, i) => [1.0 + i * 0.1, L, 29]);
  const s = summarize(steady(1).concat(frozen));
  assert.equal(s.freezeEvents, 1);
  assert.ok(gateFailures(s).some((f) => f.includes('freeze')));
});

test('slate and undecodable frames are counted separately', () => {
  const s = summarize(steady(1).concat([[1.0, MARKER_SLATE, 0], [1.03, null, null]]));
  assert.equal(s.slateFrames, 1);
  assert.equal(s.invalidFrames, 1);
});

test('a large gap fails the gate', () => {
  const s = summarize(steady(1).concat([[2.0, L, 30], [2.03, L, 31]]));
  assert.ok(s.maxGapS > 0.5);
  assert.ok(gateFailures(s).some((f) => f.includes('gap')));
});

test('the drop check can be turned off', () => {
  const over = Array.from({ length: 90 }, (_, i) => [i / 30, L, i * 3]);
  const s = summarize(over);
  assert.ok(gateFailures(s).some((f) => f.includes('dropped')));
  assert.deepEqual(gateFailures(s, { checkDrops: false }), []);
});

test('empty input is safe', () => {
  const s = summarize([]);
  assert.equal(s.frames, 0);
  assert.equal(s.fps, 0);
});

test('large gaps are listed with the time they started, so a long run explains itself', () => {
  const base = steady(2);                                              // 0 .. 1.97 s
  const afterGap = steady(1, 30, 60).map(([t, m, c]) => [t + 2.7, m, c]); // resumes 0.73 s after the last frame
  const s = summarize(base.concat(afterGap));
  assert.equal(s.bigGaps.length, 1);
  assert.ok(Math.abs(s.bigGaps[0][0] - 1.967) < 0.01);               // when the gap began
  assert.ok(Math.abs(s.bigGaps[0][1] - 0.733) < 0.01);               // how long it was
  assert.equal(summarize(steady()).bigGaps.length, 0);
});

test('the list of large gaps is capped so a bad run cannot flood the result', () => {
  const noisy = Array.from({ length: 200 }, (_, i) => [i * 0.5, L, i]);   // 0.5 s between every frame
  assert.ok(summarize(noisy).bigGaps.length <= 50);
});
