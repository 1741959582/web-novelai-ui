const GRID = 8;
const SIZE_ALIGN = 64;

function ceilToMultiple(value: number, multiple: number) {
  return Math.max(multiple, Math.ceil(value / multiple) * multiple);
}

function stripDataUrl(value: string) {
  const idx = value.indexOf(",");
  return idx >= 0 ? value.slice(idx + 1) : value;
}

function loadImage(src: string) {
  return new Promise<HTMLImageElement>((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = () => reject(new Error("无法读取重绘底图或蒙版"));
    img.src = src;
  });
}

function drawToCanvas(img: HTMLImageElement, width: number, height: number) {
  const canvas = document.createElement("canvas");
  canvas.width = width;
  canvas.height = height;
  const ctx = canvas.getContext("2d");
  if (!ctx) throw new Error("无法处理重绘底图");
  ctx.drawImage(img, 0, 0, width, height);
  return canvas;
}

/**
 * Match the official NovelAI infill mask: sample the 1/8 latent grid, then
 * nearest-neighbour expand back to the request size as opaque black/white.
 * Sending the raw brush PNG lets the API resize it and can leave a flat blob.
 */
export async function prepareOfficialInpaintAssets(
  sourceDataUrl: string,
  maskDataUrl: string,
  fallbackWidth: number,
  fallbackHeight: number,
) {
  const source = await loadImage(sourceDataUrl);
  const mask = await loadImage(maskDataUrl);
  const width = ceilToMultiple(source.naturalWidth || fallbackWidth || 832, SIZE_ALIGN);
  const height = ceilToMultiple(source.naturalHeight || fallbackHeight || 1216, SIZE_ALIGN);
  const imageCanvas = drawToCanvas(source, width, height);

  const maskCanvas = document.createElement("canvas");
  maskCanvas.width = mask.naturalWidth || width;
  maskCanvas.height = mask.naturalHeight || height;
  const maskCtx = maskCanvas.getContext("2d", { willReadFrequently: true });
  if (!maskCtx) throw new Error("无法处理重绘蒙版");
  maskCtx.drawImage(mask, 0, 0);
  const src = maskCtx.getImageData(0, 0, maskCanvas.width, maskCanvas.height);

  const latentW = width / GRID;
  const latentH = height / GRID;
  const selected = new Uint8Array(latentW * latentH);
  let usesAlpha = false;
  for (let i = 3; i < src.data.length; i += 4) {
    if (src.data[i] !== 255) {
      usesAlpha = true;
      break;
    }
  }

  let any = false;
  for (let cellY = 0; cellY < latentH; cellY++) {
    for (let cellX = 0; cellX < latentW; cellX++) {
      const sourceX = Math.min(
        src.width - 1,
        Math.floor(((cellX + 0.5) * src.width) / latentW),
      );
      const sourceY = Math.min(
        src.height - 1,
        Math.floor(((cellY + 0.5) * src.height) / latentH),
      );
      const index = (sourceY * src.width + sourceX) * 4;
      const alpha = src.data[index + 3];
      const brightest = Math.max(src.data[index], src.data[index + 1], src.data[index + 2]);
      const active = usesAlpha ? alpha > 155 : alpha > 0 && brightest > 155;
      selected[cellY * latentW + cellX] = active ? 1 : 0;
      any ||= active;
    }
  }
  if (!any) throw new Error("蒙版为空，请先涂抹需要重绘的区域。");

  const out = document.createElement("canvas");
  out.width = width;
  out.height = height;
  const outCtx = out.getContext("2d");
  if (!outCtx) throw new Error("无法生成重绘蒙版");
  const dst = outCtx.createImageData(width, height);
  for (let y = 0; y < height; y++) {
    const cellY = Math.min(latentH - 1, Math.floor(y / GRID));
    for (let x = 0; x < width; x++) {
      const cellX = Math.min(latentW - 1, Math.floor(x / GRID));
      const value = selected[cellY * latentW + cellX] ? 255 : 0;
      const index = (y * width + x) * 4;
      dst.data[index] = value;
      dst.data[index + 1] = value;
      dst.data[index + 2] = value;
      dst.data[index + 3] = 255;
    }
  }
  outCtx.putImageData(dst, 0, 0);

  return {
    width,
    height,
    imageBase64: stripDataUrl(imageCanvas.toDataURL("image/png")),
    maskBase64: stripDataUrl(out.toDataURL("image/png")),
  };
}
