import { prepareOfficialInpaintAssets } from "@/utils/inpaintMask";

const BLEED = 16;
const SEAM = 8;
const LINE = 8;
const LINE_IN_MASK = 2;

export interface InsetCanvasSize {
  width: number;
  height: number;
  anlas: boolean;
}

export interface InsetRatio {
  id: string;
  label: string;
  sizes: InsetCanvasSize[];
}

/** Same families as NAIA's reference-inset window. Sizes over 1 megapixel cost Anlas. */
export const INSET_RATIOS: InsetRatio[] = [
  {
    id: "1:1",
    label: "1:1",
    sizes: [
      { width: 1024, height: 1024, anlas: false },
      { width: 1216, height: 1216, anlas: true },
      { width: 1472, height: 1472, anlas: true },
    ],
  },
  {
    id: "9:7",
    label: "9:7",
    sizes: [
      { width: 1152, height: 896, anlas: false },
      { width: 1440, height: 1120, anlas: true },
      { width: 1728, height: 1344, anlas: true },
    ],
  },
  {
    id: "3:2",
    label: "3:2",
    sizes: [
      { width: 1216, height: 832, anlas: false },
      { width: 1536, height: 1024, anlas: true },
      { width: 1728, height: 1216, anlas: true },
    ],
  },
  {
    id: "16:9",
    label: "16:9",
    sizes: [
      { width: 1344, height: 768, anlas: false },
      { width: 1536, height: 864, anlas: true },
      { width: 1920, height: 1088, anlas: true },
    ],
  },
];

export interface InsetPlacement {
  canvasWidth: number;
  canvasHeight: number;
  background: "white" | "black";
  scale: number;
  offsetX: number;
  offsetY: number;
  crop: boolean;
}

export interface InsetMetrics {
  width: number;
  height: number;
  insetWidth: number;
  sceneWidth: number;
  sceneHeight: number;
  drawPercent: number;
  x: number;
  y: number;
  drawW: number;
  drawH: number;
  lineLeft: number;
  lineRight: number;
}

function clamp(value: number, min: number, max: number) {
  return Math.min(max, Math.max(min, value));
}

export function measureInset(
  sourceWidth: number,
  sourceHeight: number,
  placement: InsetPlacement,
): InsetMetrics {
  const width = placement.canvasWidth || 1216;
  const height = placement.canvasHeight || 832;
  const sourceH = Math.max(1, sourceHeight);
  const sourceW = Math.max(1, sourceWidth);
  const fit = (height + BLEED * 2) / sourceH;
  const user = clamp(placement.scale || 1, 0.3, 2.5);
  const drawH = Math.max(1, Math.round((height + BLEED * 2) * user));
  const drawW = Math.max(1, Math.round(sourceW * fit * user));
  const x = -BLEED + (placement.offsetX || 0);
  const y = -BLEED + (placement.offsetY || 0);
  const visibleLeft = Math.max(0, x);
  const visibleRight = Math.min(width, Math.max(0, x + drawW));
  const insetWidth = clamp(Math.round(visibleRight / 8) * 8, 0, width);
  const sceneWidth = width - insetWidth;
  const seamLeft = Math.max(visibleLeft, insetWidth - SEAM);
  const lineRight = Math.min(insetWidth, seamLeft + LINE_IN_MASK);
  const lineLeft = Math.max(visibleLeft, lineRight - LINE);
  return {
    width,
    height,
    insetWidth,
    sceneWidth,
    sceneHeight: height,
    drawPercent: width ? Math.round((sceneWidth / width) * 100) : 0,
    x,
    y,
    drawW,
    drawH,
    lineLeft,
    lineRight,
  };
}

function loadImage(src: string) {
  return new Promise<HTMLImageElement>((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = () => reject(new Error("无法读取参考插条图片"));
    img.src = src;
  });
}

export function paintInset(
  ctx: CanvasRenderingContext2D,
  img: CanvasImageSource,
  metrics: InsetMetrics,
  background: "white" | "black",
) {
  ctx.fillStyle = background === "black" ? "#000000" : "#ffffff";
  ctx.fillRect(0, 0, metrics.width, metrics.height);
  ctx.drawImage(img, metrics.x, metrics.y, metrics.drawW, metrics.drawH);
  if (metrics.lineRight > metrics.lineLeft) {
    ctx.fillStyle = "#000000";
    ctx.fillRect(metrics.lineLeft, 0, metrics.lineRight - metrics.lineLeft, metrics.height);
  }
}

export async function prepareReferenceInset(imageUrl: string, placement: InsetPlacement) {
  const img = await loadImage(imageUrl);
  const metrics = measureInset(img.naturalWidth, img.naturalHeight, placement);
  if (metrics.sceneWidth < 64 || metrics.insetWidth < 32) {
    throw new Error("角色太宽，右边没有足够的地方作画。换一张更窄的全身图，或选更宽的画布。");
  }
  const imageCanvas = document.createElement("canvas");
  imageCanvas.width = metrics.width;
  imageCanvas.height = metrics.height;
  const imageCtx = imageCanvas.getContext("2d");
  if (!imageCtx) throw new Error("无法拼参考插条");
  paintInset(imageCtx, img, metrics, placement.background);

  const maskCanvas = document.createElement("canvas");
  maskCanvas.width = metrics.width;
  maskCanvas.height = metrics.height;
  const maskCtx = maskCanvas.getContext("2d");
  if (!maskCtx) throw new Error("无法生成参考插条蒙版");
  maskCtx.fillStyle = "#ffffff";
  maskCtx.fillRect(0, 0, metrics.width, metrics.height);
  maskCtx.fillStyle = "#000000";
  maskCtx.fillRect(0, 0, metrics.insetWidth, metrics.height);
  maskCtx.fillStyle = "#ffffff";
  const seamLeft = Math.max(0, metrics.insetWidth - SEAM);
  maskCtx.fillRect(seamLeft, 0, metrics.insetWidth - seamLeft, metrics.height);

  const assets = await prepareOfficialInpaintAssets(
    imageCanvas.toDataURL("image/png"),
    maskCanvas.toDataURL("image/png"),
    metrics.width,
    metrics.height,
  );
  return {
    ...assets,
    metrics,
    crop: placement.crop
      ? { x: metrics.insetWidth, y: 0, width: metrics.sceneWidth, height: metrics.sceneHeight }
      : undefined,
  };
}
