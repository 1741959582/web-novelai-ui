import type { AccountSummary, GenerateParams } from "@/types/nai";
import { isV4Plus } from "@/types/nai";

function clamp01(value: number, fallback = 0) {
  if (!Number.isFinite(value)) return fallback;
  return Math.min(1, Math.max(0, value));
}

const BASE_PIXEL_COEFFICIENT = 2951823174884865e-21;
const STEP_PIXEL_COEFFICIENT = 5753298233447344e-22;
const OPUS_FREE_MAX_PIXELS = 1024 * 1024;

export function quoteAnlas(opts: {
  params: GenerateParams;
  account?: AccountSummary;
  batchCount?: number;
  action?: "generate" | "img2img" | "infill";
  strength?: number;
  vibeCount?: number;
  encodedVibeCount?: number;
  preciseCount?: number;
}) {
  const samples = Math.max(1, Math.floor(opts.batchCount ?? 1));
  const width = Math.max(64, opts.params.width || 832);
  const height = Math.max(64, opts.params.height || 1216);
  const pixels = Math.max(width * height, 65_536);
  const steps = Math.max(1, opts.params.steps || 28);
  const action = opts.action ?? "generate";
  const strength = action === "generate" || action === "infill" ? 1 : clamp01(opts.strength ?? 1, 1);
  const opusTier = Boolean(opts.account?.hasActiveSubscription && (opts.account.tierLevel ?? 0) >= 3);
  // Official Opus: normal generation and inpaint are free up to 1024×1024 and 28 steps.
  // Image-to-image still costs Anlas, scaled by strength.
  const opus =
    (action === "generate" || action === "infill") &&
    opusTier &&
    pixels <= OPUS_FREE_MAX_PIXELS &&
    steps <= 28;

  const smeaMultiplier =
    !isV4Plus(opts.params.model) && opts.params.smeaDyn ? 1.4 : !isV4Plus(opts.params.model) && opts.params.smea ? 1.2 : 1;
  const officialBase = Math.ceil(BASE_PIXEL_COEFFICIENT * pixels + STEP_PIXEL_COEFFICIENT * pixels * steps);
  const basePerSample = opus ? 0 : Math.min(140, Math.max(2, Math.ceil(officialBase * smeaMultiplier * strength)));
  let total = basePerSample * samples;

  const vibeCount = Math.max(0, opts.vibeCount ?? 0);
  if (isV4Plus(opts.params.model) && vibeCount > 0) {
    const encoded = Math.min(vibeCount, Math.max(0, opts.encodedVibeCount ?? 0));
    total += 2 * Math.max(0, vibeCount - encoded);
    if (vibeCount > 4) total += 2 * (vibeCount - 4) * samples;
  }
  if ((opts.preciseCount ?? 0) > 0) total += 5 * samples;

  return total;
}

export function quoteUpscaleAnlas(width: number, height: number, scale = 4, opus = false) {
  const pixels = Math.max(1, width * height);
  const max = 1024 * 1024;
  const fitted = pixels > max ? max : pixels;
  if (opus && fitted <= 409_600) return 0;
  let amount = 7;
  if (fitted <= 262_144) amount = 1;
  else if (fitted <= 409_600) amount = 2;
  else if (fitted <= 524_288) amount = 3;
  else if (fitted <= 786_432) amount = 5;
  else if (fitted <= 1_048_576) amount = 7;
  return amount * (scale === 4 ? 2 : 1);
}
