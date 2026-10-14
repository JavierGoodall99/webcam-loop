// THROWAWAY (Gate 0). JavaScript twin of probe/counter_codec.py and probe/stats.py, for the browser probe.
// A browser opens the camera as a shared client, so it can run while call apps use the camera
// (the OpenCV probe opens it exclusively and locks every other app out).

export const BITS = 48;
export const MARKER_LIVE = 0xa5c3;
export const MARKER_SLATE = 0x5a3c;
const CANDIDATE_BLOCKS = [8, 4, 12, 6, 16, 2, 3, 5, 7, 9, 10, 11, 13, 14, 15];

export const MAX_GAP_S = 0.5;
export const MAX_DROP_FRACTION = 0.01;
export const MIN_FPS = 27.0;
export const BIG_GAP_S = 0.25;
export const MAX_BIG_GAPS = 50;

function readValue(row, block) {
  // Returns the 48-bit value as [high16, low32], or null if the row is too narrow for this block size.
  let high = 0;
  let low = 0;
  for (let i = 0; i < BITS; i++) {
    const col = Math.floor((i + 0.5) * block);
    const o = col * 4;
    if (o + 2 >= row.length) return null;
    const bit = (row[o] + row[o + 1] + row[o + 2]) / 3 >= 128 ? 1 : 0;
    if (i < 16) high = (high << 1) | bit;
    else low = ((low << 1) | bit) >>> 0;
  }
  return [high, low];
}

// `row` is one RGBA pixel row (row 1 of the frame, inside the strip even when the picture is scaled down to a quarter).
export function decodeStripRow(row, width) {
  if (width < 2 || row.length < width * 4) return null;
  for (const block of CANDIDATE_BLOCKS) {
    const v = readValue(row, block);
    if (v === null) continue;
    const [marker, counter] = v;
    if (marker === MARKER_LIVE || marker === MARKER_SLATE) return { marker, counter };
  }
  return null;
}

// samples: [t_seconds, marker|null, counter|null]
export function summarize(samples, freezeThresholdS = 1.0) {
  const out = { frames: samples.length, durationS: 0, fps: 0, maxGapS: 0, dropped: 0, repeats: 0,
                freezeEvents: 0, slateFrames: 0, invalidFrames: 0, counterResets: 0, bigGaps: [] };
  if (!samples.length) return out;
  out.durationS = samples[samples.length - 1][0] - samples[0][0];
  if (samples.length > 1 && out.durationS > 0) {
    out.fps = (samples.length - 1) / out.durationS;
    for (let i = 1; i < samples.length; i++) {
      const gap = samples[i][0] - samples[i - 1][0];
      out.maxGapS = Math.max(out.maxGapS, gap);
      // [when the gap began, how long it was]; capped so a bad run cannot flood the saved result
      if (gap > BIG_GAP_S && out.bigGaps.length < MAX_BIG_GAPS) out.bigGaps.push([samples[i - 1][0], gap]);
    }
  }
  let prev = null;
  let runStart = null;
  let runFlagged = false;
  for (const [t, marker, counter] of samples) {
    if (marker === null) { out.invalidFrames++; prev = null; runStart = null; runFlagged = false; continue; }
    if (marker === MARKER_SLATE) { out.slateFrames++; prev = null; runStart = null; runFlagged = false; continue; }
    if (prev !== null) {
      const delta = counter - prev[2];
      if (delta > 1) out.dropped += delta - 1;
      else if (delta === 0) out.repeats++;
      else if (delta < 0) out.counterResets++;
      if (delta === 0) {
        if (runStart === null) runStart = prev[0];
        if (!runFlagged && t - runStart >= freezeThresholdS) { out.freezeEvents++; runFlagged = true; }
      } else { runStart = null; runFlagged = false; }
    }
    prev = [t, marker, counter];
  }
  return out;
}

export function gateFailures(s, { checkDrops = true } = {}) {
  const fails = [];
  if (s.freezeEvents > 0) fails.push(`freeze events: ${s.freezeEvents}`);
  if (s.maxGapS > MAX_GAP_S) fails.push(`max gap ${s.maxGapS.toFixed(2)}s > ${MAX_GAP_S}s`);
  const expected = s.frames + s.dropped;
  if (checkDrops && expected && s.dropped / expected > MAX_DROP_FRACTION)
    fails.push(`dropped ${s.dropped}/${expected} > ${MAX_DROP_FRACTION * 100}%`);
  if (s.invalidFrames > 0) fails.push(`undecodable frames: ${s.invalidFrames}`);
  if (s.frames > 1 && s.fps < MIN_FPS) fails.push(`fps ${s.fps.toFixed(1)} < ${MIN_FPS}`);
  return fails;
}
